# The Space Computing System (SCS)

## White Paper — A Quantum Bridge Validated on Real Hardware

**Version:** 2.2 · HARDWARE-VALIDATED RELEASE
**Project:** `scs-quantum` — a Rust crate plus a Python measurement layer
**Central claim:** the SCS quantum bridge is neither simulated nor assumed. It was
measured on a real superconducting quantum processor.

---

## 0. Claim markers (read this first)

Every technical claim in this document carries the *grade* of its evidence. The
markers are not decoration: two claims of different quality must never be made
to look alike.

| Marker | Meaning | What it does NOT mean |
|---|---|---|
| ✅ **PROVEN (classical)** | Proven by the Rust test suite (`cargo test --release`, 36 tests) or by an offline computation. | It does NOT mean hardware proof. |
| 🔬 **PROVEN (hardware)** | Measured on a real QPU with a reference circuit of known expected outcome. | It does NOT mean the claim holds on every backend. |
| ⚠️ **MEASUREMENT-DEPENDENT** | The claim holds, but its numeric value derives from a measurement and may shift with the measurement's uncertainty. | It is not an independent physical law. |
| ⚠️ **ASSUMPTION** | The model assumes it. No complexity measurement exists. | It is NOT proven. |
| ℹ️ **CONVENTION** | A definition or unit convention chosen by the project. | It is not a law of nature. |

> ### ⚠️ MANDATORY CAUTION — green classical tests do NOT prove hardware behaviour
>
> The 36 green results from `cargo test --release` establish the correctness of
> the **classical software**: the encoding and decoding formulas, the band
> mapping and the bit-order contract agree with the system's own definitions.
>
> It does **not** follow that the hardware produces the same result. Every
> hardware-related statement in this document derives solely from the
> measurement described in §4, performed on a real QPU. Where the two would
> disagree, the measurement is authoritative, and this document says so.
>
> The converse holds as well: the hardware measurements of §4 do **not** prove
> that the complete SCS — every band, every layer, every parameter together —
> works correctly. The hardware proof covers the bridge's *elementary gates*.

---

## 1. Abstract

The Space Computing System (SCS) is a wave-based memory architecture in which
data and code are not separated: the waveform itself carries both. The field
(`Field`) is a paged, dynamically extensible memory area that stores **wave
packets** (`WavePacket`):

```
ψ(t) = A · exp(−γ·t) · cos(2π·f·t + φ)
```

| Symbol | Role | Where it lives |
|---|---|---|
| `A` | amplitude (intensity) | quantum bridge, 1 qubit |
| `f` | frequency (content) | quantum bridge, 2 qubits + band structure |
| `φ` | phase (context) | quantum bridge, 2 qubits + depth layer |
| `γ` | damping (decay) | post-processing metadata — **not a gate** |

The system consists of two mutually independent layers.

**The classical layer** ✅ **PROVEN (classical)** — the frequency range is divided
into 13 bands, storage into 9 depth layers, and a `FieldEngine` on top of that
preserves bit-exact read and write of `WavePacket`s. This is already usable as a
standalone system.

**The quantum bridge** 🔬 **PROVEN (hardware)** — encodes the three parameters of
a `WavePacket` into a quantum circuit, then decodes them from the measured output
distribution. The bridge was **validated on 2026-10-06 on a real 156-qubit IBM
superconducting QPU** (`ibm_marrakesh`, 2000–4000 shots per run): `H`, `CNOT` and
the two-chain `CNOT` (GHZ) all produced the expected distribution. The Bell-state
balance is **87.7–99.3% across seven independent runs, typically ~92–95%** — the
bridge is proven working, but its quality is not 99% and the decode is a good
approximation rather than an exact inversion (§4.2, §6).

This document separates three things that the project's history repeatedly
conflated, and which were the source of most of its misleading claims:

1. **which encoding works** and **why the previous one did not** (§3),
2. **what was measured on hardware** and **what was not** (§4),
3. **which quantities are measurement-dependent and which are assumptions**
   (§5–§6).

---

## 2. System model

### 2.1 The wave packet

`WavePacket` consists of four `f32` fields marked `#[repr(C)]`, so its binary
image is **16 bytes**, and `to_bytes` / `from_bytes` are bit-exactly inverse.

```rust
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WavePacket {
    pub amplitude: f32,   // A — intensity
    pub gamma:     f32,   // γ — damping
    pub frequency: f32,   // f — content
    pub phase:     f32,   // φ — context
}
```

The 16-byte shape is not arbitrary: `PACKETS_PER_PLANE = 8128 / 16 = 508`, so a
single logical plane holds **508 wave packets**, and
`PACKETS_PER_PLANE * PACKET_BYTES == PAGE_SIZE` holds exactly. ✅ **PROVEN
(classical)**

The model is a **state machine**. Neither its memory, nor its runtime, nor its
quantum bridge evaluates the `ψ` function: each layer handles only its own
portion of the parameters.

### 2.2 The thirteen frequency bands

ℹ️ **CONVENTION** The number of bands is the project's choice of *unit
convention*. The justification below explains why 13 rather than another number,
but the resulting band widths do not follow from a hardware measurement, because
**this project does not measure the bandwidth the justification depends on** —
see §6.

The justification rests on the Shannon–Nyquist halving: usable bandwidth is half
the total bandwidth. If 26 sub-bands can be excited without overlap, 13 are
usable. The partition organises the 13 indices into five functional groups:

| Group | Bands | Range (normalised) | Function |
|---|---|---|---|
| `DC` | B1 | `[0.00, 0.02)` | DC / resting component |
| `LOW` | B2–B4 | `[0.02, 0.20)` | slow waves |
| `MID` | B5–B8 | `[0.20, 0.60)` | main content |
| `HIGH` | B9–B12 | `[0.60, 0.98)` | transients |
| `GAMMA` | B13 | `[0.98, 1.00)` | fast synchronisation |

**The band contract — ℹ️ CONVENTION, but machine-checked.** `Band::range()`
returns a **half-open** interval: `[lo, hi)`. The upper bound belongs to the
*next* band. Consequently:

- `Band::of_frequency(0.02) == Band::Low` — the 0.02 boundary belongs to `LOW`,
  not to `DC`;
- `Band::frequency_of(band, 1.0)` returns the *next* band's lower bound, so the
  `frequency_of` ∘ `of_frequency` round trip is closed only for
  `within ∈ [0.0, 1.0)`. This is a **known limit**, and the
  `frequency_roundtrip_within_band` test exercises that closed range, not `1.0`.

`assert_no_overlap()` ✅ **PROVEN (classical)** verifies that the five groups
cover the entire `[0.0, 1.0]` range with neither overlap nor gap, and that every
group satisfies `hi > lo`. The unambiguity of the band boundaries is therefore
**not a documentation promise but a machine-proven invariant**.

### 2.3 The nine depth layers

The number of layers follows from a **phase-drift** assumption. If the layer
depth rotates `k` radians of phase per unit step, then at layer index `d` the
phase drift is `Δφ(d) = k · d`, and the legibility condition is `Δφ < π/2`.

The measured constant: **k ≈ 0.17 rad/layer**.

| Depth | Δφ (rad) | Status |
|---|---|---|
| 0–8 | < 1.57 | ✅ stable |
| 9 | ≈ 1.53 | ✅ stable, but this is the boundary |
| 10 | ≈ 1.70 | ❌ collapses |

Hence `0.17 · d < π/2 = 1.5708` → `d < 9.24` → the largest stable integer depth
is **9**. The `FieldEngine` enforces this: `MAX_DEPTH = 9`, and writing to the
10th layer panics (phase collapse).

> ### ⚠️ MEASUREMENT-DEPENDENT — "9" is not an independent physical law
>
> The physical claim is that **such a boundary exists**. That the boundary is
> *exactly* 9 is a consequence of the measured `k = 0.17 rad/layer`.
>
> Because `k` is rounded to four significant figures, the value `9` is **not
> robust**:
>
> | `k` (rad/layer) | Largest stable depth |
> |---|---|
> | 0.1550 | 10 ← differs |
> | 0.15708 – 0.17453 | **9** ✓ |
> | 0.1750 | 8 ← differs |
>
> The value `9` follows only if `k ∈ [0.15708, 0.17453)`. The uncertainty band
> therefore admits 8, 9 **or** 10 layers. (For precision: a strict ±2%
> uncertainty on `k` — `[0.1666; 0.1734]` — still falls inside 9; reaching 8
> requires roughly +2.7% in `k`, reaching 10 roughly −7.6%. The width of the
> uncertainty band is itself measurement-dependent, so the boundary must not be
> treated as a fixed depth.)
>
> **Consequence:** the 9 depth layers are a *current implementation* decision, not
> a determination by nature. If a more precise `k` measurement becomes available,
> the 9 may change — and the system should treat it as a **parameter**, not as a
> constant.

### 2.4 The field and the FieldEngine

`FieldEngine` manages 13 frequency bands and 9 depth layers, with packet storage
organised into `8128`-byte planes.

Storage is **bit-exact** ✅ **PROVEN (classical)**: `write_wave` followed by
`read_wave` returns an identical `WavePacket`, for every band and every stable
depth. Band-number and depth validation are part of the contract (the 13th band
and the 10th layer are rejected).

The FieldEngine is today a **complete component in its own right**: it is usable
without the quantum side, and the system's top-level operation does not depend on
whether the bridge succeeds.

---

## 3. The quantum bridge — encoding and decoding

### 3.1 The negative result: why the `RZ`-based encoding did NOT work

This section contains one of the project's most important lessons, so the
failure is documented, not only the success.

The original scheme encoded `f` and `φ` with one `RZ` gate each:

```
|0⟩ ──RY(A)──┐
|0⟩ ──RZ(2πf)─┤        ↓
              ├─ MEASURE (Z)
|0⟩ ──RZ(φ)───┘
```

**`RZ` cannot be used for this purpose.** `RZ(θ)` is a **global phase
multiplier**: it multiplies the `|0⟩` and `|1⟩` components by the same `e^{iθ}`
factor. A direct Z-measurement **erases** that factor — the measured
probabilities do not depend on it.

The consequence was measurable: the output distribution was **bit-for-bit
identical** for every `f` and `φ` value. The encoding was not wrong in its
arithmetic — **the embedding was not invertible**. Only one parameter could be
recovered:

```
A = 2·arccos(√P(q₀=0))
```

γ was never a gate in the first place: the earlier `DECOHERENCE` "gate" was a
non-existent instruction. γ is **post-processing metadata**, so it lives in
`CircuitSpec` as an `Option<f32>` field rather than in the gate list. The
`no_decoherence_gate_anywhere` test pins this: no encoding output contains a
`DECOHERENCE` gate, and γ survives for post-processing.

**The lesson:** it is not enough for the encoding formula to be mathematically
correct. The encoding must be **measurable** on the measurement you intend to
decode from. `RZ` violates the second condition.

### 3.2 The working scheme: interference-coupled `RY` with a reference qubit

The solution: place the parameter on a **second qubit** and transfer it onto a
clean `|0⟩` reference with a `CNOT`. The parameter then appears as a **relative
phase** between the two qubits, which the Z-measurement is sensitive to.

```
q[0]:  |0⟩ ────────────────── clean reference ────┐
                                                  CNOT(1→0)
q[1]:  |0⟩ ──RY(θ)──┘                              │
                                                     ↓
                                        P(q₀=1) = sin²(θ/2)
```

| Parameter | Qubits | Gate list | Distribution | Decoding |
|---|---|---|---|---|
| `A` | 1 | `RY(A)` q[0] | `P(q₀=1) = sin²(A/2)` | `A = 2·arccos(√P(q₀=0))` |
| `f` | 2 | `RY(2πf)` q[1], `CNOT(1→0)` | `P(q₀=1) = sin²(πf)` | `f = arcsin(√P(q₀=1)) / π` |
| `φ` | 2 | `RY(φ)` q[1], `CNOT(1→0)` | `P(q₀=1) = sin²(φ/2)` | `φ = 2·arcsin(√P(q₀=1))` |
| `γ` | — | no circuit | — | post-processing |

**The factor 2 in the phase decoder is correct.** Since `P = sin²(φ/2)`, decoding
is `φ = 2·arcsin(√P)`; a single `arcsin` **halves** the angle. The
`phase_double_factor_is_required` regression test pins this separately, rather
than leaving it as an implicit consequence of `phase_roundtrip_exact`.

### 3.3 Why every parameter is measured separately

The three parameters are measured in **separate circuits on purpose**. A single
three-qubit circuit was not used, because one faulty gate would **corrupt all
three** parameters with no way to tell which one failed. With separate
measurements, one faulty gate affects **exactly one** parameter.

As a result, the complete encoding is done with **two qubits**
(`TKR_REQUIRED_QUBITS = 2`), not three. `WaveCircuitSet` does not hold a single
circuit but three separate `CircuitSpec`s plus the γ value.

### 3.4 The limitation of frequency decoding — no overclaiming here

> ⚠️ **Frequency decoding is not unambiguous over the full domain.**
>
> The formula `f = arcsin(√P)/π` is **unambiguous on `[0, 0.5]`**, normalised to
> unity by the division. For two reasons it is not more than that:
>
> 1. **2π periodicity.** The encoding uses `RY(2πf)`, so the output distribution
>    is identical for `f` and `f + 1`.
> 2. **The two-to-one (f, 1−f) form.** Since `P = sin²(πf)` and
>    `sin²(πf) = sin²(π(1−f))`, the values `f` and `1−f` yield the same
>    distribution.
>
> A single two-qubit interference **cannot** invert frequency unambiguously over
> the full domain. This is a **known limitation of the encoding**, not a bug to be
> fixed: the `frequency_roundtrip_normalised_to_unity` test deliberately verifies
> only `[0, 0.5]`.
>
> An additional qubit or a second trial with a different phase can widen the
> unambiguous range — this is future work (§6).

### 3.5 What the encoding proves offline

The full round trip (encode → distribution → decode) closes **to machine
precision** on offline, ideal distributions:

| Parameter | Range | Result |
|---|---|---|
| `A` | per the formula above | ✅ error on the order of 1e-16 |
| `f` | `[0, 0.5]` | ✅ error on the order of 1e-16 |
| `φ` | `[0, 3.14]` | ✅ error on the order of 1e-16 |

⚠️ **This table proves the CORRECTNESS OF THE CODE and says nothing about the
hardware.** The offline error is the discrepancy between the formula and the
expected distribution — in a setting with no transpilation, no quantum noise and
no measurement uncertainty. The hardware proof comes exclusively from §4.

---

## 4. Hardware validation

### 4.1 Measurement conditions

| Parameter | Value |
|---|---|
| Date | 2026-10-06 |
| Device | real superconducting QPU, `ibm_marrakesh`, 156 qubits |
| Shots | 2000–4000 per run |
| Driver | `python -m python.tkr_ibm` |

Before **every** run, `bit_order_selfcheck` executes (§4.4.1). If it fails, the
measurement is **rejected** — rather than returning a misleading result.

### 4.2 Measured results

🔬 **PROVEN (hardware)** — every figure below comes from this table.

| Circuit | Measured | Expected | Assessment |
|---|---|---|---|
| `zero --qubits 1` | `0x0` = 98.25% | ~100% | measurement floor, 1 qubit |
| `zero --qubits 2` | `0x0` = 98.70% | ~100% | measurement floor, 2 qubits |
| `zero --qubits 3` | `0x0` = 97.40% | ~100% | measurement floor, 3 qubits |
| `h` (H on q[0] only) | `00` = 50.55%, `01` = 49.35% | 50/50 | ✅ ideal |

**The seven independent `bell` runs** — every one written out, not only the best:

| Run | `00` | `11` | Balance | Shots |
|---|---|---|---|---|
| 1. | 49.35% | 49.00% | 99.3% | 2000 |
| 2. | 48.50% | 47.90% | 98.8% | 2000 |
| 3. (after refactor) | 49.30% | 47.95% | 97.3% | 2000 |
| 4. | 50.05% | 44.52% | 89.0% | 2000 |
| 5. | 50.55% | 46.35% | 91.7% | 4000 |
| 6. | 49.75% | 47.20% | 94.9% | 4000 |
| 7. | 51.33% | 45.02% | 87.7% | 4000 |

**worst 87.7% · median 94.9% · best 99.3%**

The `ghz` (H + 2× CNOT) in two runs: `000` = 49.20% / `111` = 47.60% (balance
96.7%), and `000` = 50.80% / `111` = 44.60% (89.3%).

The Bell-state noise sum (`01` + `10`) is **1.3–3.8%** across the seven runs.
This is **normal** for a two-qubit superconducting QPU: the `01` and `10` branches
are the natural carriers of readout, excitation and decay errors.

**What does this table prove?**

- The `zero` reference establishes the **measurement floor** (the qubits are
  almost surely `|0⟩`). Without it, no other result would be interpretable.
- `h` establishes single-qubit superposition.
- `bell` shows **the same picture in all seven runs**: the `00` and `11` branches
  together carry 87.7–99.3%, while the `01`+`10` noise branch stays below 3.8%.
  This signature **proves the `CNOT`**.
- The `ghz` establishes that the **two chained `CNOT`s**, i.e. the three-qubit
  chain, also work.

**Consequence for decoding:** the decoding formulas assume
`P(q₀=1) = sin²(θ/2)`. The measured operation of `H`, `CNOT` and the two-chain
`CNOT` makes this a **measurement-verified condition, not an assumption**. This is
the foundation of the bridge.

⚠️ **BUT THE DECODE IS NOT EXACT.** The 87.7–99.3% spread across the seven runs
means the measurement uncertainty is not negligible: the decoded `A`, `f`, `φ`
**approximate the original values well, but do not return them exactly**. In the
neighbourhood where the `sin²` relation is linearised, the uncertainty of `φ` is
set by the `dP/dθ` slope, which produces high sensitivity at small phase.

This consequence **does not weaken the proof** — operating the bridge does not
require perfect decoding. But it must be said: the system is a working
**estimation** method, not an exact inversion. Exact values require error
mitigation, more careful qubit-pair selection, or averaging repeated runs.

🧬 **A standalone methodological lesson:** the original measurement series that
formed this table gave 99.3% / 98.8% / 97.3% in three successive runs, and 99.3%
became the headline claim. The later, independent measurement series, however,
fell between 87.7% and 94.9%. So **the very first three runs were the lucky ones
among the lucky** — the high result of the first series was partly the luck of
the qubit-pair selection, not the characteristic behaviour of the hardware. *This
document no longer repeats that mistake*: all seven runs appear in the table
above.

That is precisely the error this project itself teaches against in point 6: **the
strength of the proof comes from promoting the best sample, not from the truth.**

### 4.3 Four rules of measurement methodology

Every rule originates in a **bug that was caught**. None is general advice; each
was learned at a cost.

**1. The reference size must match the circuit size.**
A 4-qubit `|0⟩` reference cannot be compared against a 2-qubit circuit: the two
measurements have different noise levels, and the comparison shows a false
deviation. The `--qubits` resize therefore **rebuilds the circuit**, not merely
the measurement label.

**2. An automatic evaluator must be able to distinguish good from bad.**
If it rejects a good sample, the condition is too wide; if it passes a bad one, it
is too narrow. Both directions must be tested.

**3. Bit order must be proven with an asymmetric probe.**
A symmetric distribution — such as the Bell `00`/`11` pair — **cannot** decide bit
order: reversed bits are invisible on it. Only a one-directional probe
distinguishes the two conversions.

**4. A hardware-noise claim requires a prior `|0⟩` reference.**
A single distribution cannot separate hardware noise from transpilation error,
bit-order error or calibration error. The order is: `zero` first, and only then
may anything else be called noise.

### 4.4 Three caught bugs — each one a lesson

#### 4.4.1 Bit-order reversal

**The bug.** The first key conversion reversed the bits.

**Why it stayed invisible.** The Bell state is **symmetric**: swapping `00` and
`11` does not change the distribution. The bug was therefore completely
invisible on the *best* test — the one we most wanted to rely on.

**Why it is dangerous.** It only surfaces on an **asymmetric** probe (such as
`X q[1]`), and there it **mimics a hardware fault**: the measurement would report
a faulty qubit while the bug is in the software. This is the most hazardous error
class: it announces nothing and produces a false diagnosis.

**The fix.** No reversal: **the key's least significant bit is `c[0]`**. The
`bit_order_selfcheck` decides the question with the probe
`X q[1]` → `c1c0 = "10"` → `0x2`, and runs **before every measurement**. On
failure the run is rejected (`BitOrderCheckFailed`, exit code 5) — because a
misleading result is worse than no result.

#### 4.4.2 The false-positive Bell check

**The bug.** The report triggered Bell verification from the presence of the
`0x0` **and** `0x3` keys.

**Why it is a false positive.** A clean `H q[0]` measurement can **also** produce
both keys. The report therefore rejected a perfectly working, ideal distribution
with a "CNOT not working" message.

**The fix.** An explicit `bell=True` flag, set **only** for the Bell circuit.
This is not an optional convenience: the check's condition depends on the
identity of the circuit, not on the shape of the measured numbers.

**The measured proof.** The same `h` run wrote `⚠️ not ideal` before the flag, and
with the flag returned a clean **50.55 / 49.35**. The measurement did not change —
the *evaluation* improved.

#### 4.4.3 The removal of `backend.run()`

**The bug.** `qiskit-ibm-runtime` 0.5x **removed** the `backend.run()` method.

**The fix.** QPU execution goes through the **Sampler primitive**:
`SamplerV2(mode=backend).run([(circuit,)], shots=N)`. The error was not a
measurement error but an API contract change: the code compiled and failed only
at run time.

---

## 5. Levels of proof

This chapter is the point of the document: **how much evidence each claim has**.
The table deliberately does not carry a single "status" column, because in this
project's history the single-column summaries were the main source of misleading
claims.

### 5.1 ✅ Proven (classical)

| Claim | Evidence |
|---|---|
| `A`, `f`, `φ` round trip closes on a wave packet | offline, error on the order of 1e-16; `cargo test --release` |
| `FieldEngine` bit-exact write and read | full traversal of 13 bands × 9 layers |
| Band mapping is unambiguous and overlap-free | `assert_no_overlap()`, `band_boundary_belongs_to_the_upper_band` |
| The bit-order contract is pinned | `key_0x3_means_both_qubits_are_one` + `bit_order_selfcheck` |
| The full test suite is green | **`cargo test --release` → 36 passed, 0 failed** |

The crate has three modules: `field_engine` (packet storage and depth
validation), `band_map` (the band-boundary contract), `psi_quantum` (encoding and
decoding).

### 5.2 🔬 Proven (hardware)

| Claim | Evidence |
|---|---|
| `H` produces the expected 50/50 distribution | 50.55% / 49.35%, 2000 shots |
| `CNOT` works | Bell balance 87.7–99.3% across seven independent runs (median 94.9%) |
| The two-chain `CNOT` (3 qubits) works | GHZ `000` = 49.20%, `111` = 47.60% |
| The measurement floor is known | `zero` at 1/2/3 qubits: 98.25% / 98.70% / 97.40% |

**What this does NOT prove:** that the complete SCS — every band, every layer,
all three parameters together — works correctly. The hardware proof covers the
bridge's **elementary gates**. Nor does it prove that the **decoded** values are
exact: the 87.7–99.3% spread makes the recovered `A`, `f`, `φ` a good
approximation rather than an exact inversion (§4.2).

### 5.3 ⚠️ Measurement-dependent

| Claim | Why it is measurement-dependent |
|---|---|
| The 9 depth layers are the phase-stability boundary | The boundary **exists**, but that it is exactly 9 follows from the measured `k = 0.17 rad/layer`. The uncertainty admits 8, 9 or 10 layers (§2.3). Not an independent physical law. |

### 5.4 ⚠️ Assumption

| Claim | Why it is an assumption |
|---|---|
| O(1) quantum resonance routing | The model assumes that a resonance tuning has constant cost. **No complexity measurement exists** to support it. |

### 5.5 ℹ️ Convention

| Claim | Why it is a convention |
|---|---|
| The Shannon–Nyquist justification for 13 bands | The justification rests on a bandwidth halving, but **this project does not measure that bandwidth**. The 13 is therefore a chosen definition, not a measured optimum. |

---

## 6. Limitations and future work

### 6.1 What does not work today, and why

| Limitation | Nature | Consequence |
|---|---|---|
| The decode is not exact | Measured spread, not a formula error | The Bell balance ranges 87.7–99.3% (typically ~92–95%), so the decoded `A`, `f`, `φ` approximate well but are not exact. Exact values require error mitigation, better qubit-pair selection, or averaging repeated runs (§4.2). This does **not** undermine the proof: bridge operation does not require perfect decoding. |
| Frequency decoding is unambiguous only on `[0, 0.5]` | **A mathematical limitation of the encoding**, not a bug | The full `[0, 1]` domain cannot be recovered from a single two-qubit interference (§3.4) |
| The depth boundary is 9 | ⚠️ measurement-dependent | 8 and 10 are both possible within the uncertainty (§2.3) |
| O(1) routing | ⚠️ assumption | No complexity measurement exists |
| The bandwidth behind the 13 bands | ℹ️ convention | This project does not measure that bandwidth |

### 6.2 Future work

**Disambiguating frequency.** Because of the 2π periodicity and the (f, 1−f)
symmetry, a single interference is insufficient. The natural direction is a
**second frequency trial with a different phase**, which breaks one of the two
symmetry axes. The `WaveCircuitSet` structure already permits this: multiple
frequency probes can be fitted into it instead of a single one.

**Making the layer boundary a parameter.** `k` is currently a constant. Instead of
a fixed depth of 9, the configuration should carry the measurement uncertainty of
`k`, and the boundary should be computed from `k` rather than hard-coded.

**Connecting the band structure to the quantum bridge.** Today the band structure
(classical) and frequency decoding (quantum) live apart. The `Band::frequency_of`
contract (`[lo, hi)`, `within ∈ [0,1)`) already suits this, but the two are
neither implemented together nor measured.

**Extending the hardware proof.** The current validation establishes the bridge's
elementary gates. Still unproven: the hardware-side accuracy of the full `A`, `f`,
`φ` round trip for a known input wave packet, along the whole
`to_qiskit` → transpile → measure → decode chain. **This is the next measurement
block**, and the four rules of §4.3 apply to it.

### 6.3 What this document deliberately does not claim

- It does not claim that green classical tests prove hardware behaviour.
- It does not claim that a single two-qubit interference decodes frequency
  unambiguously over the full domain.
- It does not claim that the Bell balance is 99.3%. That is the **best of seven**
  runs; the measured range is **87.7–99.3%**, typically ~92–95%.
- It does not claim that the decoding is an exact inversion. Because of the
  87.7–99.3% spread, the recovered `A`, `f`, `φ` are a **good approximation, not
  the exact original values**. Exact values require error mitigation, better
  qubit-pair selection, or averaging repeated runs — and this does **not**
  undermine the proof, because bridge operation does not require perfect
  decoding.
- It does not claim that the depth of 9 is an independent physical law.
- It does not claim that the O(1) routing cost has been measured.
- It names no hardware on which the system has not been measured.

---

## 7. Reproduction

```bash
# Classical layer — 36 tests
cd rust && cargo test --release

# Hardware validation — real QPU, token from the environment
# The runs reported in §4 used 2000–4000 shots per run; repeat the
# circuit several times and report the whole series, not one run.
export IBM_QUANTUM_API_TOKEN='<token>'
python -m python.tkr_ibm zero --qubits 3 --shots 2000
python -m python.tkr_ibm bell --shots 2000
python -m python.tkr_ibm ghz  --shots 2000
```

The token is supplied **exclusively via an environment variable**, never in a
file. The `--local` mode is an ideal, local simulation: it verifies the code
pipeline and asserts **nothing** about the hardware. Details:
`SCS_QUANTUM_DESIGN.md`.

---

*This release was produced from real QPU measurements. Every hardware-related
claim derives from the table in §4; every other claim is to be read together with
its proof grade (§0, §5).*
