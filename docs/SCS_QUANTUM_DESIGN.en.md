# The SCS Quantum Bridge — Engineering Design Document

**Audience:** anyone modifying the `scs-quantum` crate or the Python measurement
layer, or adding a new quantum-hardware backend.
**Companion document:** `SCS_WHITE_PAPER.en.md` (the scientific claims and the
hardware validation).

---

## 0. Claim markers

This document uses the same marker system as the white paper, so that a claim
does not change meaning between the two documents.

| Marker | Meaning |
|---|---|
| ✅ **PROVEN (classical)** | Proven by `cargo test --release` (36 tests) or an offline computation |
| 🔬 **PROVEN (hardware)** | Measured on a real QPU |
| ⚠️ **MEASUREMENT-DEPENDENT** | True, but its numeric value derives from a measurement |
| ⚠️ **ASSUMPTION** | No measurement behind it |
| ℹ️ **CONVENTION** | A chosen definition |

> ### ⚠️ Green tests do not prove the hardware
>
> The 36 green Rust tests establish the correctness of the **classical
> software**. They do not claim that the hardware produces the same result. The
> source for every hardware-related statement is exclusively the QPU measurement
> performed by `python -m python.tkr_ibm`.
>
> The same holds at this document's engineering level: a passing `to_qiskit` unit
> test proves that the lowering produces a circuit matching the specification —
> **not** that the lowered circuit behaves correctly on the QPU.

---

## 1. What is this layer's job, and what is not

| Layer | Responsibility | Dependencies |
|---|---|---|
| `rust/src/psi_quantum.rs` | encoding / decoding, producing `CircuitSpec` | `anyhow`, `serde` |
| `rust/src/field_engine.rs` | wave-packet storage, depth validation | – |
| `rust/src/band_map.rs` | band-boundary contract, frequency → band | – |
| `python/tkr_measure.py` | hardware-independent measurement contracts | – |
| `python/tkr_ibm.py` | `CircuitSpec` → Qiskit, execution, reporting | `qiskit`, `qiskit-ibm-runtime` |

The crate has **two** dependencies (`anyhow`, `serde`). The earlier layer
containing a network client, vendor device identifiers and an external IR emitter
has been removed from the crate entirely. ✅ **PROVEN (classical)** — verifiable
by the two dependency entries in `rust/Cargo.toml`.

---

## 2. Why `CircuitSpec` rather than a platform circuit directly

`CircuitSpec` is a **hardware-independent data structure**: a qubit count, a gate
list and parameters. The Rust layer knows no Qiskit class, no backend and no
networking.

This is decisive for three reasons:

1. **Testability.** The entire encoding and decoding logic can be tested without
   hardware, network or credentials. The result of `encode_frequency(0.25)` is a
   `CircuitSpec` whose gate list a unit test compares row by row. No QPU needed.
2. **Swapping.** For a new target platform, **only the lowering layer** is
   rewritten. The encoding formulas, the band contract and the storage remain
   unchanged (§10).
3. **Independence.** The crate needs no vendor SDK installed or linked in order to
   build and test.

**Inside the lowering layer.** `python/tkr_measure.py` itself contains the
`CircuitSpec`, `Gate` and `Counts` definitions — a hardware-independent,
vendor-neutral model whose semantics match the Rust-side spec. Keeping the two
definitions in agreement is a **contract, not an automatic guarantee**: if the
meaning of a field changes on one side, the other side must be changed too. The
key contract of §5 is the most sensitive point for exactly this reason.

---

## 3. The data structures

### 3.1 `CircuitSpec` (Rust)

```rust
pub struct CircuitSpec {
    pub qubits: usize,
    pub gates: Vec<Gate>,
    pub decoherence: Option<f32>,   // γ — NOT a gate
}

pub struct Gate {
    pub gate_type: String,          // canonical name: "RY", "H", "CX", "CNOT", …
    pub qubits: Vec<usize>,         // 1 qubit: [q]; 2 qubits: [control, target]
    pub params: Vec<f64>,           // rotation angle
}
```

**The `decoherence` field is not a gate.** The earlier `DECOHERENCE` "gate" was a
non-existent IR instruction: γ (damping) is not a quantum-state quantity but
post-processing metadata. The `no_decoherence_gate_anywhere` test pins that no
encoding output contains such a gate, and that γ survives in the
`WaveCircuitSet` field. ✅ **PROVEN (classical)**

**Three parameters as separate outputs.** `encode_wave` does not return a single
circuit but a `WaveCircuitSet`:

```rust
pub struct WaveCircuitSet {
    pub amplitude: CircuitSpec,   // 1 qubit
    pub frequency: CircuitSpec,   // 2 qubits
    pub phase:     CircuitSpec,   // 2 qubits
    pub gamma:     f32,           // post-processing
}
```

This keeps the complete encoding within **2 qubits**
(`TKR_REQUIRED_QUBITS = 2`). The `wave_splits_into_three_measurements` test pins
this: in one large circuit a single faulty gate would distort every parameter,
with no way to tell which one failed.

### 3.2 The canonical gate name matters

Arity is examined on the **canonical** name (`g.gate_type`), not on the mapped
name (`cx`). This is not a style question:

> 🧬 An earlier iteration sent `cx` into the single-qubit branch, so `cx(control)`
> was called with **one** argument →
> `missing 1 required positional argument: 'target_qubit'`.

This is why there is a separate `GATE_ALIASES` and a separate `TWO_QUBIT_GATES`
set: the alias is the *mapping* name, the arity is the *contract*. If a new gate
is two-qubit, it must be added to `TWO_QUBIT_GATES`, or it will silently be
treated as single-qubit.

---

## 4. `CircuitSpec` → Qiskit lowering rules

`to_qiskit(spec, QuantumCircuit)` is the single entry point of the lowering.

### 4.1 The arity rule

**Application is arity-based, not list-length-based:**

| Gate class | Listed qubits | Call form |
|---|---|---|
| single-qubit (`H`, `X`, `Y`, `Z`, `RY`, `RZ`) | `[q]`, or `[q0, q1, …]` | **separately on every listed qubit** |
| two-qubit (`CX`, `CNOT`, `CZ`, `SWAP`, `ISWAP`) | exactly `[control, target]` | **both in one call** |

```python
if g.gate_type in TWO_QUBIT_GATES:
    method(g.qubits[0], g.qubits[1])       # cx(control, target)
else:
    for q in g.qubits:                     # h, ry, … on each listed q
        method(q, g.params[0]) if g.params else method(q)
```

This is why the `random` reference looks like this: `Gate("H", [0, 1, 2])` —
**one** gate entry, three applications.

### 4.2 Why the broadcast form cannot be used

⚠️ **Qiskit 2.x removed the broadcast form.** `qc.h(range(n))` no longer works:

```
TypeError: takes 2 positional arguments but N were given
```

This is why the `random` reference cannot be written as `qc.h(range(n))` either.
The arity-based application is not a style choice but a **compatibility
requirement** for Qiskit 2.x.

### 4.3 Two-qubit length validation

If a gate in `TWO_QUBIT_GATES` does not receive exactly two qubits, the lowering
raises immediately:

```
ValueError: CX két kvantumot kér, 1 megadva
```

This is **fail-fast**: a malformed spec yields an immediate error rather than a
wrong circuit, and rather than a silent misdirection. The same holds for an
unknown gate name (`ismeretlen kapu: …`).

### 4.4 Adding the measurement

At the end of the loop, every qubit is measured into the classical register of the
same index:

```python
qc.measure(range(spec.qubits), range(spec.qubits))
```

The `c[i] = q[i]` pairing is ℹ️ **CONVENTION**, and it is what fixes the meaning
of the key order — see §5.

### 4.5 The meaning of two-qubit direction

The SCS-specific `CX` lowers to Qiskit's `cx`, and **the first qubit is the
control, the second is the target**. `CNOT` is an alias of `cx`, so the
SCS-specific `CNOT` name and the Qiskit-level `cx(control, target)` meaning agree.

SCS's frequency and phase encoding uses `CNOT` in the **`q[1]` → `q[0]`**
direction: the control (`q[1]`) carries the parameter, and the target (`q[0]`) is
the clean reference. The `interference_circuits_keep_reference_qubit_at_zero`
test pins that `q[0]` receives no `RY`, so it genuinely remains a reference. ✅
**PROVEN (classical)** — for hardware behaviour see white paper §4.2.

---

## 5. The key contract

This is the most sensitive part of the document, because a **silent** mistake can
be made here.

### 5.1 The format

`counts_to_keys` converts measurement results into the `{"0x..": count}` form:

| Key | Meaning (for 2 qubits) |
|---|---|
| `0x0` | `q[0]=0`, `q[1]=0` |
| `0x1` | **`q[0]=1`**, `q[1]=0` |
| `0x2` | `q[0]=0`, `q[1]=1` |
| `0x3` | `q[0]=1`, `q[1]=1` |

**The rule: the key's least significant bit is `c[0]`.**

The two formats use the **same** order:
- Qiskit writes the classical register left to right, highest index first, so the
  key is `c[n-1] … c[1] c[0]` — `c[0]` (rightmost) is the least significant bit;
- `0x3` means `q[0]=1` **and** `q[1]=1`.

**So there is no reversal.** `counts_to_keys` deliberately does not reverse the
bits.

### 5.2 Why it is critical: the bug is invisible on the best test

> 🧬 **The measured lesson:** an earlier implementation reversed the bits, and
> this produced **no loud error**. The Bell state is symmetric (`00`/`11`), so
> reversed bits are invisible on it. It only surfaces on an **asymmetric**
> circuit (for example the `X q[1]` probe), where it **mimics a hardware fault**.
>
> This is the most hazardous error class: it signals nothing and produces a false
> diagnosis — the measurement would report a faulty qubit while the bug is in the
> software.

### 5.3 `bit_order_selfcheck` — before every measurement

Because the bug lives silently inside the conversion, **the conversion checks
itself**, and this check runs **before every run**:

```
bit-sorrend önellenőrzés: X q[1] -> 0x2 (várt 0x2) ✓
```

The probe compares an **unknown, one-directional** result against the known
expected value:

| Step | Result |
|---|---|
| simulated measurement | `{"10": 100}` |
| `counts_to_keys(·, 2)` | `{"0x2": 100}` |
| with reversed bits | `{"0x1": 100}` ← ✗ fails |

**If the check fails, the run is rejected** (`BitOrderCheckFailed`, exit code
**5**), rather than returning a misleading result. `bit_order_selfcheck` runs at
the start of `IbmRunner.run`, before the measurement.

**Do not relax it:** if someone skips this check to "simplify" the code, a silent
bug returns to the system and the best test takes over the role of bug-detector.
The `tkr_measure.py` module docstring says this explicitly.

---

## 6. The reference circuits

The references are **not** SCS encodings. They are the hardware's measuring
instruments: every reference has a **known expected outcome**, and if the hardware
does not produce it, the fault is in the hardware or in the measurement path.

| Name | Size | Gates | Expected |
|---|---|---|---|
| `zero` | 1 (any) | none | `0x0` ≈ 100% — the measurement floor |
| `h` | 2 | `H q[0]` | `00` ≈ `01` ≈ 50/50, `10`/`11` vanishing |
| `bell` | 2 | `H q[0]`, `CX q[0]→q[1]` | `00` ≈ `11` ≈ 50% |
| `ghz` | 3 | `H q[0]`, `CX q[0]→q[1]`, `CX q[1]→q[2]` | `000` ≈ `111` ≈ 50% |
| `random` | 4 | `H` on every qubit | uniform, `100/2ⁿ` per outcome |

### 6.1 The size-matching rule

> ⚠️ **The reference size must match the circuit size.** Measurements of different
> sizes have different noise levels and cannot be compared.

This is why the `--qubits` resize **rebuilds the circuit** via `resize_reference`,
rather than merely rewriting the measurement label:

```python
if n != spec.qubits:
    spec = resize_reference(kind, n)
```

The difference is not academic but practical: if the measurement layer asks for
3 qubits while the circuit still contains 4 `H` gates, the measurement returns
**4-bit keys**, and the converter rejects them:

```
bites szám-eltérés: '1100' (4 bit) vs n=3
```

— or, worse, would silently accept them.

`resize_reference` also respects the minimum sizes: `zero` is valid at any size
(it has no gates), but `h` and `bell` require at least 2 qubits and `ghz` at
least 3.

### 6.2 The `--qubits` override

`zero` establishes the measurement floor: since the qubits are certainly in `|0⟩`,
`0x0` close to 100% is expected. `1×0` → 98.25%, `2×0` → 98.70%,
`3×0` → 97.40% 🔬 **PROVEN (hardware)**. Increasing the size shows that the
measurement floor is **not qubit-count-dependent** — which means the deviations
observed elsewhere come from the gates, not from the measurement infrastructure.

---

## 7. The evaluator — `report_counts`

### 7.1 The `bell` flag is mandatory

Bell verification is valid **exclusively** for the Bell circuit, so the caller
passes it as an explicit flag:

```python
bell=(a.cmd == "bell")
```

> 🧬 **The flag is not an optional convenience.** Bell verification previously
> triggered on the presence of the `0x0` **and** `0x3` keys. A clean `H q[0]`
> measurement can produce both keys — and the report then printed *„CNOT not
> working"* for a **working, ideal** measurement.
>
> **The measured proof:** the `h` command wrote `⚠️ not ideal` without the flag,
> and with the flag returned a clean **50.55 / 49.35** for the very same
> measurement. The measurement did not change — the *evaluation* improved.

### 7.2 The `uniform_bits` parameter

The uniformity check receives the expected value **from the caller**, and does not
estimate it from the keys that appeared:

```python
uniform_bits=(n if a.cmd == "random" else None)
```

If the server does not return every outcome, the largest reported key may carry
**fewer bits**, and the ideal value computed from it would be falsely too high.

### 7.3 The evaluator's main rule

> 🧬 An automatic evaluator **must be able to distinguish the GOOD sample from the
> BAD one.** If it rejects a good sample, the condition is **too wide**; if it
> passes a bad one, it is **too narrow**.

When adding a new check condition to `report_counts`, this question **must** be
answered — otherwise the measurement produces a false result.

### 7.4 What a poor `00`/`11` ratio does NOT prove

If the `00`/`11` ratio is weak, that alone **does not prove hardware noise**. It
may equally be a transpilation error, a bit-order problem or a calibration
deviation. Separating them requires a `|0>` reference (the `zero` command).

> ⚠️ **This is the engineering form of white paper rule 4.3:** a single
> distribution cannot determine the origin of a fault. The order is mandatory:
> `zero` first, and only then may anything else be called noise.

---

## 8. Execution modes

### 8.1 QPU mode (the hardware proof)

```bash
export IBM_QUANTUM_API_TOKEN='<token>'
python -m python.tkr_ibm bell --shots 2000
```

The sequence:

1. `bit_order_selfcheck()` — **rejects the run** if it fails;
2. `to_qiskit(spec, QuantumCircuit)` — the lowering;
3. `transpile(qc, backend, optimization_level=1, seed_transpiler=42)` — the fixed
   seed gives **reproducible** transpilation;
4. `SamplerV2(mode=backend).run([(tq,)], shots=N)`;
5. `counts_to_keys(counts, n)` — the key conversion, without reversal;
6. `report_counts(...)` — the evaluation.

**`backend.run()` does not exist.** `qiskit-ibm-runtime` 0.5x removed it
(`Support for backend.run() has been removed`), so execution goes through the
**Sampler primitive**:

```python
from qiskit_ibm_runtime import SamplerV2
job = SamplerV2(mode=self.backend).run([(tq,)], shots=shots)
```

This was an **API contract** error, not a measurement error: the code compiled and
failed only at run time. When the next `qiskit-ibm-runtime` major release arrives,
the `SamplerV2` construction is the first thing to check.

### 8.2 The `--local` mode — what it proves and what it does not

```bash
python -m python.tkr_ibm bell --local --shots 2000
```

**What it proves:** that the measurement pipeline is correct — bit order, the
classical register pairing, the conversion and the report. With an ideal
`StatevectorSampler`, any error can only be in **our own code**, because there is
no physical noise and no transpilation.

**What it does NOT prove:** anything about the hardware.

This is why `--local` (and `--simulator`) print a highly visible warning on
startup:

```
!!  NEM VALÓDI HARDVER. Ez IDEÁLIS számítás.
!!  Eredménye a KÓD PIPELINE-ellenőrzése, nem QPU-validáció.
!!  Ha ez elhasal, a kód hibás. Ha átmegy, a kód rendben van —
!!  de a hardverről még semmit nem tudtunk.
```

> ⚠️ **Writing a simulator result into the proof material would assert exactly the
> opposite of what was measured.** The output may be called
> `statevector (ideális, helyi)`, and nothing else.

`--local` requires no token, so it is usable in CI and in teaching environments to
verify the pipeline.

### 8.3 Exit codes

Errors are separate classes, so a calling script can distinguish them:

| Code | Raised by |
|---|---|
| 0 | success |
| 1 | `IbmNotInstalled` — `qiskit` / `qiskit-ibm-runtime` missing |
| 2 | `IbmAuthFailed` — token/CRN wrong, or no access to the instance |
| 3 | `IbmBackendUnavailable` — no working QPU (quota, maintenance) |
| 4 | execution error |
| **5** | **`BitOrderCheckFailed` — the measurement was rejected as misleading** |

Code 5 deserves separate attention: there is **no result**, rather than a bad
result.

---

## 9. Authentication and secrets

**The token never reaches disk.** It is supplied exclusively via environment
variable:

| Variable | Required? | Meaning |
|---|---|---|
| `IBM_QUANTUM_API_TOKEN` | yes (alternative: `QISKIT_IBM_TOKEN`) | API token |
| `IBM_QUANTUM_INSTANCE` | optional (alternative: `QISKIT_IBM_INSTANCE`) | CRN |
| `IBM_QUANTUM_CHANNEL` | optional, default `ibm_quantum_platform` | channel |

Verification rules:

1. **No file-based reading.** `_token()` calls `os.getenv` exclusively. There is no
   config file, no keystore, no command-line argument.
2. **The missing token carries its own error.** The exception message says what to
   set (`export IBM_QUANTUM_API_TOKEN='<token>'`) and where to get it
   (`Account settings → API key`). The error is **not** re-wrapped: double
   wrapping ("authentication failed: IbmAuthFailed: token missing") hides the
   real cause.
3. **The `dotenv` load is optional.** If `python-dotenv` is installed, a local
   `.env` may be loaded — but that is the user's decision, and the token still
   enters via the environment, not via the code.
4. **`--local` requires no credential.** The local ideal mode is deliberately
   credential-free so it can be used for pipeline verification.

> **Before a public release:** no token, CRN or instance identifier may reach the
> repository. The `.env` file is gitignored, and the token is never an argument —
> so it is not even stored in the shell history.

---

## 10. Adding a new hardware backend

The crate requires **no** modification. A backend affects only the Python layer.
The required steps:

### 10.1 The backend implementation

Following the pattern of `python/tkr_ibm.py`, the backend fulfils this contract:

| Contract | Requirement |
|---|---|
| **Input** | one `CircuitSpec` + `shots` |
| **Output** | `{bitstring: count}` in the platform's native form |
| **Precondition** | `bit_order_selfcheck()` passed |
| **Name** | `backend_label` — the identifier shown in the report |

### 10.2 The `IbmRunner` surface to implement

| Method | Responsibility |
|---|---|
| `__init__` | initialisation + `self._refs = reference_circuits()` |
| `_pick_backend` | backend selection, signalling `IbmBackendUnavailable` |
| `run(kind, qubits, shots)` | size matching → self-check → lowering → execution → conversion |
| `describe()` | print backend status |
| `backend_label` | report the true mode, also in the `local` case |

### 10.3 What a new backend **must** inherit

These four points come from the measurement lessons and are **not optional**:

1. **Size matching** (`resize_reference`) — measurements of different sizes cannot
   be compared (§6.1).
2. **`bit_order_selfcheck` before the measurement** — otherwise a silent
   bit-order bug returns (§5.3).
3. **Use of `counts_to_keys`** — key conversion stays in one central place
   (§5.1).
4. **The `bell=True` flag only on the Bell circuit** — otherwise false-positive
   Bell verification (§7.1).

### 10.4 What a new backend **must not** do

- **Must not reverse the bits** for the sake of "normalisation". The contract is
  `c[0] = LSB`.
- **Must not estimate the expected value from the measured keys** — `uniform_bits`
  comes from the caller (§7.2).
- **Must not present a simulator result as hardware proof.** `backend_label` is
  obliged to distinguish the ideal mode.
- **Must not carry a config filename in the crate.** Credentials come from the
  environment only (§9).

### 10.5 Extending the lowering

If the new platform uses different gate names, **only** the `to_qiskit`-style
lowering function needs modification:

- the new name must be added to the alias table;
- if the new gate is two-qubit, it must also be added to the `TWO_QUBIT_GATES`
  set (§3.2 — otherwise it is silently treated as single-qubit);
- if the platform cannot express the `Gate` contract, then the `Gate`/`CircuitSpec`
  **specification** must be extended — and in the **Rust crate too**, because the
  definitions on the two sides must agree.

---

## 11. The preserve record

This module **guards three detected bugs**. Do not delete them for the sake of
"simplification" — each was either present silently, or produced a false-positive
report.

| Guarded contract | The bug it guards |
|---|---|
| `bit_order_selfcheck` + reversal-free conversion | Bit-order reversal — invisible on the symmetric Bell distribution, and on an asymmetric probe it **mimicked a hardware fault** |
| The `bell` flag in `report_counts` | False-positive Bell verification — a clean `H q[0]` measurement was judged "CNOT not working" |
| `SamplerV2(mode=backend)` | Removal of `backend.run()` in `qiskit-ibm-runtime` 0.5x |

Two further limits pinned in the specification:

| Limit | Where |
|---|---|
| `f = arcsin(√P)/π` is unambiguous only on `[0, 0.5]` | `decode_frequency` docstring + `frequency_roundtrip_normalised_to_unity` |
| `φ = 2·arcsin(√P)`, **not** `arcsin(√P)` | the `phase_double_factor_is_required` regression test |

The latter exists as a separate test, not merely alongside
`phase_roundtrip_exact`, because a single `arcsin` **halves** the angle, and that
is only visible if someone deliberately substitutes the wrong formula.

---

## 12. Test coverage — what it proves and what it does not

`cargo test --release` → **36 passed, 0 failed** ✅ **PROVEN (classical)**

| Module | What it proves |
|---|---|
| `field_engine` | bit-exact packet write/read for every band and stable layer; the 13th band and the 10th layer rejected |
| `band_map` | the bands cover the range without overlap or gap; the upper boundary belongs to the higher band; closure of `within ∈ [0,1)` |
| `psi_quantum` | the structure of the encoding formulas; decoding accuracy on ideal distributions; the `RZ` ban; γ is not a gate; the factor 2 in phase decoding |

> ### ⚠️ What the 36 tests do NOT prove
>
> - They do **not** prove that the hardware produces the same result.
> - They do **not** prove that the `to_qiskit` lowering produces a correct circuit
>   on the QPU — that requires the measurement of §8.1.
> - They do **not** prove unambiguous frequency decoding over the full domain —
>   that is a known limitation of the encoding, not a test question.
>
> The only valid hardware proof is the QPU measurement performed by
> `python -m python.tkr_ibm`. 🔬 **PROVEN (hardware)** — see white paper §4.2.

---

## 13. Quick reference

```bash
# Full classical test suite
cd rust && cargo test --release

# Pipeline verification in IDEAL simulation — no token needed,
# asserts nothing about the hardware
python -m python.tkr_ibm bell --local --shots 2000

# Hardware validation — real QPU
export IBM_QUANTUM_API_TOKEN='<token>'
python -m python.tkr_ibm zero --qubits 3 --shots 2000
python -m python.tkr_ibm h    --shots 2000
python -m python.tkr_ibm bell --shots 2000
python -m python.tkr_ibm ghz  --shots 2000
```

| Exit code | Meaning |
|---|---|
| 0 | success |
| 1 | `qiskit` / `qiskit-ibm-runtime` not installed |
| 2 | authentication error (token / CRN / channel) |
| 3 | no QPU backend available |
| 4 | execution error |
| 5 | **bit-order check failed — the measurement was rejected** |

---

## 14. Tesseract Anchor Integration (SCS V0.4 + Quantum Anchor V1.2)

> **Status:** AWAITING IQM MEASUREMENT — anchor drive compensation validation in progress.

SCS V0.4 integrates the **Quantum Anchor V1.2 Tesseract architecture** as a
coherence-preservation layer. This complements the `python/tkr_ibm.py` IBM-based
quantum bridge with a pulse-level anchoring mechanism.

### 14.1 Tesseract Architecture

| Parameter | Value |
|---|---|
| **Planes** | 4 (XY, XZ, XW, YZ) — orthogonal in R⁴ |
| **Realities/plane** | 5 (phases: 0, 72°, 144°, 216°, 288°) |
| **Total realities** | 20 simultaneous pre-realities |
| **Measurement** | No collapse — selection R = |⟨ψ_anchor|ψ_answer⟩|² |
| **Resonance** | R ≥ 0.5 → γ = 0 (anchored); R < 0.5 → γ = 0.1 (self-annihilation) |

Mathematical form:
```
Ψ(x,y,z,w) = Π_{i=1}^{4} λ_i · δ(p_i - p0_i) · ψ(t)
```
where λ_i = 0.08, p0_i ∈ {0.0, 0.25, 0.5, 0.75}.

### 14.2 Hardware Status (2026-10-07)

| Component | Evidence Grade | Source |
|---|---|---|
| **ψ(37ns) = 0.331662** (single-qubit dynamics) | 🔬 **PROVEN (hardware)** | ibm_marrakesh VALIDATION.md §7.7 |
| **Borg 16-node 100% clear (96.43% balance)** | 🔬 **PROVEN (hardware)** | ibm_marrakesh VALIDATION.md §8 |
| **Matryoshka D0→D8 fractal preservation** | ⚠️ **UNVERIFIED** | 97.4%→89.4%, cumulative noise |
| **Anchor drive compensation (Tesseract 4-plane)** | ❌ **NOT MEASURED** | IQM Resonance pending |

### 14.3 IBM vs IQM — Why Another Platform?

| Blocker | IBM (2025 Q1+) | IQM Resonance |
|---|---|---|
| `qiskit.pulse` / `meas_level=0` | ❌ Removed from production QPUs | ✅ Pulse-level access, raw IQ |
| SamplerV2 limits | ❌ Coherent gates only, no T1/T2 compensation | ✅ Native pulse schedule, Gaussian |
| Cost | Cloud credits | ✅ Starter 30 credits/month free |

**Consequence:** The anchor drive compensation (damping γ>0 noise via resonance pulse)
is **impossible on IBM** with current APIs. The IQM Garnet 20Q (Starter tier) enables
the pulse-level experiment at 0 cost.

### 14.4 Experimental Plan — IQM Garnet 20Q

```python
# anchor_measure_iqm.py (quantum-anchor repo)
DURATION = 37      # ns
AMP = 0.08         # Gaussian amplitude
SIGMA = 10         # ns
FREQ = 4.11e9      # Hz (detuned, not qubit resonance)
BACKEND = "garnet" # 20Q free tier
SHOTS = 1024

# 4 planes → 4 drive channels
for ch in range(4):
    gauss = Gaussian(duration=37, amp=0.08, mu=18.5, sigma=10)
    sched += Play(gauss, DriveChannel(ch))

# Raw IQ measurement (meas_level=0 equivalent)
job = backend.run(qc, shots=1024, use_raw=True)
iq_data = result.get_memory()  # complex IQ vectors!
```

**Expected result:** Balance >97% + Raw IQ not at 0/1 (continuum) → 🔬 HARDWARE PROVEN.

### 14.5 Updated Reproduction Guide (V0.4)

```bash
# SCS classical layer
cd rust && cargo test --release

# SCS quantum bridge (elementary gates)
export IBM_QUANTUM_API_TOKEN='<token>'
python -m python.tkr_ibm zero --qubits 3 --shots 2000
python -m python.tkr_ibm h    --shots 2000
python -m python.tkr_ibm bell --shots 2000
python -m python.tkr_ibm ghz  --shots 2000

# Quantum Anchor Tesseract (IQM - pulse level)
export IQM_TOKEN='<iqm_token>'
python anchor_measure_iqm.py --shots 1024 --backend garnet
```

---

*The source of every hardware-related claim is the measurement performed by
`python -m python.tkr_ibm` (IBM) and `anchor_measure_iqm.py` (IQM). The green
tests establish the correctness of the classical software, not of the hardware
— these two kinds of proof are not interchangeable.*
