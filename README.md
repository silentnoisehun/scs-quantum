# SCS — Space Computing System

Frekvencialapú hullámcsomag-kódolás, **valódi kvantumhardveren igazolt** kvantumhíddal.

> 🧬 **A rendszer 2 és 3 qubites kvantumhídja hardveresen bizonyított.**
> Valódi 156 qubites szupravezető QPU-n mérve: a Bell-állapot egyensúlya
> **87,7–99,3%** hét független futásból, **jellemzően ~92–95%**
> (IBM `ibm_marrakesh`, 2026.10.06).
>
> ⚠️ A híd **működik**, de a minősége nem 99%: a visszafejtés
> jól közelít, nem egzakt. Lásd [Korlátok](#korlátok--amit-a-rendszer-nem-bizonyít).

**English:** [README in English](README.en.md) ·
**Dokumentumok:** [White Paper (HU)](docs/SCS_WHITE_PAPER.md) ·
[White Paper (EN)](docs/SCS_WHITE_PAPER.en.md) ·
[Mérnöki terv (HU)](docs/SCS_QUANTUM_DESIGN.md) ·
[Engineering Design (EN)](docs/SCS_QUANTUM_DESIGN.en.md)

---

## Mi ez a rendszer

Az SCS egy hullámcsomagot

```
ψ(t) = A · exp(−γ·t) · cos(2π·f·t + φ)
```

négy paraméterre bontja — **A** amplitúdó (intenzitás), **γ** csillapítás,
**f** frekvencia (tartalom), **φ** fázis (kontextus) — és ezeket egy
**kvantumáramkörbe** kódolja úgy, hogy a mért kimeneti eloszlásból
visszafejthetők legyenek.

A frekvencialapú megközelítés az állapotot nem `N` kvantumban, hanem
**13 frekvenciasáv × 9 mélységi réteg** szerkezetben tárolja.

## Gyors indítás

```bash
git clone https://github.com/silentnoisehun/scs-quantum
cd scs-quantum

# 1. A klasszikus réteg ellenőrzése (nincs semmi telepítendő)
cd rust && cargo test --release && cd ..
python -m proofs.tkr_proofs

# 2. A mérési kód ellenőrzése IDEÁLIS szimulátoron (token NEM kell)
python -m python.tkr_ibm bell --local --shots 2000

# 3. A hardveres bizonyíték újrafuttatása (token kell)
pip install "scs-quantum[qpu]"
export IBM_QUANTUM_API_TOKEN="..."
python -m python.tkr_ibm devices
python -m python.tkr_ibm bell --backend ibm_marrakesh --shots 2000
```

## A mért hardveres eredmények

`ibm_marrakesh`, 156 qubit, valódi szupravezető QPU, 2000–4000 shots/db.

| Áramkör | Mért | Egyensúly |
|---|---|---|
| `zero --qubits 1` | `0x0` = 98,25% | mérési alap |
| `zero --qubits 2` | `0x0` = 98,70% | mérési alap |
| `zero --qubits 3` | `0x0` = 97,40% | mérési alap |
| `h` | `00` = 50,55%, `01` = 49,35% | 50/50 kontroll |

### Bell — mind a hét futás, nem csak a legjobb

| Futás | `00` | `11` | Egyensúly | Shots |
|---|---|---|---|---|
| 1. | 49,35% | 49,00% | 99,3% | 2000 |
| 2. | 48,50% | 47,90% | 98,8% | 2000 |
| 3. (refaktor után) | 49,30% | 47,95% | 97,3% | 2000 |
| 4. | 50,05% | 44,52% | 89,0% | 2000 |
| 5. | 50,55% | 46,35% | 91,7% | 4000 |
| 6. | 49,75% | 47,20% | 94,9% | 4000 |
| 7. | 51,33% | 45,02% | 87,7% | 4000 |

**legrosszabb 87,7% · jellemző (medián) 94,9% · legjobb 99,3%**

### GHZ

| Futás | `000` | `111` | Egyensúly |
|---|---|---|---|
| korábbi | 49,20% | 47,60% | 96,7% |
| későbbi | 50,80% | 44,60% | 89,3% |

🧬 **Miért publikálunk mind a hét futást?** Mert a 99,3% a legjobb
megfigyelt érték, nem a jellemző. Ha csak a legjobbat írjuk ki, a
rendszer sokkal jobbnak látszik, mint amit a mérés indokol — és pont
ezt a hibát követte el az a naiv bizonyítás, amely egyetlen sikeres
mérésből általánosít. A híd **működik** (minden futásban a 00/11 ág
dominál), de a minősége nem 99%.

**Ebből következik**, hogy a dekódolási képletek `P(q0=1) = sin²(θ/2)`
feltevése nem feltételezés, hanem méréssel igazolt állítás — de a
mérési bizonytalanság (87,7–99,3%) miatt a **visszafejtett érték
pontosságára** külön becslés szükséges.

## 🧬 Két bizonyítási szint — ne keverd össze őket

| Szint | Mi bizonyítja | Parancs |
|---|---|---|
| **Klasszikus** | a KÓD helyességét | `cargo test --release` → 36 teszt |
| **Hardveres** | a KVANTUMHÍD működését | `python -m python.tkr_ibm bell --backend …` |

**A zöld tesztek NEM bizonyítják a hardver működését.** A kizárólagos
hardveres bizonyíték az 5. bizonyítási pont. A `--local` és a
`--simulator` mód NEM QPU-bizonyíték — ezek a mérési kódot ellenőrzik.

## A kódolási séma

Minden paraméter **külön áramkörben** mérhető. Ez tudatos: egyetlen nagy
áramkörben egyetlen hibás kapu mindhárom paramétert eltorzítaná, és nem
lenne megkülönböztethető, melyik hibázott.

| Paraméter | Kvantum | Áramkör | Visszafejtés |
|---|---|---|---|
| `A` | 1 | `RY(A)` q[0] | `A = 2·arccos(√P(q0=0))` |
| `f` | 2 | `RY(2πf)` q[1] + `CNOT q1→q0` | `f = arcsin(√P(q0=1)) / π` |
| `φ` | 2 | `RY(φ)` q[1] + `CNOT q1→q0` | `φ = 2·arcsin(√P(q0=1))` |
| `γ` | — | nincs — posztprocesszálás | — |

A q[0] tiszta `|0⟩` marad referenciakvantumnak.

**A korábbi `RZ`-alapú séma nem működött.** Az `RZ` globális fázisszorzó,
amit a közvetlen Z-mérés kitöröl — az eredménypeloszlás minden `f` és `φ`
értékre azonos maradt. A `RY`-alapú interferencia-séma ezt megoldja.

## Csomagszerkezet

```
rust/                  a klasszikus tér-motor és a kódolási híd
  src/field_engine.rs    13 sáv × 9 réteg hullámtár, bitexakt írás/olvasás
  src/band_map.rs        sávhatár-szerződés: [lo, hi), a felső a következő sávé
  src/psi_quantum.rs     WavePacket ↔ kvantumáramkör, hardverfüggetlen
python/
  tkr_measure.py         mérési réteg (csak stdlib!) — WavePacket, CircuitSpec,
                         referencia-áramkörök, bit-sorrend-ellenőrzés, riport
  tkr_ibm.py             IBM Quantum backend + CLI
proofs/
  tkr_proofs.py          a 8 bizonyítási pont, mindegyik státusszal
docs/
  SCS_WHITE_PAPER.md        white paper (magyar)
  SCS_WHITE_PAPER.en.md     white paper (English)
  SCS_QUANTUM_DESIGN.md     mérnöki dokumentum (magyar)
  SCS_QUANTUM_DESIGN.en.md  engineering design (English)
config/.env.template     környezeti változók helye
RELEASE.md               a DOI / Zenodo / arXiv kiadás lépései
```

A magja (`tkr_measure.py`) **nulla harmadik fél függőséget** igényel. A
Qiskit csak a QPU-mérésekhez kell, és opcionális extra.

## Biztonság

Az API token **környezeti változóban** adható át, és a csomag soha nem
írja fájlba:

```bash
export IBM_QUANTUM_API_TOKEN="..."
```

A token <https://quantum.cloud.ibm.com> → Account settings → API key
oldalon hozható létre, 44 karakter, és **csak egyszer látható**.

## A mérési módszertan

Négy szabály, mindegyik egy-egy kimutatott hibából:

1. **A referencia-méret kötelezően egyezzen az áramkör méretével.** Egy
   4 kvantumos `|0>` referencia nem hasonlítható egy 2 kvantumes
   áramkörhöz — más a zajszint.
2. **Az automatikus értékelőnek meg kell különböztetnie a jót a
   rossztól.** Ha a jó mintát is elutasítja, túl széles a feltétel;
   ha a rosszat is átengedi, túl szűk.
3. **A bit-sorrendet aszimmetrikus próbával kell igazolni.** A Bell-állapot
   szimmetrikus, tehát a fordított bitek nem látszanak rajta.
4. **A hardverzaj-diagnózis referencia után jár.** Egyetlen eloszlásból
   nem különböztethető meg a zaj a transzpilálási vagy regisztrációs
   hibától.

Minden futás elején lefut a `bit_order_selfcheck`, és ha nem megy át, a
mérés **elutasításra kerül**.

## Korlátok — amit a rendszer NEM bizonyít

- **A visszafejtés nem egzakt.** A híd működik, de a Bell-egyensúly
  futásonként 87,7–99,3% (jellemzően ~92–95%) közé esik. Ezért a
  visszafejtett `A`, `f`, `φ` **jól közelít, de nem adja vissza az
  eredeti értéket pontosan**. Pontos értékhez hibajavító
  (error mitigation), jobb kvantumpár-kiválasztás vagy több lőés kell.
  A korábbi dokumentáció a 99,3%-os (legjobb) futást kezelte jellemző
  értékként — ez túlzás volt.
- **Az `O(1)` kvantum-rezonancia routing feltételezés.** Nincs
  komplexitásmérés, és egy 2 qubitemes kódolás önmagában nem ad
  kvantum-előnyt — egy klasszikus számítás is elvégzi ugyanezt.
- **A 9-es mélységhatár mérésfüggő.** A határ létezik, de a `k = 0.17
  rad/réteg` mért értékből jön; ±2% bizonytalansággal 8, 9 vagy 10 is
  adódhat. Nem önálló fizikai törvény.
- **A frekvencia-visszafejtés csak `[0, 0.5]`-en egyértelmű.** A 2π
  periodicitás és az (f, 1−f) kétirányú forma miatt egyetlen kétkvantumos
  interferenciából a teljes tartomány nem fordítható meg.
- **A 13 sáv Shannon-Nyquist indoklása feltételes.** A hardver
  sávszélességét ez a projekt nem méri.

## Licenc

MIT