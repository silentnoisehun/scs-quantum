# Az SCS kvantumhíd — mérnöki tervezési dokumentum

**Célközönség:** aki a `scs-quantum` crate-et és a Python mérési réteget
módosítja, illetve új kvantumhardver-backendet akar hozzáadni.
**Kapcsolódó dokumentum:** `SCS_WHITE_PAPER.md` (a tudományos állítások
és a hardveres validáció).

---

## 0. Az állítás-jelölések

Ez a dokumentum ugyanazt a jelölésrendszert használja, mint a white paper,
azért hogy egy állítás ne változtasson jelentést a két dokumentum között.

| Jelölés | Jelentés |
|---|---|
| ✅ **BIZONYÍTVA (klasszikus)** | `cargo test --release` (36 teszt) vagy offline számítás igazolja |
| 🔬 **BIZONYÍTVA (hardveres)** | Valódi QPU-n mért |
| ⚠️ **MÉRÉSTŐL FÜGGŐ** | Igaz, de a számérték mérésre épül |
| ⚠️ **FELTEVÉS** | Nincs mérés mögötte |
| ℹ️ **KONVENCIÓ** | Választott definíció |

> ### ⚠️ A zöld tesztek nem bizonyítják a hardvert
>
> A 36 zöld Rust-teszt a **klasszikus szoftver** helyességét igazolja. Nem
> állítja, hogy a hardver ugyanezt produkálja. A hardverre vonatkozó állítás
> forrása kizárólag a `python -m python.tkr_ibm` által végzett QPU-mérés.
>
> Ugyanez a mérnöki dokumentum szintjén: egy átmenő `to_qiskit` egységteszt azt
> bizonyítja, hogy a leképezés a specifikációnak megfelelő áramkört állít elő
> — **nem** azt, hogy a leképezett áramkör a QPU-n helyesen viselkedik.

---

## 1. Mi a feladat és mi nem

| Réteg | Felelősség | Függőségei |
|---|---|---|
| `rust/src/psi_quantum.rs` | kódolás / visszafejtés, `CircuitSpec` előállítása | `anyhow`, `serde` |
| `rust/src/field_engine.rs` | hullámcsomag-tárolás, mélységi ellenőrzés | – |
| `rust/src/band_map.rs` | sávhatár-szerződés, frekvencia → sáv | – |
| `python/tkr_measure.py` | hardverfüggetlen mérési szerződések | – |
| `python/tkr_ibm.py` | `CircuitSpec` → Qiskit, futtatás, riport | `qiskit`, `qiskit-ibm-runtime` |

A crate függőségei **két** (`anyhow`, `serde`). A korábbi, hálózati klienst,
gyártói eszközazonosítókat és külső IR-emittert tartalmazó réteg teljesen
kikerült a crate-ből. ✅ **BIZONYÍTVA (klasszikus)** — ez ellenőrizhető a
`rust/Cargo.toml` két függőségsorával.

---

## 2. Miért `CircuitSpec` és nem közvetlenül egy célplatform-áramkör

A `CircuitSpec` **hardverfüggetlen adatstruktúra**: kvantumszám, kapulista,
paraméterek. A Rust réteg nem ismer Qiskit-osztályt, nem ismer backendet, nem
tud hálózatot kezelni.

Ez három okból döntő:

1. **Tesztelhetőség.** A teljes kódolási és visszafejtési logika ellenőrizhető
   hardver, hálózat és hitelesítés nélkül. A `encode_frequency(0.25)` eredménye
   egy `CircuitSpec`, amelynek a kapulistáját egy egységteszt sorra hasonlítja.
   Nem kell hozzá QPU.
2. **Csere.** Új célplatform esetén **csak a lowering réteg** írandó újra. A
   kódolási képletek, a sávszerződés és a tárolás változatlan marad (§9).
3. **Fájlfüggetlenség.** A crate-nek nem kell semmilyen gyártói SDK-t
   telepíteni vagy linkelni ahhoz, hogy buildelhető és tesztelhető legyen.

**A lowering réteg belseje.** A `python/tkr_measure.py` maga a
`CircuitSpec`, `Gate` és `Counts` definícióját is tartalmazza — egy
hardverfüggetlen, gyártótól független modell, amellyel a Rust-oldali spec
szemantikailag egyezik. A két definíció összehangolása **szerződés**, nem
automatikus garancia: ha az egyikben megváltozik a mezőjelentés, a másikat is
kell módosítani. A §5 kulcsszerződés az, ami miatt ez a legérzékenyebb pont.

---

## 3. Az adatszerkezetek

### 3.1 `CircuitSpec` (Rust)

```rust
pub struct CircuitSpec {
    pub qubits: usize,
    pub gates: Vec<Gate>,
    pub decoherence: Option<f32>,   // γ — NEM kapu
}

pub struct Gate {
    pub gate_type: String,          // kanonikus név: "RY", "H", "CX", "CNOT", …
    pub qubits: Vec<usize>,         // 1 kvantumnál: [q]; 2 kvantumnál: [kontroll, cél]
    pub params: Vec<f64>,           // forgó kapu szöge
}
```

**A `decoherence` mező nem kapu.** A korábbi `DECOHERENCE` „kapu" nem létező
IR-utasítás volt: a γ (csillapítás) nem kvantumállapot-mennyiség, hanem
posztprocesszálási metaadat. A `no_decoherence_gate_anywhere` teszt rögzíti,
 hogy egyetlen kódolási kimenetben sincs ilyen kapu, és a γ megmarad a
 `WaveCircuitSet` mezőjében. ✅ **BIZONYÍTVA (klasszikus)**

**A három paraméter külön kimenetként.** Az `encode_wave` nem egyetlen
áramkört ad, hanem egy `WaveCircuitSet`-et:

```rust
pub struct WaveCircuitSet {
    pub amplitude: CircuitSpec,   // 1 kvantum
    pub frequency: CircuitSpec,   // 2 kvantum
    pub phase:     CircuitSpec,   // 2 kvantum
    pub gamma:     f32,           // posztprocesszálás
}
```

Ez a teljes kódolást **2 kvantumra** szorítja (`TKR_REQUIRED_QUBITS = 2`).
A `wave_splits_into_three_measurements` teszt ezt rögzíti: egyetlen nagy
áramkörben egy hibás kapu minden paramétert eltorzítana, és nem lenne
megkülönböztethető, melyik hibázott.

### 3.2 A kanonikus kapunév fontos

Az arititás-elvizsgálás a **kanonikus** néven (`g.gate_type`) történik, nem a
leképezés utáni néven (`cx`). Ez nem stíluskérdés:

> 🧬 Egy korábbi iteráció a `cx`-et az egykvantumos ágba küldte, így a
> `cx(control)` **egyetlen** argumentummal hívódott →
> `missing 1 required positional argument: 'target_qubit'`.

Ezért létezik külön `GATE_ALIASES` és külön `TWO_QUBIT_GATES` lista: az alias
a *leképezés* neve, az arititás a *szerződés*. Ha egy új kapu két kvantumos,
hozzá kell kerülnie a `TWO_QUBIT_GATES` halmazba, különben csendben
egykvantumosként kezeljük.

---

## 4. A `CircuitSpec` → Qiskit lowering szabályai

A `to_qiskit(spec, QuantumCircuit)` a lowering egyetlen belépési pontja.

### 4.1 Az arititás-szabály

**Az alkalmazás arititás-alapú, nem lista-hossz alapú:**

| Kapu osztály | Felsorolt kvantumok | Hívásmód |
|---|---|---|
| egykvantumos (`H`, `X`, `Y`, `Z`, `RY`, `RZ`) | `[q]`, vagy `[q0, q1, …]` | **minden felsorolt kvantumon külön** |
| kétkvantumos (`CX`, `CNOT`, `CZ`, `SWAP`, `ISWAP`) | pontosan `[kontroll, cél]` | **egy hívásban mindkettő** |

```python
if g.gate_type in TWO_QUBIT_GATES:
    method(g.qubits[0], g.qubits[1])       # cx(control, target)
else:
    for q in g.qubits:                     # h, ry, … minden listán szereplő q-n
        method(q, g.params[0]) if g.params else method(q)
```

Ezért a `random` referencia így néz ki: `Gate("H", [0, 1, 2])` — **egy** kapu
bejegyzés, három alkalmazás.

### 4.2 Miért nem lehet a sugarzási alak

⚠️ **Qiskit 2.x megszüntette a sugarzási alakot.** A `qc.h(range(n))` már nem
működik:

```
TypeError: takes 2 positional arguments but N were given
```

Ezért **nem** írható le a `random` referencia sem `qc.h(range(n))` formában.
Az arititás-alapú alkalmazás nem stílusválasztás, hanem **kompatibilitási
követelmény** a Qiskit 2.x-szal.

### 4.3 A kétkvantumos kapu hossz-ellenőrzése

Ha egy `TWO_QUBIT_GATES`-ben lévő kapu nem pontosan két kvantumot kap, a
lowering azonnal hibát emel:

```
ValueError: CX két kvantumot kér, 1 megadva
```

Ez **fail-fast**: egy rosszul felépített spec hibás áramkör helyett azonnali
hibát ad, nem néma félrevezetést. Ugyanez az ismeretlen kapunévre is igaz
(`ismeretlen kapu: …`).

### 4.4 A mérés hozzáadása

A lowering a ciklus végén, **minden** kvantumot a vele azonos sorszámú
klasszikus regisztre mér:

```python
qc.measure(range(spec.qubits), range(spec.qubits))
```

A `c[i] = q[i]` párosítás ℹ️ **KONVENCIÓ**, és a kulcssorrend értelmezését
*ez* rögzíti le — lásd §5.

### 4.5 A kétkvantumos kapu irányának jelentése

Az SCS-specifikus `CX` a Qiskit `cx`-re képez le, és **az első kvantum a
kontroll, a második a cél**. A `CNOT` az `cx` aliasa, így Az SCS-specifikus
`CNOT` név és a Qiskit-szintű `cx(control, target)` jelentése egyezik.

Az SCS frekvencia- és fáziskódolása `CNOT`-ot használ **`q[1]` → `q[0]`
irányban**: a vezérlő (`q[1]`) hordozza a paramétert, a cél (`q[0]`) a tiszta
referencia. A `interference_circuits_keep_reference_qubit_at_zero` teszt
rögzíti, hogy a `q[0]` nem kap `RY`-t, tehát valóban referencia marad. ✅
**BIZONYÍTVA (klasszikus)** — a hardveres működésre lásd a white paper §4.2-t.

---

## 5. A kulcsszerződés

Ez a dokumentum legérzékenyebb része, mert **hangos hiba nélküli** hibát lehet
elkövetni benne.

### 5.1 A formátum

A mérési eredményt a `counts_to_keys` a `{"0x..": db}` alakba hozza:

| Kulcs | Jelentés (2 kvantumnál) |
|---|---|
| `0x0` | `q[0]=0`, `q[1]=0` |
| `0x1` | **`q[0]=1`**, `q[1]=0` |
| `0x2` | `q[0]=0`, `q[1]=1` |
| `0x3` | `q[0]=1`, `q[1]=1` |

**A szabály: a kulcs legkisebb helyértékű bitje `c[0]`.**

A két formátum **azonos** sorrendet használ:
- a Qiskit a klasszikus regisztert balról jobbra írja ki, nagyobb sorszámmal
  előbb, tehát a kulcs `c[n-1] … c[1] c[0]` — a `c[0]` (jobban) a legkisebb bit;
- a `0x3` azt jelenti, hogy `q[0]=1` **ÉS** `q[1]=1`.

**Tehát nincs fordítás.** A `counts_to_keys` a biteket szándékosan nem
fordítja meg.

### 5.2 Miért kritikus: a hiba a legjobb teszten láthatatlan

> 🧬 **A mérési tanulság:** egy korábbi implementáció megfordította a biteket,
> és ez **hangos hibát nem adott**. A Bell-állapot szimmetrikus (`00`/`11`), tehát
> a fordított bitek rajta nem látszanak. Csak **aszimmetrikus** áramkörön
> jelentkezik (például az `X q[1]` próbán), ahol pedig **tévesen hardverhibát
> imitál**.
>
> Ez a legkockázatosabb hibatípus: nem jelez magáról semmit, és hamis
> diagnózist termel — a mérés azt mondaná, a kvantum hibás, miközben a hiba
> a szoftverben van.

### 5.3 A `bit_order_selfcheck` — minden mérés előtt

Mivel a hiba csendben lakozik a konverzióban, **a konverzió maga ellenőrzi
magát**, és ezt az ellenőrzés **minden futás előtt** lefut:

```
bit-sorrend önellenőrzés: X q[1] -> 0x2 (várt 0x2) ✓
```

A próba egy **ismeretlen, egyirányú** eredményt hasonlít az ismert
várttal:

| Lépés | Eredmény |
|---|---|
| szimulált mérés | `{"10": 100}` |
| `counts_to_keys(·, 2)` | `{"0x2": 100}` |
| Fordított bitekkel | `{"0x1": 100}` ← ✗ elhasal |

**Ha az ellenőrzés elhasal, a futás elutasításra kerül** (`BitOrderCheckFailed`,
kilépési kód **5**), nem pedig félrevezető eredményt ad tovább. A
`bit_order_selfcheck` a `IbmRunner.run` elején fut, a mérés előtt.

**Ne lazítsuk:** ha valaki „egyszerűsítés" kedvéért kihagyja ezt az
ellenőrzést, egy néma hiba kerül vissza a rendszerbe, és a legjobb teszt
tölti be a hibakereső szerepét. A `tkr_measure.py` modul docstringje ezt
explicit meg is írja.

---

## 6. A referencia-áramkörök

A referenciák **nem** az SCS kódolásai. Ők a hardver mérőszerszámai: minden
referenciának **ismert elvárt eredménye** van, és ha a hardver nem adja, a
hiba a hardverben vagy a mérési útban van.

| Név | Méret | Kapuk | Elvárt |
|---|---|---|---|
| `zero` | 1 (bármely) | nincs | `0x0` ≈ 100% — a mérési alapszint |
| `h` | 2 | `H q[0]` | `00` ≈ `01` ≈ 50/50, `10`/`11` elenyésző |
| `bell` | 2 | `H q[0]`, `CX q[0]→q[1]` | `00` ≈ `11` ≈ 50% |
| `ghz` | 3 | `H q[0]`, `CX q[0]→q[1]`, `CX q[1]→q[2]` | `000` ≈ `111` ≈ 50% |
| `random` | 4 | `H` minden qubiten | egyenletes, `100/2ⁿ` kimenetenként |

### 6.1 A méret-egyeztetés szabály

> ⚠️ **A referencméret kötelezően egyezik az áramkör méretével.**
> Különböző méretű mérések zajszintje nem hasonlítható össze.

Ezért a `--qubits` átméretezés **az áramkört is újraépíti** a
`resize_reference`-szel, nem csak a mérés címkéjét írja át:

```python
if n != spec.qubits:
    spec = resize_reference(kind, n)
```

A különbség nem elvi, hanem gyakorlati: ha a mérési réteg 3 kvantum, de az
áramkör még mindig 4 `H`-t tartalmaz, a mérés **4 bites kulcsokat** ad vissza,
és a konverter elutasítja:

```
bites szám-eltérés: '1100' (4 bit) vs n=3
```

— vagy, ami rosszabb, csendden elfogadná.

A `resize_reference` a minimum-méreteket is betartja: a `zero` bármire
érvényes (nincs kapuja), de a `h` és a `bell` legalább 2, a `ghz` legalább 3
kvantumot igényel.

### 6.2 A `--qubits` override

A `zero` a mérési alapszint: mivel a kvantumok biztosan `|0⟩` állapotban
vannak, `0x0` közel 100% várható. Az `1x0` → 98,25%, `2x0` → 98,70%,
`3x0` → 97,40% 🔬 **BIZONYÍTVA (hardveres)**. A méret növelésével
megmutatható, hogy a mérési alapszint **nem kvantumszám-függő** — vagyis az
eltérések a kapukból, nem a mérési infrastruktúrából jönnek.

---

## 7. Az értékelő — `report_counts`

### 7.1 A `bell` flag kötelező

A Bell-ellenőrzés **kizárólag** a Bell-áramkörnél érvényes, ezért a hívó
explicit flaggel adja át:

```python
bell=(a.cmd == "bell")
```

> 🧬 **A flag nem opcionális kényelmi elem.** A Bell-ellenőrzés korábban a
> `0x0` **ÉS** `0x3` kulcs jelenlétéből indult. Egy tiszta `H q[0]` mérés is
> adhatja mindkét kulcsot — a riport ilyenkor azt írta, hogy *„a CNOT nem
> működik"*, egy **működő, ideális** mérésre.
>
> **A mért bizonyíték:** a `h` parancs a flag nélkül `⚠️ nem ideális`-t írt,
> a flag-gyel tisztán **50,55 / 49,35**-öt adott ugyanarról a mérésről. Nem a
> mérés változott — a *kiértékelés* javult.

### 7.2 Az `uniform_bits` paraméter

Az egyenletesség-ellenőrzés az elvárt értéket **a hívótól** kapja, nem a
megjelent kulcsokból becsüli:

```python
uniform_bits=(n if a.cmd == "random" else None)
```

Ha a szerver nem küldi az összes kimenetet, a legnagyobb megjelent kulcs
**kevesebb bitet** hordozhat, és a belőle számított ideális érték hamisan
magasabb lenne.

### 7.3 Az értékelő fő szabálya

> 🧬 Egy automatikus értékelőnek **meg kell tudnia különböztetni a JÓ mintát a
> ROSSZtól.** Ha a jó mintát is elutasítja, a feltétel **túl széles**; ha a
> rossz mintát is átengedi, **túl szűk**.

Ha új ellenőrző feltételt írunk a `report_counts`-ba, ezt a kérdést **kötelező**
megválaszolni hozzá — különben a mérés hamis eredményt ad.

### 7.4 Amit a `00`/`11` eltérés NEM bizonyít

Ha a `00`/`11` arány gyenge, **önmagában ez nem bizonyít hardverzajt**. Lehet
transzpilálási hiba, bit-sorrend-probléma vagy kalibrációs eltérés is. Az
elkülönítéshez `|0>` referencia kell (`zero` parancs).

> ⚠️ **Ez a white paper 4.3. szabályának mérnöki formája:** egyetlen
> eloszlásból nem dönthető el a hiba eredete. A sorrend kötelező: előbb `zero`,
> utána lehet bármi mást zajnak nevezni.

---

## 8. A futtatási módok

### 8.1 QPU-mód (a hardveres bizonyíték)

```powershell
$env:IBM_QUANTUM_API_TOKEN = '<token>'
python -m python.tkr_ibm bell --shots 2000
```

A folyamat:

1. `bit_order_selfcheck()` — **elutasítja a futást**, ha nem megy át;
2. `to_qiskit(spec, QuantumCircuit)` — a lowering;
3. `transpile(qc, backend, optimization_level=1, seed_transpiler=42)` —
   a fix seed **reprodukálható** transzpilálást ad;
4. `SamplerV2(mode=backend).run([(tq,)], shots=N)`;
5. `counts_to_keys(counts, n)` — a kulcskonverzió, fordítás nélkül;
6. `report_counts(...)` — a kiértékelés.

**A `backend.run()` nem létezik.** A `qiskit-ibm-runtime` 0.5x eltávolította
(`Support for backend.run() has been removed`), ezért a futtatás a **Sampler
primitíven** megy:

```python
from qiskit_ibm_runtime import SamplerV2
job = SamplerV2(mode=self.backend).run([(tq,)], shots=shots)
```

Ez **API-szerződésbeli** hiba volt, nem mértékviteli: a kód lefordult, és csak
futásidőben bukott el. Ha újabb `qiskit-ibm-runtime` major kiadás jön, a
`SamplerV2` konstrukció az első ellenőrzési pont.

### 8.2 A `--local` mód — mit bizonyít és mit nem

```powershell
python -m python.tkr_ibm bell --local --shots 2000
```

**Amit bizonyít:** hogy a mérési pipeline helyes — a bit-sorrend, a klasszikus
regiszter-párosítás, a konverzió és a riport. Ideális `StatevectorSampler`
esetén a hiba csak a **saját kódunkban** lehet, mert nincs fizikai zaj és
nincs transzpilálás.

**Amit NEM bizonyít:** semmit a hardverről.

Ezért a `--local` (és a `--simulator`) induláskor **nagyon látható
figyelmeztetést** nyomtat ki:

```
!!  NEM VALÓDI HARDVER. Ez IDEÁLIS számítás.
!!  Eredménye a KÓD PIPELINE-ellenőrzése, nem QPU-validáció.
!!  Ha ez elhasal, a kód hibás. Ha átmegy, a kód rendben van —
!!  de a hardverről még semmit nem tudtunk.
```

> ⚠️ **Szimulátoros eredményt a bizonyítási anyagba írni pontosan az
> ellenkezőjét állítaná annak, amit mért.** A kimenet nevezhető
> `statevector (ideális, helyi)`-nak, és semmi másnak.

A `--local` token nélkül fut, ezért CI-beli és oktatási környezetben is
használható a pipeline ellenőrzésére.

### 8.3 Kilépési kódok

A hibák külön osztályok, hogy a CLI-t használó szkript megkülönböztesse őket:

| Kód | Kiváltó ok |
|---|---|
| 0 | siker |
| 1 | `IbmNotInstalled` — a `qiskit` / `qiskit-ibm-runtime` hiányzik |
| 2 | `IbmAuthFailed` — token/CRN hibás vagy nincs hozzáférés |
| 3 | `IbmBackendUnavailable` — nincs működő QPU (kvóta, karbantartás) |
| 4 | futási hiba |
| **5** | **`BitOrderCheckFailed` — a mérés elutasítva, mert félrevezető lenne** |

Az 5-ös kód külön figyelmet érdemel: **nincs eredmény**, nem pedig rossz eredmény.

---

## 9. Hitelesítés és titkok

**A token soha nem kerül fájlba.** Kizárólag környezeti változó:

| Változó | Kötelező? | Jelentés |
|---|---|---|
| `IBM_QUANTUM_API_TOKEN` | igen (alternatíva: `QISKIT_IBM_TOKEN`) | API token |
| `IBM_QUANTUM_INSTANCE` | opcionális (alternatíva: `QISKIT_IBM_INSTANCE`) | CRN |
| `IBM_QUANTUM_CHANNEL` | opcionális, alap: `ibm_quantum_platform` | csatorna |

Ellenőrzési szabályok:

1. **Nincs fájlból olvasás.** A `_token()` kizárólag `os.getenv`-et hív. Nincs
   config-fájl, nincs kulcstár, nincs argumentum-parancssor.
2. **A hiányzó token saját hibát hordoz.** A kivételüzenet elmondja, mit kell
   beállítani (`$env:IBM_QUANTUM_API_TOKEN = '<token>'`) és hol van a token
   (`Account settings → API key`). A hiba **nem** csomagolódik újra: a dupla
   becsomagolás („hitelesítés sikertelen: IbmAuthFailed: token hiányzik")
   elrejti a valódi okot.
3. **A `dotenv` betöltés opcionális.** Ha a `python-dotenv` telepítve van, egy
   helyi `.env` betölthető — de ez a felhasználó döntése, és a token így is a
   környezetbe kerül, nem a kódba.
4. **A `--local` nem igényel tokent.** A helyi ideális mód szándékosan
   credential-mentes, hogy pipeline-ellenőrzésre használható legyen.

> **Nyilvános kiadás előtt:** semmilyen token, CRN vagy példányazonosító nem
> kerülhet a repóba. A `.env` fájl gitignore-olt, a token pedig soha nem
> argumentum — a shells előzménye sem tárolja el.

---

## 10. Új hardver-backend hozzáadása

A crate **nem** igényel módosítást. A backend kizárólag a Python réteget
érinti. A szükséges lépések:

### 10.1 A backend-implementáció

A `python/tkr_ibm.py` mintája szerint a backend a következő szerződést
teljesíti:

| Szerződés | Követelmény |
|---|---|
| **Bemenet** | egy `CircuitSpec` + `shots` |
| **Kimenet** | `{bitstring: darabszám}` a platform natív formájában |
| **Előfeltétel** | a `bit_order_selfcheck()` átment |
| **Név** | `backend_label` — a riportban megjelenő azonosító |

### 10.2 Az `IbmRunner` felület, amit implementálni kell

| Metódus | Felelősség |
|---|---|
| `__init__` | inicializálás + `self._refs = reference_circuits()` |
| `_pick_backend` | backend kiválasztás, `IbmBackendUnavailable` jelzéssel |
| `run(kind, qubits, shots)` | méret-egyeztetés → önellenőrzés → lowering → futtatás → konverzió |
| `describe()` | backend státusz kiírása |
| `backend_label` | a `local` esetén is a valós módot írja ki |

### 10.3 Amit az új backendnek **kötelező** átvennie

Ez a négy pont a mérési tanulságokból származik, és **nem opcionális**:

1. **A méret-egyeztetés** (`resize_reference`) — különböző méretű mérések
   zajszintje nem hasonlítható össze (§6.1).
2. **A `bit_order_selfcheck` a mérés előtt** — különben csendes bit-sorrend-hiba
   (§5.3).
3. **A `counts_to_keys` használata** — a kulcskonverzió központilag, egy
   helyen (§5.1).
4. **A `bell=True` flag csak a Bell-áramkörön** — különben hamis pozitív
   Bell-verifikáció (§7.1).

### 10.4 Amit az új backendnek **nem** szabad megtennie

- **Nem fordíthatja meg a biteket** „szabványosítás" céljából. A szerződés a
  `c[0] = LSB`.
- **Nem becsülheti az elvárt értéket a mért kulcsokból** — a `uniform_bits`-t a
  hívó adja (§7.2).
- **Nem írhat szimulátoros eredményt hardveres bizonyítékként.** A `backend_label`
   köteles megkülönböztetni az ideális módot.
- **Nem létezhet konfignév a crate-ben.** A credential kizárólag környezetből
   jön (§9).

### 10.5 A lowering bővítése

Ha az új platform más kapu-neveket használ, **csak** a `to_qiskit`-szerű
lowering függvényt kell módosítani:

- az új névet az alias-táblába kell felvenni;
- ha az új kapu két kvantumos, a `TWO_QUBIT_GATES` halmazba is bekerül
  (§3.2 — különben csendden egykvantumosként kezeljük);
- ha a platform nem érti a `Gate` szerződését, akkor a `Gate`/`CircuitSpec`
  *specifikációt* kell bővíteni — és ezt **a Rust crate-ben is**, mert a két
  réteg definíciójának egyeznie kell.

---

## 11. A preserve-rekord

Ez a modul **három kimutatott hibát őriz**. Ne töröljük ki őket
„egyszerűsítés" kedvéért — mindegyik olyan hiba, amely csendben volt jelen,
vagy hamis pozitív riportot termelt.

| Őrzött szerződés | Őrzött hiba |
|---|---|
| `bit_order_selfcheck` + fordítás nélküli konverzió | Bit-sorrend megfordítása — a szimmetrikus Bell-eloszláson láthatatlan, aszimmetrikus próbán pedig **hardverhibát imitált** |
| `bell` flag a `report_counts`-ban | Hamis pozitív Bell-ellenőrzés — egy tiszta `H q[0]` mérést `CNOT nem működik`-nek ítélt |
| `SamplerV2(mode=backend)` | `backend.run()` eltűnése a `qiskit-ibm-runtime` 0.5x-ben |

Két további, a specifikációban rögzített határ:

| Határ | Hol |
|---|---|
| `f = arcsin(√P)/π` csak `[0, 0,5]`-en egyértelmű | `decode_frequency` docstring + `frequency_roundtrip_normalised_to_unity` |
| `φ = 2·arcsin(√P)`, **nem** `arcsin(√P)` | `phase_double_factor_is_required` regressziós teszt |

Az utóbbi külön tesztként létezik, nem csak a `phase_roundtrip_exact` mellett,
mert az egyszeres `arcsin` **félre redukálja** a szöget, és ez csak akkor látszik,
ha valaki direkt erre a hibára cseréli a képletet.

---

## 12. Tesztlefedettség — mit bizonyít és mit nem

`cargo test --release` → **36 passed, 0 failed** ✅ **BIZONYÍTVA (klasszikus)**

| Modul | Amit bizonyít |
|---|---|
| `field_engine` | csomag írás/olvasás bit-pontosan, minden sávra és stabil rétegre; a 13. sáv és a 10. réteg elutasítva |
| `band_map` | a sávok átfedés- és résmentes lefedése; a felső sávhoz tartozó határ; a `within ∈ [0,1)` zártság |
| `psi_quantum` | a kódolási képletek szerkezete; a visszafejtés pontossága ideális eloszláson; az `RZ`-tilalom; a γ nem kapu; a fázisfaktor-2 |

> ### ⚠️ Amit a 36 teszt NEM bizonyít
>
> - **Nem** bizonyítja, hogy a hardver ugyanezt produkálja.
> - **Nem** bizonyítja, hogy a `to_qiskit` leképezés a QPU-n helyes áramkört
>   állít elő — ehhez a §8.1 szerinti mérés kell.
> - **Nem** bizonyítja a frekvencia egyértelmű visszafejtését a teljes
>   tartományban — ez a kódolás ismert korlátja, nem tesztkérdés.
>
> Az egyetlen érvényes hardveres bizonyíték a `python -m python.tkr_ibm`
> QPU-mérése. 🔬 **BIZONYÍTVA (hardveres)** — a white paper §4.2.

---

## 13. Gyors referencia

```powershell
# Klasszikus teljes tesztkészlet
cd rust; cargo test --release

# Pipeline-ellenőrzés IDEÁLIS szimulációban — token nélkül,
# a hardverről semmit nem állít
python -m python.tkr_ibm bell --local --shots 2000

# Hardveres validáció — valódi QPU
$env:IBM_QUANTUM_API_TOKEN = '<token>'
python -m python.tkr_ibm zero --qubits 3 --shots 2000
python -m python.tkr_ibm h    --shots 2000
python -m python.tkr_ibm bell --shots 2000
python -m python.tkr_ibm ghz  --shots 2000
```

| Kilépési kód | Jelentés |
|---|---|
| 0 | siker |
| 1 | a `qiskit` / `qiskit-ibm-runtime` nincs telepítve |
| 2 | hitelesítési hiba (token / CRN / csatorna) |
| 3 | nincs elérhető QPU backend |
| 4 | futási hiba |
| 5 | **bit-sorrend-ellenőrzés elhasalt — a mérés elutasítva** |

---

*Minden hardverre vonatkozó állítás forrása a `python -m python.tkr_ibm`
mérése. A zöld tesztek a klasszikus szoftver helyességét bizonyítják, nem a
hardverét — ez a két bizonyíték nem cserélhető fel.*
