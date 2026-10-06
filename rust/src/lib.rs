//! SCS (Space Computing System) kvantum híd.
//!
//! A csomag két hardverfüggetlen réteget tartalmaz:
//!
//! - **`field_engine`** — a 13 frekvenciasáv × 9 mélység lapokra osztott
//!   hullámtár; a visszafejtés roundtrip-pontosságát teszteli.
//! - **`band_map`** — a sávhatár-szemantika kanonikus szerződése.
//! - **`psi_quantum`** — a WavePacket → kvantumáramkör kódolás és a
//!   visszafejtés.
//!
//! 🧬 HARDVERES BIZONYÍTÉK (2026.10.06, 156 qubites IBM szupravezető QPU,
//! 2000–4000 shots/db): a 2 qubites Bell-állapot egyensúlya hét
//! független futásból **87,7–99,3%** (jellemzően ~92–95%), a 3 qubites
//! GHZ 89,3–96,7%. A `psi_quantum` dekódolási képleteinek
//! `P(q0=1) = sin²(θ/2)` feltevése tehát méréssel igazolt, nem
//! feltételezés.
//!
//! ⚠️ DE A VISSZAFEJTÉS NEM EGZAKT: a mérési bizonytalanság miatt a
//! visszafejtett érték JÓ KÖZELÍTÉS. A 99,3% a legjobb megfigyelt
//! futás, nem a jellemző érték — lásd a `README.md` hardveres
//! eredménytáblázatát a hét futás mindegyikével.
//!
//! A crate NEM tartalmaz hálózati klienst vagy eszközazonosítót: a
//! kvantumáramkör kimenete platformfüggetlen adatszerkezet, amit a
//! Python réteg fordít le a célplatform szintaxisára.

pub mod band_map;
pub mod field_engine;
pub mod psi_quantum;

pub use band_map::{assert_no_overlap, band_of_wave, packets_per_plane, Band};
pub use field_engine::{
    FieldEngine, FieldPage, BAND_COUNT, MAX_DEPTH, PACKET_BYTES, PAGE_SIZE,
    PHASE_DRIFT_PER_DEPTH, PHASE_READ_LIMIT,
};
pub use psi_quantum::{
    CircuitSpec, Counts, Gate, PsiQuantumBridge, WaveCircuitSet, WavePacket,
    TKR_REQUIRED_QUBITS,
};