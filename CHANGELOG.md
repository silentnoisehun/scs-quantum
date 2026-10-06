# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

The Rust crate (`rust/Cargo.toml`) and the Python distribution (`pyproject.toml`)
are released together and always carry the same version number.

## [Unreleased]

### Added

- English versions of both documents: `docs/TKR_WHITE_PAPER_v2.en.md` and
  `docs/TKR_QUANTUM_DESIGN.en.md`, plus `README.en.md`. The Hungarian originals
  are unchanged in substance; the two languages carry the same proof grades.
- `RELEASE.md`: the ordered procedure for obtaining a DOI, including the Zenodo
  concept-DOI vs version-DOI distinction and the arXiv identifier relationship.
- `config/.env.template`: the placeholder environment template that
  `.gitignore` and `MANIFEST.in` already referenced but which did not exist.
- CI (`.github/workflows/ci.yml`): Rust tests, a stdlib-only import check of the
  measurement layer, the proof suite, an ideal-simulation pipeline check, and a
  secrets guard that fails if a real `.env` or a non-placeholder token is tracked.
- Release automation (`.github/workflows/release.yml`): verifies the classical
  proof, checks the sdist actually contains the Rust sources and the paper, then
  publishes to PyPI and crates.io and creates the GitHub Release.

### Changed

- README: the measured-results table now lists the Bell reproducibility runs
  (99.3% / 98.8% / 97.3%) separately. The GHZ row previously carried a computed
  balance figure that is not part of the measured record; it now reports the raw
  measurement and the measured noise, consistent with the white paper.
- `pyproject.toml`: package URLs point at the public repository instead of a
  placeholder; the English README is the PyPI long description.

## [0.2.0] - 2026-10-06

The release that turns TKR from a design into a measured claim: the quantum
bridge was validated on real superconducting hardware, and every piece of
measurement code was made hardware-independent.

### Added

- **Hardware-validated quantum bridge.** A 2-qubit Bell state reached 99.3%
  balance (`00`: 49.35%, `11`: 49.00%) and a 3-qubit GHZ state came out ideal
  (`000`: 49.20%, `111`: 47.60%), measured on a real 156-qubit superconducting
  QPU — IBM `ibm_marrakesh`, 2000 shots. Reproducibility check on the Bell
  circuit: 98.8% balance.
- `python/tkr_measure.py`: hardware-independent measurement layer —
  `WavePacket`, `CircuitSpec`, `Gate`, `reference_circuits()`,
  `resize_reference()`, `counts_to_keys()`, `bit_order_selfcheck()` and
  `report_counts()`. Imports the standard library only, so it stays usable
  without qiskit installed.
- `python/tkr_ibm.py`: IBM Quantum backend plus CLI (`python -m python.tkr_ibm`,
  console script `tkr-qpu`), including a `--local` ideal-simulation mode that
  requires no token and is labelled as *not* QPU validation.
- Interference-coupled `RY` + `CNOT` encoding circuit, replacing the previous
  `RZ`-based scheme, which did not produce usable interference.

### Changed

- Rust crate refactored to be hardware-independent: the vendor HTTP client,
  the device-specific constants and the OriginIR text emitter were removed.
  Dependencies dropped from 4 to 2 (`anyhow`, `serde`); test count 33 → 36.
- Every measurement run now performs a bit-order self-check before submitting
  the circuit to hardware, so a mis-ordered result can no longer be mistaken
  for a hardware fault.
- `python/tkr_ibm.py` moved to the `SamplerV2` primitive (see Fixed).

### Removed

- The single-vendor backend module and its probe, batch and interference
  scripts. The measurement layer is now backend-neutral.
- `to_originir` text emitter.
- The `DECOHERENCE` pseudo-gate. Gamma is post-processing metadata describing
  a wave packet, not a gate that belongs in a circuit.

### Fixed

Three bugs found by measurement, not by reading the code:

- **Bit-order conversion reversed the bits.** The Bell state is symmetric
  (`00` ↔ `11`), so the error was invisible on the main test and only surfaced
  on an asymmetric probe circuit. Left unfixed it would have mimicked a
  hardware fault and sent debugging down the wrong path entirely.
- **The Bell report keyed off the presence of `0x0`/`0x3`.** That is a false
  positive: it flagged a perfectly working plain `H q[0]` measurement as
  "CNOT not working".
- **`backend.run()` was removed upstream** in qiskit-ibm-runtime 0.5x. Switched
  to the `SamplerV2` primitive.

### Security

- API tokens are read from environment variables only
  (`IBM_QUANTUM_API_TOKEN`, or the `QISKIT_IBM_TOKEN` alternative; optional CRN
  via `IBM_QUANTUM_INSTANCE` / `QISKIT_IBM_INSTANCE`). This package never
  writes a token to disk, and `.env` files are excluded from the repository and
  from the source distribution.

[Unreleased]: https://github.com/silentnoisehun/tkr-quantum/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/silentnoisehun/tkr-quantum/releases/tag/v0.2.0