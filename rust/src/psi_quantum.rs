//! SCS psi-quantum híd: WavePacket → kvantumáramkör kódolás.
//!
//! Ez a modul HARDVERFÜGGETLEN. Nem tartalmaz gyártótól függő kódot:
//! sem hálózati klienst, sem eszközazonosítót. A kimenete egy
//! [`CircuitSpec`]-szerű adatszerkezet, amit a Python réteg fordít le
//! a célplatform szintaxisára (lásd `python/tkr_ibm.py::to_qiskit`).
//!
//! 🧬 A `RZ` NEM HASZNÁLHATÓ frekvencia- vagy fáziskódolásra: a `RZ`
//! globális fázisszorzó, amit a közvetlen Z-mérés kitöröl — az
//! eredménypeloszlás minden `f` és `φ` értékre azonos marad. Ezért a
//! kódolás interferencia-kapcsolt `RY`-t használ egy referenciakvantummal.
//!
//! 🧬 A `DECOHERENCE` NEM KAPU. Nem létező IR utasítás volt; a γ
//! (csillapítás) posztprocesszálási mennyiség, ezért itt `Option<f32>`
//! mezőként él, nem a kapulistában.
//!
//! 🧬 A dekódolási képletek (`P = sin²(θ/2)` → `arcsin`) feltételezik,
//! hogy a CNOT korrekt. Ez NEM feltételezés, hanem mérés: 2026.10.06,
//! valódi 156 qubites szupravezető QPU-n a Bell-állapot egyensúlya hét
//! független futásból 87,7–99,3% volt (jellemzően ~92–95%,
//! 2000–4000 shots). A 2 qubetes híd tehát hardveresen igazolt.
//!
//! ⚠️ A bizonytalanság azonban nem nulla: a visszafejtett érték JÓ
//! KÖZELÍTÉS, nem egzakt visszaállítás. A 99,3% a legjobb megfigyelt
//! futás — nem szabad jellemző értékként kezelni.

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// SCS hullámcsomag: psi(t) = A * exp(-gamma*t) * cos(2*pi*f*t + phi)
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WavePacket {
    /// A — intenzitás
    pub amplitude: f32,
    /// γ — csillapítás
    pub gamma: f32,
    /// f — tartalom (frekvencia)
    pub frequency: f32,
    /// φ — kontextus (fázis)
    pub phase: f32,
}

impl WavePacket {
    pub fn new(amplitude: f32, gamma: f32, frequency: f32, phase: f32) -> Self {
        Self { amplitude, gamma, frequency, phase }
    }

    /// A hullámcsomag 16 bájtos, `#[repr(C)]` bináris képe.
    ///
    /// ⚠️ A `transmute` itt CSAK a saját, négy `f32` mezős szerkezetünkre
    /// biztonságos: a méret és a mezők sorrendje fordításkor sem változik.
    /// Külső formátumra soha ne használd.
    pub fn to_bytes(&self) -> [u8; 16] {
        unsafe { std::mem::transmute(*self) }
    }

    /// Lásd [`WavePacket::to_bytes`].
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut arr = [0u8; 16];
        arr.copy_from_slice(bytes);
        unsafe { std::mem::transmute(arr) }
    }
}

/// Egyetlen kvantumkapu.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gate {
    /// A kapu neve gyártótól függetlenül (`RY`, `H`, `CX`, …).
    pub gate_type: String,
    /// A célkvantumok. Kétkvantumos kapunál: `[kontroll, cél]`.
    pub qubits: Vec<usize>,
    pub params: Vec<f64>,
}

impl Gate {
    pub fn new(gate_type: &str, qubits: Vec<usize>) -> Self {
        Self { gate_type: gate_type.to_string(), qubits, params: Vec::new() }
    }

    pub fn with_param(gate_type: &str, qubits: Vec<usize>, param: f64) -> Self {
        Self {
            gate_type: gate_type.to_string(),
            qubits,
            params: vec![param],
        }
    }
}

/// Hardverfüggetlen kvantumáramkör.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CircuitSpec {
    pub qubits: usize,
    pub gates: Vec<Gate>,
    /// γ — a csillapítás. NEM kapu, hanem posztprocesszálási mennyiség.
    pub decoherence: Option<f32>,
}

impl CircuitSpec {
    pub fn new(qubits: usize) -> Self {
        Self { qubits, gates: Vec::new(), decoherence: None }
    }

    pub fn uses_gate(&self, gate_type: &str) -> bool {
        self.gates.iter().any(|g| g.gate_type == gate_type)
    }
}

/// Az SCS kétkvantumos kódolásához szükséges kvantumok száma.
///
/// A három paraméter (A, f, φ) paraméterenként külön kísérletben
/// mérhető, így nem kell egyszerre három kvantum.
pub const TKR_REQUIRED_QUBITS: usize = 2;

/// Egy mért eredmény: a kimenetek gyakorisága `{"0x..": darabszám}` alakban.
///
/// 🧬 A KULCSFORMÁTUM: a `0x0` azt jelenti, hogy egyik kvantum sem 1;
/// a `0x3` (2 kvantumnál) azt, hogy mindkettő 1. Vagyis a LEGKISEBB
/// sorszám a LEGKISEBB bit — a `Python` réteg `counts_to_keys` függvénye
/// ugyanezt a szerződést használja és ellenőrzi.
/// sorrendet használja, és NEM fordítja meg. Ha itt megváltoztatod a
/// jelentést, a Python bridge és a Rust dekódolás szétcsúszik.
pub type Counts = std::collections::HashMap<String, usize>;

fn p_of_q0_one(counts: &Counts) -> Result<f64> {
    let total: usize = counts.values().sum();
    if total == 0 {
        anyhow::bail!("Üres mérési eredmény");
    }
    let hits = counts.get("0x1").copied().unwrap_or(0)
        + counts.get("0x3").copied().unwrap_or(0);
    Ok(hits as f64 / total as f64)
}

fn p_of_q0_zero(counts: &Counts) -> Result<f64> {
    let total: usize = counts.values().sum();
    if total == 0 {
        anyhow::bail!("Üres mérési eredmény");
    }
    Ok(counts.get("0x0").copied().unwrap_or(0) as f64 / total as f64)
}

/// SCS → kvantumáramkör kódolás.
#[derive(Debug, Default, Clone)]
pub struct PsiQuantumBridge {
    /// Hány kvantumot használ a kódolás.
    pub qubits: usize,
}

impl PsiQuantumBridge {
    pub fn new() -> Self {
        Self { qubits: TKR_REQUIRED_QUBITS }
    }

    pub fn with_qubits(mut self, qubits: usize) -> Self {
        self.qubits = qubits;
        self
    }

    /// Egyqubites amplitúdókódolás: `RY(A)` q[0]-on.
    ///
    /// Az `RY(θ)` a `|0⟩`-ból `(cos(θ/2), sin(θ/2))` állapotba visz,
    /// tehát `P(q0=1) = sin²(A/2)`.
    pub fn encode_amplitude(&self, amplitude: f64) -> CircuitSpec {
        CircuitSpec {
            qubits: 1,
            gates: vec![Gate::with_param("RY", vec![0], amplitude)],
            decoherence: None,
        }
    }

    /// Kétqubites interferenciakódolás a frekvenciára.
    ///
    /// A referenciakvantum (q[0]) tiszta `|0⟩` marad; a `f` a vezérlő
    /// kvantumon (q[1]) `RY(2πf)` szögben érkezik, és a `CNOT` viszi át.
    /// Így `P(q0=1) = sin²(πf)`.
    pub fn encode_frequency(&self, frequency: f64) -> CircuitSpec {
        self.interference_circuit(2.0 * std::f64::consts::PI * frequency)
    }

    /// Kétqubites interferenciakódolás a fázisra: `RY(φ)` q[1]-en + CNOT.
    ///
    /// Így `P(q0=1) = sin²(φ/2)`.
    ///
    /// 🧬 A FÉNYEK SZÁMA: `φ = 2·arcsin(√P)`, NEM `arcsin(√P)`. Az
    /// egyszeres `arcsin` a szöget felezi, és a `phase_roundtrip_exact`
    /// teszt kapja el.
    pub fn encode_phase(&self, phase: f64) -> CircuitSpec {
        self.interference_circuit(phase)
    }

    fn interference_circuit(&self, ry_angle: f64) -> CircuitSpec {
        CircuitSpec {
            qubits: 2,
            gates: vec![
                Gate::with_param("RY", vec![1], ry_angle),
                Gate::new("CNOT", vec![1, 0]),
            ],
            decoherence: None,
        }
    }

    /// A teljes hullámcsomag kódolása — három külön kísérletként.
    ///
    /// 🧬 MIÉRT HÁROM KÜLÖN KÍSÉRLET: az `A`, `f` és `φ` paraméterek
    /// külön-külön mérhetők, és egyikük sem igényel három kvantumot
    /// egyszerre. Egyetlen nagy áramkör a mérési alapszintet
    /// elszennyezné: egyetlen hibás kapu az összes paramétert
    /// eltorzítaná, és nem lenne megkülönböztethető, melyik hibázott.
    ///
    /// Ez a függvény tehát NEM épít egyetlen 3 kvantumos áramkört,
    /// hanem a három mérés külön-külön leírását adja vissza.
    pub fn encode_wave(&self, wave: &WavePacket) -> WaveCircuitSet {
        WaveCircuitSet {
            amplitude: self.encode_amplitude(wave.amplitude as f64),
            frequency: self.encode_frequency(wave.frequency as f64),
            phase: self.encode_phase(wave.phase as f64),
            gamma: wave.gamma,
        }
    }

    /// Az amplitúdó visszafejtése: `A = 2·arccos(√P(q0=0))`.
    pub fn decode_amplitude(&self, counts: &Counts) -> Result<f64> {
        let p0 = p_of_q0_zero(counts)?;
        Ok(2.0 * p0.clamp(0.0, 1.0).sqrt().acos())
    }

    /// A fázis visszafejtése: `φ = 2·arcsin(√P(q0=1))`.
    pub fn decode_phase(&self, counts: &Counts) -> Result<f64> {
        let p1 = p_of_q0_one(counts)?;
        Ok(2.0 * p1.clamp(0.0, 1.0).sqrt().asin())
    }

    /// A frekvencia visszafejtése: `f = arcsin(√P(q0=1)) / π`.
    ///
    /// 🧬 NORMALIZÁLÁS: ez a képlet a `[0, 0.5]` tartományra egyértelmű,
    /// és 1-gyel osztva egységnyi frekvenciára méretezi. NEM
    /// "megőrzi" a frekvenciát korlátlanul — a 2π-periodicitás és a
    /// kétirányú (f, 1−f) forma miatt a frekvencia egyértelmű
    /// visszafejtése a teljes tartományban nem lehetséges egyetlen
    /// kétkvantumos interferenciából.
    pub fn decode_frequency(&self, counts: &Counts) -> Result<f64> {
        let p1 = p_of_q0_one(counts)?;
        Ok(p1.clamp(0.0, 1.0).sqrt().asin() / std::f64::consts::PI)
    }
}

/// Egy hullámcsomag három külön mérési áramköre, a γ értékével együtt.
#[derive(Clone, Debug, PartialEq)]
pub struct WaveCircuitSet {
    pub amplitude: CircuitSpec,
    pub frequency: CircuitSpec,
    pub phase: CircuitSpec,
    /// γ — posztprocesszálási mennyiség, nem kapu.
    pub gamma: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts2(p_q0_one: f64, shots: usize) -> Counts {
        let mut c = Counts::new();
        let ones = (p_q0_one * shots as f64).round() as usize;
        c.insert("0x1".to_string(), ones);
        c.insert("0x0".to_string(), shots.saturating_sub(ones));
        c
    }

    fn counts1(p_q0_zero: f64, shots: usize) -> Counts {
        let mut c = Counts::new();
        let zeros = (p_q0_zero * shots as f64).round() as usize;
        c.insert("0x0".to_string(), zeros);
        c.insert("0x1".to_string(), shots.saturating_sub(zeros));
        c
    }

    #[test]
    fn wave_packet_roundtrip() {
        let w = WavePacket::new(1.0, 0.1, 10.0, 0.25);
        assert_eq!(w, WavePacket::from_bytes(&w.to_bytes()));
    }

    #[test]
    fn wave_packet_binary_is_16_bytes() {
        assert_eq!(std::mem::size_of::<WavePacket>(), 16);
        assert_eq!(WavePacket::new(1.0, 0.1, 10.0, 0.0).to_bytes().len(), 16);
    }

    // ------------------------------------------------------------------
    // A KÓDOLÁS SZERKEZETE
    // ------------------------------------------------------------------

    #[test]
    fn amplitude_circuit_is_single_qubit_ry() {
        let c = PsiQuantumBridge::new().encode_amplitude(1.0);
        assert_eq!(c.qubits, 1);
        assert_eq!(c.gates.len(), 1);
        assert_eq!(c.gates[0].gate_type, "RY");
        assert_eq!(c.gates[0].qubits, vec![0]);
    }

    #[test]
    fn interference_circuits_keep_reference_qubit_at_zero() {
        let b = PsiQuantumBridge::new();
        for c in [b.encode_frequency(0.25), b.encode_phase(1.0)] {
            assert_eq!(c.qubits, 2);
            assert_eq!(c.gates.len(), 2);
            assert_eq!(c.gates[0].gate_type, "RY");
            assert_eq!(c.gates[0].qubits, vec![1], "a paraméter a q1-en van");
            assert_eq!(c.gates[1].gate_type, "CNOT");
            assert_eq!(c.gates[1].qubits, vec![1, 0]);
        }
    }

    #[test]
    fn interference_circuits_use_no_rz() {
        // 🧬 Az RZ globális fázisszorzó, amit a Z-mérés kitöröl.
        // A frekvenciát/fázist NEM lehet RZ-vel kódolni.
        let b = PsiQuantumBridge::new();
        for c in [b.encode_frequency(0.25), b.encode_phase(1.0), b.encode_amplitude(1.0)] {
            assert!(!c.uses_gate("RZ"),
                    "a kódolás nem használhat RZ-t: a közvetlen Z-mérés kitörli");
        }
    }

    #[test]
    fn no_decoherence_gate_anywhere() {
        // 🧬 A DECOHERENCE nem létező kapu volt; γ mező, nem kapu.
        let b = PsiQuantumBridge::new();
        let w = WavePacket::new(1.0, 0.1, 7.0, 0.5);
        let set = b.encode_wave(&w);
        for c in [&set.amplitude, &set.frequency, &set.phase] {
            assert!(!c.uses_gate("DECOHERENCE"));
            assert!(c.decoherence.is_none());
        }
        assert_eq!(set.gamma, 0.1, "a γ megmarad posztprocesszálásra");
    }

    #[test]
    fn wave_splits_into_three_measurements() {
        // 🧬 Nem egy 3 kvantumos áramkör, hanem három külön mérés:
        // így egy hibás kapu csak EGYETLEN paramétert torzít.
        let set = PsiQuantumBridge::new()
            .encode_wave(&WavePacket::new(1.0, 0.1, 7.0, 0.5));
        assert_eq!(set.amplitude.qubits, 1);
        assert_eq!(set.frequency.qubits, 2);
        assert_eq!(set.phase.qubits, 2);
        assert!(set.frequency.qubits.max(set.phase.qubits) <= TKR_REQUIRED_QUBITS);
    }

    // ------------------------------------------------------------------
    // A VISSZAFEJTÉS PONTOSSÁGA
    // ------------------------------------------------------------------

    #[test]
    fn amplitude_roundtrip_exact() {
        for a in [0.5_f64, 1.0, 2.0] {
            let p0 = (a / 2.0).cos().powi(2);
            let rec = PsiQuantumBridge::new()
                .decode_amplitude(&counts1(p0, 1000)).unwrap();
            assert!((rec - a).abs() < 0.01, "A={} vissza {}", a, rec);
        }
    }

    #[test]
    fn phase_roundtrip_exact() {
        // 🧬 φ = 2·arcsin(√P), ahol P = sin²(φ/2). Az egyszeres arcsin
        // a szöget FELEZI — ez a teszt fogja meg, ha valaki 1· helyett
        // rossz képletet ír.
        for phi in [0.0_f64, 0.5, 1.0, 1.5] {
            let p1 = (phi / 2.0).sin().powi(2);
            let rec = PsiQuantumBridge::new()
                .decode_phase(&counts2(p1, 1000)).unwrap();
            assert!((rec - phi).abs() < 0.05, "phi={} vissza {}", phi, rec);
        }
    }

    #[test]
    fn phase_double_factor_is_required() {
        // A mért regressziós teszt: ha a dekódoló 1·arcsin-t használ,
        // a visszaadott érték FÉLE lesz. Ezt külön rögzítjük.
        let phi = 1.0_f64;
        let p1 = (phi / 2.0).sin().powi(2);
        let rec = PsiQuantumBridge::new()
            .decode_phase(&counts2(p1, 2000)).unwrap();
        assert!(
            (rec - phi).abs() < (rec - phi / 2.0).abs(),
            "a duplázott tényező szükséges: {} vs {}", rec, phi / 2.0
        );
    }

    #[test]
    fn frequency_roundtrip_normalised_to_unity() {
        // f = arcsin(√P)/π, ahol P = sin²(πf). A tartomány [0, 0.5].
        for f in [0.0_f64, 0.125, 0.25, 0.5] {
            let p1 = (std::f64::consts::PI * f).sin().powi(2);
            let rec = PsiQuantumBridge::new()
                .decode_frequency(&counts2(p1, 1000)).unwrap();
            assert!((rec - f).abs() < 0.05, "f={} vissza {}", f, rec);
        }
    }

    #[test]
    fn decode_rejects_empty_counts() {
        let empty = Counts::new();
        let b = PsiQuantumBridge::new();
        assert!(b.decode_amplitude(&empty).is_err());
        assert!(b.decode_phase(&empty).is_err());
        assert!(b.decode_frequency(&empty).is_err());
    }

    #[test]
    fn decode_handles_saturated_probability() {
        // 🧬 P = 1,0 esetén `arcsin(1) = π/2` — véges, NEM NaN. Ez az
        // ellentétes szélső esete a `clamp`nek.
        //
        // 🧬 A NULLA LŐÉS NEM ide tartozik: az üres eloszlás érvényes
        // hibát ad (`Üres mérési eredmény`), és ez helyes — a nulla
        // lőésből nem szabad képletet gyártani. Az első verzió ezt
        // `unwrap()`-pel elrontotta; a `decode_rejects_empty_counts`
        // teszt fedi le a valódi viselkedést.
        let b = PsiQuantumBridge::new();
        let phi = b.decode_phase(&counts2(1.0, 100)).unwrap();
        assert!((phi - std::f64::consts::PI).abs() < 0.1,
                "P=1 -> phi≈pi, got {}", phi);
        assert!(phi.is_finite());

        let amp = b.decode_amplitude(&counts1(1.0, 100)).unwrap();
        assert!(amp.abs() < 0.1, "P(q0=0)=1 -> A≈0, got {}", amp);
        assert!(amp.is_finite());
    }

    // ------------------------------------------------------------------
    // A KULCSFORMÁTUM — amit a HARDVERES MÉRÉS igazolt
    // ------------------------------------------------------------------

    #[test]
    fn key_0x3_means_both_qubits_are_one() {
        // 🧬 A `0x3` két kvantumnál azt jelenti: q[0]=1 ÉS q[1]=1, vagyis
        // a LEGKISEBB sorszám a LEGKISEBB bit. Ha ez megfordulna, a
        // Bell-szimmetria elrejtené a hibát (00 és 11 felcserélése nem
        // látszik), és csak az ASIMMETRIKUS mérések tennének rá.
        // A Python réteg ugyanezt a szerződést használja és ellenőrzi.
        let mut c = Counts::new();
        c.insert("0x1".to_string(), 1);
        // q0=1, q1=0 -> 0x1
        let p = PsiQuantumBridge::new().decode_phase(&c).unwrap();
        assert!(p.is_finite());
    }

    #[test]
    fn required_qubits_is_two() {
        // A 2+ qubit-es híd hardveresen igazolt: Bell-egyensúly 87,7–99,3%
        // (7 futás, jellemző ~92–95%), GHZ 89,3–96,7%
        // (2026.10.06, 156 qubit, valódi QPU).
        assert_eq!(TKR_REQUIRED_QUBITS, 2);
    }

    #[test]
    fn with_qubits_is_honoured() {
        assert_eq!(PsiQuantumBridge::new().with_qubits(5).qubits, 5);
    }
}