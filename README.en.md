# SCS — Space Computing System

Frequency-domain wave-packet encoding with a quantum bridge **validated on real
quantum hardware**.

> 🧬 **The system's 2- and 3-qubit quantum bridge is hardware-proven.**
> Measured on a real 156-qubit superconducting QPU: Bell-state balance **99.3%**,
> GHZ ideal (2000 shots, IBM `ibm_marrakesh`, 2026-10-06).

**Hungarian:** [README in Hungarian](README.md) ·
**Documents:** [White Paper (EN)](docs/SCS_WHITE_PAPER.en.md) ·
[White Paper (HU)](docs/SCS_WHITE_PAPER.md) ·
[Engineering Design (EN)](docs/SCS_QUANTUM_DESIGN.en.md) ·
[Engineering Design (HU)](docs/SCS_QUANTUM_DESIGN.md)

---

## What this system is

SCS decomposes a wave packet

```
ψ(t) = A · exp(−γ·t) · cos(2π·f·t + φ)
```

into four parameters — **A** amplitude (intensity), **γ** damping, **f** frequency
(content), **φ** phase (context) — and encodes them into a **quantum circuit** so
that they can be decoded from the measured output distribution.

The frequency-domain approach stores the state not in `N` qubits but in a
**13 frequency bands × 9 depth layers** structure.

## Quick start

```bash
git clone https://github.com/silentnoisehun/scs-quantum
cd scs-quantum

# 1. Verify the classical layer (nothing to install)
cd rust && cargo test --release && cd ..
python -m proofs.tkr_proofs

# 2. Verify the measurement code on an IDEAL simulator (no token needed)
python -m python.tkr_ibm bell --local --shots 2000

# 3. Re-run the hardware evidence (token required)
pip install "scs-quantum[qpu]"
export IBM_QUANTUM_API_TOKEN="..."
python -m python.tkr_ibm devices
python -m python.tkr_ibm bell --backend ibm_marrakesh --shots 2000
```

## Measured hardware results

`ibm_marrakesh`, 156 qubits, real superconducting QPU, 2000 shots per run.

| Circuit | Measured | Balance |
|---|---|---|
| `zero --qubits 1` | `0x0` = 98.25% | measurement floor |
| `zero --qubits 2` | `0x0` = 98.70% | measurement floor |
| `zero --qubits 3` | `0x0` = 97.40% | measurement floor |
| `h` | `00` = 50.55%, `01` = 49.35% | 50/50 control |
| **`bell`** | `00` = 49.35%, `11` = 49.00% | **99.3%** |
| `bell` (run 2) | `00` = 48.50%, `11` = 47.90% | **98.8%** |
| `bell` (after refactor) | `00` = 49.30%, `11` = 47.95% | **97.3%** |
| **`ghz`** | `000` = 49.20%, `111` = 47.60% | ideal, noise 1.80% |

Bell noise (01+10) = 1.65% — normal for a 2-qubit superconducting QPU.

**From this follows** that the decoding formulas' assumption
`P(q0=1) = sin²(θ/2)` is not an assumption but a measurement-verified claim.

## 🧬 Two levels of proof — do not conflate them

| Level | What it proves | Command |
|---|---|---|
| **Classical** | the correctness of the CODE | `cargo test --release` → 36 tests |
| **Hardware** | the operation of the QUANTUM BRIDGE | `python -m python.tkr_ibm bell --backend …` |

**Green tests do NOT prove hardware behaviour.** The exclusive hardware evidence
is proof point 5. The `--local` and `--simulator` modes are NOT QPU evidence —
they verify the measurement code.

## The encoding scheme

Every parameter is measured in a **separate circuit**. This is deliberate: in one
large circuit a single faulty gate would distort all three parameters, and it
would be impossible to tell which one failed.

| Parameter | Qubits | Circuit | Decoding |
|---|---|---|---|
| `A` | 1 | `RY(A)` q[0] | `A = 2·arccos(√P(q0=0))` |
| `f` | 2 | `RY(2πf)` q[1] + `CNOT q1→q0` | `f = arcsin(√P(q0=1)) / π` |
| `φ` | 2 | `RY(φ)` q[1] + `CNOT q1→q0` | `φ = 2·arcsin(√P(q0=1))` |
| `γ` | — | none — post-processing | — |

q[0] stays a pure `|0⟩` reference.

**The previous `RZ`-based scheme did not work.** `RZ` is a global phase
multiplier that a direct Z-measurement erases — the output distribution stayed
identical for every `f` and `φ` value. The `RY`-based interference scheme fixes
this.

## Package layout

```
rust/                  the classical field engine and the encoding bridge
  src/field_engine.rs    13 bands × 9 layers wave storage, bit-exact write/read
  src/band_map.rs        band-boundary contract: [lo, hi), upper belongs to next
  src/psi_quantum.rs     WavePacket ↔ quantum circuit, hardware-independent
python/
  tkr_measure.py         measurement layer (stdlib only!) — WavePacket,
                         CircuitSpec, reference circuits, bit-order check, report
  tkr_ibm.py             IBM Quantum backend + CLI
proofs/
  tkr_proofs.py          the 8 proof points, each with a status
docs/
  SCS_WHITE_PAPER.md        white paper (Hungarian)
  SCS_WHITE_PAPER.en.md     white paper (English)
  SCS_QUANTUM_DESIGN.md     engineering document (Hungarian)
  SCS_QUANTUM_DESIGN.en.md  engineering document (English)
config/.env.template     where environment variables go
RELEASE.md               the DOI / Zenodo / arXiv release steps
```

The core (`tkr_measure.py`) requires **zero third-party dependencies**. Qiskit is
needed only for QPU measurements, and is an optional extra.

## Security

The API token is supplied via an **environment variable** only, and the package
never writes it to disk:

```bash
export IBM_QUANTUM_API_TOKEN="..."
```

Create a token at <https://quantum.cloud.ibm.com> → Account settings → API key;
it is 44 characters and **shown only once**. See `config/.env.template` for the
full set of variables.

## The measurement methodology

Four rules, each derived from a detected bug:

1. **The reference size must match the circuit size.** A 4-qubit `|0⟩` reference
   cannot be compared with a 2-qubit circuit — the noise levels differ.
2. **An automatic evaluator must be able to distinguish good from bad.** If it
   rejects a good sample, the condition is too wide; if it passes a bad one, too
   narrow.
3. **Bit order must be proven with an asymmetric probe.** The Bell state is
   symmetric, so reversed bits are invisible on it.
4. **A hardware-noise diagnosis comes after the reference.** A single distribution
   cannot separate noise from a transpilation or registration error.

`bit_order_selfcheck` runs before every measurement, and **rejects the run** if it
does not pass.

## Limitations — what the system does NOT prove

- **O(1) quantum-resonance routing is an assumption.** No complexity measurement
  exists, and a 2-qubit encoding does not by itself give a quantum advantage — a
  classical computation does the same thing.
- **The depth-9 boundary is measurement-dependent.** The boundary exists, but the
  value 9 follows from the measured `k = 0.17 rad/layer`; with ±2% uncertainty, 8,
  9 or 10 may follow. Not an independent physical law.
- **Frequency decoding is unambiguous only on `[0, 0.5]`.** Because of the 2π
  periodicity and the two-to-one (f, 1−f) form, a single two-qubit interference
  cannot invert the full domain.
- **The 13-band Shannon–Nyquist justification is conditional.** This project does
  not measure the hardware bandwidth it depends on.

## Citing this work

```bibtex
@software{scs_quantum_2026,
  title  = {SCS: A Frequency-Domain Wave-Packet Encoding System
            with a Hardware-Validated Quantum Bridge},
  author = {Máté Róbert},
  year   = {2026},
  version = {0.2.0},
  license = {MIT},
  url    = {https://github.com/silentnoisehun/scs-quantum},
  doi    = {10.5281/zenodo.XXXXXXX}   % the concept DOI, minted on Zenodo
}
```

See `RELEASE.md` for how the DOI is minted, and `CITATION.cff` for the
machine-readable metadata.

## Licence

MIT
