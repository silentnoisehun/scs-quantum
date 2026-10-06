//! Sávfelosztás: a frekvenciatartomány 13 stabil sávra bontása.
//!
//! A white paper §3.1 szerint Shannon-Nyquist alapon 26 al-sáv gerjeszthető
//! átfedés nélkül, ebből 13 használható. Ez a modul a felosztás
//! *definícióját* és a frekvencia→sáv leképezést adja meg.
//!
//! A felosztás nem önkényes: minden sávnak van fizikai célja
//! (lassú hullámok, tartalom, tranziensek, szinkron), és a sávok
//! között nincs átfedés — ezt a [`assert_no_overlap`] ellenőrzi.

use crate::psi_quantum::WavePacket;

/// Frekvenciasávok száma (S₁₃).
pub const BAND_COUNT: usize = 13;

/// Egy frekvenciasáv-számítás.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Band {
    /// Egyenáram/nyugalmi komponens (B1).
    Dc,
    /// Lassú hullámok (B2–B4).
    Low,
    /// Fő tartalom (B5–B8).
    Mid,
    /// Tranziensek (B9–B12).
    High,
    /// Gyors szinkron (B13).
    Gamma,
}

impl Band {
    /// A sáv sorszáma 0-tól `BAND_COUNT - 1`-ig.
    pub fn index(self) -> usize {
        match self {
            Band::Dc => 0,
            Band::Low => 1,
            Band::High => 9,
            Band::Mid => 5,
            Band::Gamma => 12,
        }
    }

    /// A sáv emberi neve.
    pub fn name_hu(self) -> &'static str {
        match self {
            Band::Dc => "DC (nyugalmi)",
            Band::Low => "LOW (lassú hullámok)",
            Band::Mid => "MID (fő tartalom)",
            Band::High => "HIGH (tranziensek)",
            Band::Gamma => "GAMMA (szinkron)",
        }
    }

    /// A sáv a teljes tartomány mely részét fedi le, 0.0-tól 1.0-ig.
    ///
    /// A határok úgy vannak választva, hogy az 5 csoport egyenletes
    /// szélességű legyen a frekvenciatartományban — a spektrális
    /// felosztás alapelve.
    pub fn range(self) -> (f32, f32) {
        match self {
            Band::Dc => (0.0, 0.02),
            Band::Low => (0.02, 0.20),
            Band::Mid => (0.20, 0.60),
            Band::High => (0.60, 0.98),
            Band::Gamma => (0.98, 1.0),
        }
    }

    /// A sávba tartozó sorszámok tartománya (befogó).
    pub fn indices(self) -> std::ops::RangeInclusive<usize> {
        match self {
            Band::Dc => 0..=0,
            Band::Low => 1..=4,
            Band::Mid => 5..=8,
            Band::High => 9..=11,
            Band::Gamma => 12..=12,
        }
    }

    /// Mind az 5 sávcsoport.
    pub fn all() -> [Band; 5] {
        [
            Band::Dc,
            Band::Low,
            Band::Mid,
            Band::High,
            Band::Gamma,
        ]
    }

    /// A normalizált frekvenciából (`0.0`–`1.0`) a sáv meghatározása.
    ///
    /// A `freq_norm` a frekvencia aránya a legmagasabb használható
    /// frekvenciához képest — vagyis a Nyquist-fé felezés után.
    ///
    /// # Panika
    /// Ha `freq_norm` nem véges. Ha véges, de a tartományon kívül esik
    /// (`< 0.0`), a legalsó sávba soroljuk.
    pub fn of_frequency(freq_norm: f32) -> Band {
        assert!(freq_norm.is_finite(), "Band::of_frequency: nem véges bemenet");
        if freq_norm < 0.0 {
            return Band::Dc;
        }
        for band in Band::all() {
            let (lo, hi) = band.range();
            if freq_norm < hi || band == Band::Gamma {
                if freq_norm >= lo {
                    return band;
                }
            }
        }
        Band::Gamma
    }

    /// Az adott sávban tárolt WavePacket frekvenciájának normált értéke.
    ///
    /// Fordítottja az [`Band::of_frequency`] leképezésnek.
    pub fn frequency_of(band: Band, within: f32) -> f32 {
        let (lo, hi) = band.range();
        let within = within.clamp(0.0, 1.0);
        lo + within * (hi - lo)
    }
}

/// Ellenőrzi, hogy az öt sávcsoport lefedi az egész 0.0–1.0 tartományt,
/// átfedés és rés nélkül. A white paper §3.1 „átfedés nélkül" állításának
/// ellenőrzése.
pub fn assert_no_overlap() -> bool {
    let mut cursor = 0.0f32;
    for band in Band::all() {
        let (lo, hi) = band.range();
        assert!(
            (lo - cursor).abs() < 1e-6,
            "sávrés a határon: {:?} lo={} de az előző sáv {}-nál végződik",
            band,
            lo,
            cursor
        );
        assert!(hi > lo, "{:?}: hi={} nem nagyobb lo={}-nál", band, hi, lo);
        cursor = hi;
    }
    assert!(
        (cursor - 1.0).abs() < 1e-6,
        "a sávok {} -nál végződnek, nem 1.0-nál",
        cursor
    );
    true
}

/// Egy sávba helyezhető csomagok száma egy síkon.
pub const PACKETS_PER_PLANE: usize = PAGE_SIZE / PACKET_BYTES;

/// A `PAGE_SIZE` és `PACKET_BYTES` konstansok importja lokálisan, hogy a modul
/// önmagát dokumentálja.
use crate::field_engine::{PACKET_BYTES, PAGE_SIZE};

/// A sáv szerkezeti mérete egy síkon: hány csomag fér el.
pub fn packets_per_plane() -> usize {
    PACKETS_PER_PLANE
}

/// Egy WavePacket sávba sorolása a frekvenciájából.
///
/// A `WavePacket::frequency` nyers Hz-ben értendő, és a sávhatárok a
/// Nyquist-félre normált `[0.0, 1.0]` tartományra vonatkoznak. A
/// normalizálás ezért a `FREQUENCY_HZ_RANGE` névre ugrik: 2.0 Hz a Nyquist-fél,
/// vagyis `norm = f / 2.0`.
///
/// # Megjegyzés a határértékekhez
/// A `Band::range()`-ek `[lo, hi)` alakúak, ezért `frequency_of(band, 1.0)`
/// pont a *következő* sáv alsó határát adja. Emiatt a `within = 1.0` nem
/// önmagába tér vissza — ez a `frequency_roundtrip_within_band` teszt
/// ismert korlátja, nem a leképezés hibája. A visszatérés csak
/// `within ∈ [0.0, 1.0)` esetén zárt.
pub fn band_of_wave(wave: &WavePacket) -> Band {
    // A normált frekvencia: a 0.0 referencia 0 Hz, az 1.0 a Nyquist-fél.
    // A SCS-ben a frekvenciahordozó 0 és 0.5 közé esik.
    let norm = (wave.frequency / 2.0).clamp(0.0, 1.0);
    Band::of_frequency(norm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn band_count_matches_paper() {
        assert_eq!(BAND_COUNT, 13);
        let total: usize = Band::all().iter().map(|b| b.indices().count()).sum();
        assert_eq!(total, 13, "a sávoknak összesen 13 indexet kell fedniük");
    }

    #[test]
    fn band_indices_are_contiguous_and_unique() {
        let mut seen = vec![false; BAND_COUNT];
        for band in Band::all() {
            for i in band.indices() {
                assert!(!seen[i], "a {}. index duplán fedi", i);
                seen[i] = true;
                assert_eq!(
                    band.index() <= i,
                    true,
                    "a {:?} sáv indexe {} nem tartozik a sávhoz",
                    band,
                    i
                );
            }
        }
        assert!(seen.iter().all(|&s| s), "minden indexnek lefedetté kell lennie");
    }

    #[test]
    fn no_overlap_covers_unity() {
        assert!(assert_no_overlap());
    }

    #[test]
    fn frequency_maps_into_expected_bands() {
        assert_eq!(Band::of_frequency(0.0), Band::Dc);
        assert_eq!(Band::of_frequency(0.1), Band::Low);
        assert_eq!(Band::of_frequency(0.3), Band::Mid);
        assert_eq!(Band::of_frequency(0.8), Band::High);
        assert_eq!(Band::of_frequency(0.99), Band::Gamma);
    }

    #[test]
    fn frequency_roundtrip_within_band() {
        // A sávhatárok félköztesek: a `within=1.0` érték már a KÖVETKEZŐ
        // sáv határára esik. Ez a `of_frequency` helyes viselkedése,
        // nem hiba — a határ mindig egyértelműen egy sávhoz tartozik.
        for band in Band::all() {
            for within in [0.0f32, 0.25, 0.5, 0.75] {
                let f = Band::frequency_of(band, within);
                assert_eq!(
                    Band::of_frequency(f),
                    band,
                    "{:?} @ {:.2} → {:.4} nem önmaga",
                    band,
                    within,
                    f
                );
            }
        }
    }

    #[test]
    fn band_boundary_belongs_to_the_upper_band() {
        // Minden dokumentált határ a felső sávhoz tartozik — ez a
        // `of_frequency` "<" összehasonlításának következménye.
        assert_eq!(Band::of_frequency(0.02), Band::Low);
        assert_eq!(Band::of_frequency(0.20), Band::Mid);
        assert_eq!(Band::of_frequency(0.60), Band::High);
        assert_eq!(Band::of_frequency(0.98), Band::Gamma);
    }

    #[test]
    fn wave_lands_in_expected_band() {
        // A `band_of_wave` a frekvenciát 2.0-val normalizálja (Nyquist-fél),
        // tehát a 0.2 frekvencia 0.1 normált érték = LOW sáv.
        let mut w = WavePacket {
            amplitude: 1.0,
            gamma: 0.0,
            frequency: 0.0,
            phase: 0.0,
        };
        assert_eq!(band_of_wave(&w), Band::Dc);
        w.frequency = 0.2;
        assert_eq!(band_of_wave(&w), Band::Low);
        w.frequency = 0.8;
        assert_eq!(band_of_wave(&w), Band::Mid);
        w.frequency = 1.6;
        assert_eq!(band_of_wave(&w), Band::High);
    }

    #[test]
    fn packets_per_plane_is_508() {
        // 8128 / 16 = 508 csomag egy síkon — a white paper §4 mérete
        assert_eq!(packets_per_plane(), 508);
        assert_eq!(packets_per_plane() * PACKET_BYTES, PAGE_SIZE);
    }

    #[test]
    fn band_names_are_hungarian() {
        for band in Band::all() {
            let n = band.name_hu();
            assert!(!n.is_empty());
            assert!(n.chars().next().unwrap().is_uppercase());
        }
    }

    #[test]
    fn of_frequency_handles_out_of_range() {
        assert_eq!(Band::of_frequency(-1.0), Band::Dc);
        assert_eq!(Band::of_frequency(10.0), Band::Gamma);
    }

    #[test]
    #[should_panic(expected = "nem véges")]
    fn of_frequency_rejects_nan() {
        Band::of_frequency(f32::NAN);
    }
}
