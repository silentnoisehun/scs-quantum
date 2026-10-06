//! Tér-motor: Az SCS 13×9 hullámtér lapokra osztott, dinamikus tárolója.
//!
//! A white paper v2.0 §5 "Tér-motor Rustban" pseudokódjának implementációja.
//! A tér S₁₃ frekvenciasávból és D₉ mélységi rétegből áll; minden logikai sík
//! [`PAGE_SIZE`] bájt, minden elhelyezés egy [`PACKET_BYTES`] bájtos WavePacket.
//!
//! Nincs `unsafe` ebben a modulban: a síkok `Vec<u8>` lapmemóriát használnak, és
//! a WavePacket↔bájt konverziót a crate-ben már meglévő `WavePacket::to_bytes` /
//! `WavePacket::from_bytes` végzi (az továbbra is a meglévő transmute-es,
//! `#[repr(C)]` struktúrát használja — azt itt nem módosítom).

use crate::psi_quantum::WavePacket;

/// Frekvenciasávok száma (S₁₃).
pub const BAND_COUNT: usize = 13;

/// Legnagyobb stabil mélységi réteg (D₉). A 10. réteg fázisszétesés miatt tiltott.
pub const MAX_DEPTH: usize = 9;

/// Logikai sík mérete bájtban (white paper §4: 8128 bájt/sík).
pub const PAGE_SIZE: usize = 8128;

/// Egy WavePacket szerkezeti mérete bájtban (4 × f32, `#[repr(C)]`).
pub const PACKET_BYTES: usize = 16;

/// Mélységenkénti fázisdrift (rad/réteg). A white paper §3.2 mért k értéke.
pub const PHASE_DRIFT_PER_DEPTH: f32 = 0.17;

/// A fázis olvashatósági határa: Δφ < π/2 rad.
pub const PHASE_READ_LIMIT: f32 = std::f32::consts::FRAC_PI_2;

/// Egy logikai sík: fix méretű, léptethető bájtlap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldPage {
    bytes: Vec<u8>,
    page_size: usize,
}

impl FieldPage {
    /// Üres lap `page_size` kapacitással (white paper: `FieldPage::with_capacity`).
    pub fn with_capacity(page_size: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(page_size),
            page_size,
        }
    }

    /// A lap kapacitása (logikai síkméret) bájtban.
    pub fn page_size(&self) -> usize {
        self.page_size
    }

    /// A lap ténylegesen lefoglalt hossza bájtban.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Igaz-e, hogy a lap még üres.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// `data` bájtok beírása `off` eltolásra.
    ///
    /// # Panika
    /// Ha az írás átnyúlná a lap határán — ez a `page_size % PACKET_BYTES != 0`
    /// esetén fordulhatna elő, amit a [`FieldEngine::new`] már kiszűr.
    pub fn write(&mut self, off: usize, data: &[u8]) {
        assert!(
            off + data.len() <= self.page_size,
            "FieldPage::write: írás átnyúlná a lap határán (off={}, len={}, page_size={})",
            off,
            data.len(),
            self.page_size
        );
        if self.bytes.len() < off + data.len() {
            self.bytes.resize(off + data.len(), 0);
        }
        self.bytes[off..off + data.len()].copy_from_slice(data);
    }

    /// `len` bájt visszaolvasása `off` eltolásról.
    ///
    /// # Panika
    /// Ha a kért tartomány nem fér a lapba, vagy még nincs megírva.
    pub fn read(&self, off: usize, len: usize) -> [u8; PACKET_BYTES] {
        assert_eq!(
            len, PACKET_BYTES,
            "FieldPage::read: csak {}-bájtos csomag olvasható",
            PACKET_BYTES
        );
        assert!(
            off + len <= self.page_size,
            "FieldPage::read: olvasás átnyúlná a lap határán (off={}, len={}, page_size={})",
            off,
            len,
            self.page_size
        );
        assert!(
            off + len <= self.bytes.len(),
            "FieldPage::read: a kért terület nincs megírva (off={}, len={}, írt={})",
            off,
            len,
            self.bytes.len()
        );
        let mut out = [0u8; PACKET_BYTES];
        out.copy_from_slice(&self.bytes[off..off + len]);
        out
    }
}

/// Az SCS tér-motor: lapokra osztott hullámtár `S₁₃ × D₉` méretben.
#[derive(Debug, Clone)]
pub struct FieldEngine {
    pages: Vec<FieldPage>,
    page_size: usize,
}

impl FieldEngine {
    /// Új tér-motor `page_size` bájtos síkokkal.
    ///
    /// A white paper szerinti 8128 bájt/sík 16 bájttal osztható (508 csomag/sík),
    /// így egy csomag soha nem fog átnyúlni egy lap határán.
    ///
    /// # Panika
    /// Ha `page_size` 0, 16-nál kisebb, vagy nem osztható 16-tal.
    pub fn new(page_size: usize) -> Self {
        assert!(
            page_size >= PACKET_BYTES,
            "FieldEngine::new: a síkméret legalább {} bájt kell legyen, kapott {}",
            PACKET_BYTES,
            page_size
        );
        assert!(
            page_size.is_multiple_of(PACKET_BYTES),
            "FieldEngine::new: a síkméret {} osztható {}-tal kell legyen, különben egy WavePacket lap-határon átnyúlná",
            page_size,
            PACKET_BYTES
        );
        Self {
            pages: Vec::new(),
            page_size,
        }
    }

    /// A white paper szerinti tér-motor a 8128 bájtos síkkal.
    pub fn new_standard() -> Self {
        Self::new(PAGE_SIZE)
    }

    /// A logikai sík mérete bájtban.
    pub fn page_size(&self) -> usize {
        self.page_size
    }

    /// A lapokból foglalt jelenlegi darabszám.
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// `(band, depth, index)` → logikai bájteltolás.
    ///
    /// `(band * 10 + depth) * page_size + index * 16`
    ///
    /// # Panika
    /// Ha `band >= 13` vagy `depth > 9`.
    #[allow(dead_code)]
    fn logical_index(&self, band: usize, depth: usize, index: usize) -> usize {
        assert!(
            band < BAND_COUNT,
            "logical_index: band={} kívül esik az S{} sávon",
            band,
            BAND_COUNT
        );
        assert!(
            depth <= MAX_DEPTH,
            "logical_index: depth={} kívül esik a D{} határon",
            depth,
            MAX_DEPTH
        );
        (band * 10 + depth) * self.page_size + index * PACKET_BYTES
    }

    /// A logikai sík sorszáma (`band * 10 + depth`) — a lapozás kulcsa.
    #[allow(dead_code)]
    fn plane_index(&self, band: usize, depth: usize) -> usize {
        self.logical_index(band, depth, 0) / self.page_size
    }

    /// WavePacket kiírása a `(band, depth, index)` logikai helyre.
    ///
    /// Ha a logikai index átnyúlik az aktuális utolsó lapon, új lapokat foglal
    /// (`Vec::resize_with`), ahogy a white paper §5 pseudokódja írja.
    ///
    /// # Panika
    /// - `0.17 * depth >= π/2` esetén: fázis-összeomlás (a 10. réteg tiltott),
    /// - `band >= 13` vagy `depth > 9` esetén: síkhatár,
    /// - ha egy csomag átnyúlná egy lap határán.
    pub fn write_wave(&mut self, band: usize, depth: usize, index: usize, wave: &WavePacket) {
        // Fázis-stabilitás: Δφ(depth) = k · depth < π/2
        let err = PHASE_DRIFT_PER_DEPTH * depth as f32;
        assert!(
            err < PHASE_READ_LIMIT,
            "Fázis összeomlana depth={} (Δφ={} rad ≥ π/2 = {} rad)",
            depth,
            err,
            PHASE_READ_LIMIT
        );
        let bytes = wave.to_bytes();
        let li = self.logical_index(band, depth, index);
        let p = li / self.page_size;
        let off = li % self.page_size;
        if self.pages.len() <= p {
            self.pages
                .resize_with(p + 1, || FieldPage::with_capacity(self.page_size));
        }
        self.pages[p].write(off, &bytes);
    }

    /// WavePacket visszaolvasása a `(band, depth, index)` logikai helyről.
    ///
    /// # Panika
    /// - `band >= 13` vagy `depth > 9` esetén: síkhatár,
    /// - ha a lap nincs még foglalva, vagy az elhelyezés nincs megírva.
    pub fn read_wave(&self, band: usize, depth: usize, index: usize) -> WavePacket {
        let li = self.logical_index(band, depth, index);
        let p = li / self.page_size;
        let off = li % self.page_size;
        assert!(
            p < self.pages.len(),
            "Nincs foglalt lap a logikai helyhez (lap={}, foglalt={}) — írd be előbb a hullámot",
            p,
            self.pages.len()
        );
        WavePacket::from_bytes(&self.pages[p].read(off, PACKET_BYTES))
    }
}

impl Default for FieldEngine {
    fn default() -> Self {
        Self::new_standard()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_wave() -> WavePacket {
        WavePacket {
            amplitude: 1.0,
            gamma: 0.1,
            frequency: 10.0,
            phase: 0.0,
        }
    }

    /// White paper §6 / 1. bizonyítás: a fázishiba 0..=9 mélységen olvasható,
    /// a 10. rétegen már nem.
    /// A white paper §6 szó szerinti alakja: a két állítás konstans kifejezés,
    /// éppen ez a bizonyítás lényege — a határ futásidejű mérésből nem függ.
    #[test]
    fn depth_phase_stability() {
        for d in 0u8..=9 {
            assert!(
                PHASE_DRIFT_PER_DEPTH * (d as f32) < PHASE_READ_LIMIT,
                "mélység {} stabilnak kell lennie",
                d
            );
        }
        // A white paper §6 `assert!(!(0.17 * 10.0 < FRAC_PI_2))` alakjának
        // azonosítója: a 10. réteg fázisszáma már NEM kisebb π/2-nél.
        // `const` blokkban fordításkor kiértékelődik — a határ nem futásidejű mérés.
        const { assert!(PHASE_DRIFT_PER_DEPTH * 10.0 >= PHASE_READ_LIMIT) };
    }

    /// White paper §6 / 4. bizonyítás: mind a 10 megengedett rétegen
    /// bitpontos visszaolvasás.
    #[test]
    fn roundtrip_9_depth() {
        let mut engine = FieldEngine::new(8128);
        let wave = WavePacket {
            amplitude: 1.0,
            gamma: 0.1,
            frequency: 10.0,
            phase: 0.0,
        };
        for depth in 0..=9 {
            engine.write_wave(7, depth, 0, &wave);
            assert_eq!(wave, engine.read_wave(7, depth, 0), "mélység {}", depth);
        }
    }

    /// White paper §6: a 10. mélység panic-et dob.
    #[test]
    #[should_panic]
    fn reject_depth_10() {
        let mut e = FieldEngine::new(8128);
        let w = WavePacket {
            amplitude: 1.0,
            gamma: 0.1,
            frequency: 10.0,
            phase: 0.0,
        };
        e.write_wave(7, 10, 0, &w); // Panic: fázis összeomlás
    }

    /// A lapozás valódi: a logikai index síkonként és laponként nő,
    /// és az új lapot tényleg foglalja le a FieldEngine.
    #[test]
    fn paging_allocates_real_pages() {
        let mut e = FieldEngine::new_standard();
        let w = sample_wave();

        assert_eq!(e.page_count(), 0, "üres térnek nincs lapja");

        // 0. sík (band 0, depth 0) → 0. lap
        e.write_wave(0, 0, 0, &w);
        assert_eq!(e.page_count(), 1);

        // band 1, depth 0 → sík 10 → a 10. lapon kezdődik.
        // A 10. lap (index 10) megcímzéséhez 11 lap foglalódik: 0..=10.
        e.write_wave(1, 0, 0, &w);
        assert_eq!(
            e.page_count(),
            11,
            "a 10. logikai síkot foglaló lapig (0..=10) tizenegy lap kell"
        );

        // band 12, depth 9 → sík 129 → 130. lap
        e.write_wave(12, 9, 0, &w);
        assert_eq!(e.page_count(), 130);

        // minden írás külön logikai helyen van, nem ürültek össze
        assert_eq!(e.read_wave(0, 0, 0), w);
        assert_eq!(e.read_wave(1, 0, 0), w);
        assert_eq!(e.read_wave(12, 9, 0), w);
    }

    /// A logikai index képlete pontosan (band*10 + depth) * page_size + index*16,
    /// és a síkok nem csúsznak össze.
    #[test]
    fn logical_index_layout() {
        let e = FieldEngine::new(8128);
        assert_eq!(e.logical_index(0, 0, 0), 0);
        assert_eq!(e.logical_index(0, 0, 1), 16);
        assert_eq!(e.logical_index(0, 1, 0), 8128);
        assert_eq!(e.logical_index(1, 0, 0), 81_280);
        assert_eq!(e.logical_index(12, 9, 0), 129 * 8128);
        assert_eq!(e.plane_index(7, 3), 73);

        // 507. és 508. csomag ugyanabban a lapban, de más offseten
        assert_eq!(e.logical_index(0, 0, 507) / 8128, 0);
        assert_eq!(e.logical_index(0, 0, 508) / 8128, 1);
    }

    /// 508 csomag fér egy síkba — a 8128 = 508 × 16 éppen.
    #[test]
    fn full_plane_fits_in_one_page() {
        let mut e = FieldEngine::new_standard();
        assert_eq!(8128 / PACKET_BYTES, 508);
        for i in 0..508usize {
            let w = WavePacket {
                amplitude: i as f32,
                gamma: 0.5,
                frequency: 3.0,
                phase: 0.25,
            };
            e.write_wave(0, 0, i, &w);
        }
        assert_eq!(e.page_count(), 1, "egy teljes sík egy lap");
        for i in 0..508usize {
            let r = e.read_wave(0, 0, i);
            assert_eq!(r.amplitude, i as f32);
        }
        // az 509. csomag átnyúlik a síkon → új lap
        e.write_wave(0, 0, 508, &sample_wave());
        assert_eq!(e.page_count(), 2);
    }

    /// Mind a 13 sáv mind a 10 megengedett rétegén megvan az adat.
    #[test]
    fn all_bands_all_depths_roundtrip() {
        let mut e = FieldEngine::new_standard();
        for band in 0..BAND_COUNT {
            for depth in 0..=MAX_DEPTH {
                let w = WavePacket {
                    amplitude: (band * 10 + depth) as f32 * 0.5,
                    gamma: 0.125,
                    frequency: 440.0 + band as f32,
                    phase: depth as f32 * 0.3,
                };
                e.write_wave(band, depth, 17, &w);
            }
        }
        for band in 0..BAND_COUNT {
            for depth in 0..=MAX_DEPTH {
                let w = WavePacket {
                    amplitude: (band * 10 + depth) as f32 * 0.5,
                    gamma: 0.125,
                    frequency: 440.0 + band as f32,
                    phase: depth as f32 * 0.3,
                };
                assert_eq!(e.read_wave(band, depth, 17), w, "sáv{} mélység{}", band, depth);
            }
        }
    }

    /// A 13. sávnál feljebb már nincs sík — band=13 panic.
    #[test]
    #[should_panic]
    fn reject_band_13() {
        let mut e = FieldEngine::new_standard();
        e.write_wave(13, 0, 0, &sample_wave());
    }

    /// A 16-tal nem osztható síkméretet a konstruktor elutasítja,
    /// különben egy csomag lap-határon átnyúlna.
    #[test]
    #[should_panic]
    fn reject_unaligned_page_size() {
        let _ = FieldEngine::new(100);
    }
}