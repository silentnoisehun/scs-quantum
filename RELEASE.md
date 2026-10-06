# RELEASE.md — how SCS gets a DOI

**Read this before tagging a release.** The order matters: Zenodo mints the DOI,
and arXiv mints an identifier that should be cited *in* the Zenodo record, so the
two interact.

---

## 0. The one thing to get right first

> ⚠️ **The author list is not a formality.**
>
> A DOI is **permanent and public**. Zenodo mints it for *specific named people*,
> and it cannot be quietly reassigned later. Before any of the steps below,
> decide:
>
> - Who is on the author list, spelled exactly as it should appear forever?
> - Which affiliation string goes with each name?
> - Does every listed person **agree** to be named? (Being named on a DOI is a
>   public, citable attribution, not a private favour.)

The author in `CITATION.cff` is `Máté Róbert`, confirmed by the project owner.
Anyone added to that list later must **agree** to be named: a DOI is a public,
citable attribution, not a private favour. The repository URL is no longer a
placeholder — `https://github.com/silentnoisehun/scs-quantum` is the real public
location, so confirm it resolves before the Zenodo upload.

---

## 1. Preconditions

| Requirement | Why |
|---|---|
| A clean tag (`v0.2.0` or later) | Zenodo and arXiv both bind to a version |
| Real author names agreed (§0) | A DOI is permanent |
| A real public repository URL | `CITATION.cff`, `pyproject.toml` and the README all point at it |
| Green CI | The release workflow refuses to publish otherwise |
| An ORCID per author | Optional but strongly recommended — it disambiguates names |

---

## 2. Set the real URLs first

Three files currently hold `https://github.com/silentnoisehun/scs-quantum` placeholders.
Replace them **before** publishing:

| File | Field |
|---|---|
| `CITATION.cff` | `repository-code`, `authors[].website` |
| `pyproject.toml` | `[project.urls] Homepage`, `Repository` |
| `README.md` | the `git clone <repo-url>` line |
| `CHANGELOG.md` | the `[Unreleased]` and `[0.2.0]` link definitions at the bottom |

The intended repository is `https://github.com/silentnoisehun/scs-quantum`.

---

## 3. GitHub release

```powershell
# 1. Make sure everything is committed and green
cd C:\path\to\scs_quantum
git tag -a v0.2.0 -m "SCS 0.2.0 — hardware-validated quantum bridge"
git push origin v0.2.0
```

Pushing the tag triggers `.github/workflows/release.yml`, which:

1. runs the 36 Rust tests and the proof suite,
2. builds the sdist + wheel and checks the sdist actually contains the Rust
   sources and the white paper,
3. publishes to PyPI and crates.io,
4. creates the GitHub Release with the sdist and wheel attached.

### 3.1 One-time PyPI setup: trusted publishing

The workflow uses **PyPI trusted publishing (OIDC)**, not an API token. This must
be configured once on PyPI, otherwise the publish step fails with
`missing or insufficient OIDC token permissions`.

1. Create the project on <https://pypi.org/manage/account/publishing/> (or
   claim the name `scs-quantum` if it is available).
2. Add a **pending publisher** on the project page:
   - **Owner:** `silentnoisehun`
   - **Repository:** `scs-quantum`
   - **Workflow name:** `release.yml`
   - **Environment:** *(leave empty)*

No `PYPI_API_TOKEN` secret is needed, and none should be created — OIDC means
GitHub proves the identity to PyPI cryptographically, so there is no long-lived
credential in the repository at all.

### 3.2 One-time crates.io setup

Create an API token at <https://crates.io/settings/tokens> and add it as the
repository secret `CARGO_REGISTRY_TOKEN`. A token **is** required here, because
crates.io has no equivalent of OIDC trusted publishing.

> 🧬 **What the release workflow does NOT do:** it does not run a QPU measurement.
> That requires your own IBM token and is done by hand
> (`docs/SCS_WHITE_PAPER.md` §4). The release archives the *classical* proof;
> the hardware proof lives in the paper and in the Zenodo record.

---

## 4. Zenodo — this is what mints the DOI

### 4.1 Connect the repository (once)

1. Go to <https://zenodo.org> → sign in with your GitHub account.
2. **New upload** → *GitHub* tab.
3. Pick `silentnoisehun/scs-quantum`.
4. Enable **"Enable repository"** → Zenodo now snapshots every release you push.

### 4.2 Configure the metadata template

Zenodo reads `CITATION.cff` from your repository root, so fixing that file (§2)
is what populates the Zenodo form. Check that:

- the title reads well as a standalone title,
- the abstract is self-contained,
- the author list is correct and agreed (§0),
- the licence is MIT,
- the **version** matches the tag you pushed.

### 4.3 Upload the artefacts

Upload the artefacts built by the release workflow, or build them locally:

```powershell
pip install build
python -m build
```

Upload at minimum:

- `dist/scs_quantum-0.2.0-py3-none-any.whl` (the installable package)
- `dist/scs_quantum-0.2.0.tar.gz` (the sdist, which contains the Rust crate)

Optionally also:

- `docs/SCS_WHITE_PAPER.md` and `docs/SCS_WHITE_PAPER.en.md` as
  supplementary files,
- a tarball of the exact tagged source tree.

### 4.4 Add the measurement provenance as a description

This is the part reviewers and readers care about. Paste, into the Zenodo
**description** field, the measured table from the white paper §4.2 — including
the device name, the qubit count, the shot count and the date:

> Measured on `ibm_marrakesh`, a 156-qubit superconducting QPU, 2000 shots per
> run, 2026-10-06. Bell balance 99.3% (run 1), 98.8% (run 2), 97.3% (after
> refactor). GHZ `000` = 49.20%, `111` = 47.60%, noise 1.80%. Measurement floor
> from the `|0⟩` reference: 98.25% / 98.70% / 97.40% at 1 / 2 / 3 qubits.

### 4.5 Publish → mint the DOI

**Publish** the record. Zenodo assigns a DOI of the form
`10.5281/zenodo.<number>`.

> ⚠️ **Concept DOI vs version DOI.** Zenodo mints two kinds:
> - the **version DOI** (e.g. `10.5281/zenodo.1234567`) for this exact upload —
>   use this one for "the version I used";
> - the **concept DOI** (e.g. `10.5281/zenodo.1234568`) covering all versions —
>   use this one in the paper and the README, because it always resolves to the
>   *latest* version.
>
> Put the **concept DOI** in the README, `CITATION.cff` and the paper.

---

## 5. arXiv

### 5.1 Submission requirements

arXiv wants **LaTeX**, not Markdown. Two paths:

| Path | When |
|---|---|
| Convert with `pandoc` | acceptable for a first submission; arXiv accepts PDF-only submissions |
| Hand-write LaTeX | better control over the reference list and formatting |

Minimal conversion:

```powershell
pandoc docs/SCS_WHITE_PAPER.en.md -o arxiv/scs-quantum.tex --standalone
```

> ⚠️ **arXiv requires at least one author with an institutional email address.**
> A generic `@gmail.com` address is accepted by arXiv but reads as unusual for a
> preprint; use an institutional address if you have one.

### 5.2 Category

`quant-ph` is the primary category. Secondary candidates:

- `cs.DC` (data structures — the band/depth lattice is a data-structure claim)
- `physics.comp-ph` (computational physics)

### 5.3 What to declare

Because the paper reports hardware measurements, the abstract must make the
provenance unambiguous: which device, how many shots, which date. Do **not** let
a reader infer the numbers came from simulation. The white paper's §0 and §5
structure exists precisely so this cannot be misread, and that structure should
survive into the arXiv version.

### 5.4 The DOI/identifier relationship

arXiv assigns an identifier like `arXiv:2510.xxxxx`. It is **not** a DOI. The
usual practice:

- cite the **arXiv identifier** in the Zenodo record and in later work;
- cite the **Zenodo DOI** for the software artefact;
- a **DOI for the arXiv paper** only appears once you use the arXiv–DOI
  integration, and it is minted *later* than the arXiv identifier.

So: submit to arXiv first, then add the arXiv identifier to the already-published
Zenodo record. Zenodo lets you edit a published record's description.

---

## 6. Order of operations (summary)

```
1. fix CITATION.cff + pyproject.toml URLs   (§2)
2. get author agreement on names           (§0)
3. push tag v0.2.0 → CI verifies + publishes (§3)
4. Zenodo: enable repo, upload artefacts, paste provenance, publish  (§4)
   → CONCEPT DOI minted
5. put the concept DOI in README + CITATION.cff  (and commit)
6. submit to arXiv                          (§5)
7. add the arXiv identifier back to the Zenodo record
```

---

## 7. Final checklist before anything is public

- [ ] `git grep -i` finds no credential, CRN or instance ID
- [ ] no `example.invalid` placeholder survives in a published file
- [ ] every author agreed to be named on the DOI
- [ ] the abstract distinguishes classical proof from hardware proof
- [ ] the hardware table states device, shots and date
- [ ] `--local` output is nowhere presented as hardware evidence
- [ ] CI is green on the tagged commit

---

*`cargo test --release` → 36 passed, 0 failed is the **classical** proof. It is
archived by the release workflow. The **hardware** proof is the QPU measurement in
`docs/SCS_WHITE_PAPER.md` §4, recorded in the Zenodo provenance. The two are
never the same thing.*
