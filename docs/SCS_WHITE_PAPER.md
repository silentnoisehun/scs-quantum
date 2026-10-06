# A Space Computing System (SCS)

## White paper — a kvantumhíd hardveres validációval

**Verzió:** 2.2 · HARDVERESEN VALIDÁLT KIADÁS
**Projekt:** `scs-quantum` — crate `scs-quantum` (Rust) + mérési réteg (Python)
**Központi állítás:** az SCS kvantumhídja nem szimulált és nem feltételezett, hanem
valódi szupravezető kvantumprocesszoron mért.

---

## 0. Az állítás-jelölések (olvasd el előbb)

Ez a dokumentum minden műszaki állításhoz megadja a bizonyítékának *fokozatát*.
A jelölések nem díszek: két különböző minőségű állítást soha nem szabad
összekeverni.

| Jelölés | Jelentés | Mit NEM jelent |
|---|---|---|
| ✅ **BIZONYÍTVA (klasszikus)** | A Rust tesztkészlet (`cargo test --release`, 36 teszt) vagy offline számítás igazolja. | Nem jelent hardveres bizonyítékot. |
| 🔬 **BIZONYÍTVA (hardveres)** | Valódi QPU-n, ismert elvárt eredményű referencia-áramkörrel mért. | Nem jelent minden backend-en érvényes állítást. |
| ⚠️ **MÉRÉSTŐL FÜGGŐ** | Az állítás igaz, de a számérték mérési eredményre épül, és a mérés bizonytalanságával változhat. | Nem önálló fizikai törvény. |
| ⚠️ **FELTEVÉS** | A modell feltételezi. Nincs róla komplexitásmérés. | Nem bizonyított. |
| ℹ️ **KONVENCIÓ** | A rendszer által választott definíció vagy mértékegység-választás. | Nem természeti törvény. |

> ### ⚠️ KÖTELEZŐ FIGYELMEZTETÉS — a zöld tesztek NEM bizonyítják a hardvert
>
> A `cargo test --release` 36 zöld eredménye a **klasszikus szoftver** helyességét
> bizonyítja: a kódolási és visszafejtési képlet, a sávmegfeleltetés és a
> bit-sorrend szerződése egyezik a saját definíciójával.
>
> Ebből **nem** következik, hogy a hardver ugyanezt produkálja. A hardverre
> vonatkozó minden állítás kizárólag a §4-ben ismertetett, valódi QPU-n
> végzett mérésből származik. Ahol a kettő ellentmondana, a mérés az
> mérvadó, és ezt a dokumentum ki is mondja.
>
> Ugyanez fordítva is igaz: a §4 szerinti hardveres mérések **nem** bizonyítják,
> hogy a teljes SCS — minden sáv, minden réteg, minden paraméter együtt —
> helyesen működik. A hardveres bizonyíték a híd *alapkapuira* vonatkozik.

---

## 1. Absztrakt

A Space Computing System (SCS) egy hullám-alapú memóriaarchitektúra, amelyben nem az
adat és a kód válik szét, hanem maga a hullámforma hordozza mindkettőt. A
tér (`Field`) egy lapokra osztott, dinamikusan bővíthető memória-mező, amely
**hullámcsomagokat** (`WavePacket`) tárol:

```
ψ(t) = A · exp(−γ·t) · cos(2π·f·t + φ)
```

| Jel | Szerep | Hol tárolódik |
|---|---|---|
| `A` | amplitúdó (intenzitás) | kvantumhíd, 1 kvantum |
| `f` | frekvencia (tartalom) | kvantumhíd, 2 kvantum + sávszerkezet |
| `φ` | fázis (kontextus) | kvantumhíd, 2 kvantum + mélységi réteg |
| `γ` | csillapítás (decay) | posztprocesszálási mennyiség — **nem kapu** |

A rendszer két, egymástól teljesen független rétegből áll.

**A klasszikus réteg** ✅ **BIZONYÍTVA (klasszikus)** — a frekvenciatartomány
13 sávra, a tárolás 9 mélységi rétegre bontott, és e felett egy `FieldEngine`
őrzi a `WavePacket`-ek bit-pontos olvasását és írását. Ez ma önállóan is
használható rendszer.

**A kvantumhíd** 🔬 **BIZONYÍTVA (hardveres)** — a `WavePacket` három
paraméterét kvantumáramkörbe képezi, majd a mért eloszlásból visszafejti őket.
Ez a híd **2026.10.06-án egy valódi, 156 kvantumos IBM szupravezető QPU-n**
(`ibm_marrakesh`, 2000 lőés/futás) lett validálva: a `H`, a `CNOT` és a
kétláncú `CNOT` (GHZ) egyaránt az elvárt eloszlást adja.

A dokumentum három dolgot tesz egymástól elválaszthatóvá, mert a projekt
története során pont ezek összekeveredése okozta a legtöbb félrevezető
állítást:

1. **melyik kódolás működik** és **miért nem működött a korábbi** (§3),
2. **mit mértünk a hardveren** és **mit nem** (§4),
3. **melyik számítás mérésfüggő, és melyik feltételezés** (§5–§6).

---

## 2. A rendszer modellje

### 2.1 A hullámcsomag

A `WavePacket` négy `f32` mezőből áll, `#[repr(C)]` attribútummal, így a
bináris képe **16 bájt**, és a `to_bytes` / `from_bytes` oda-vissza
bit-pontosan azonos.

```rust
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WavePacket {
    pub amplitude: f32,   // A — intenzitás
    pub gamma:     f32,   // γ — csillapítás
    pub frequency: f32,   // f — tartalom
    pub phase:     f32,   // φ — kontextus
}
```

A 16 bájtos alak nem véletlen: `PACKETS_PER_PLANE = 8128 / 16 = 508`, vagyis
egy logikai síkba **508 hullámcsomag** fér el, és a `PACKETS_PER_PLANE *
PACKET_BYTES == PAGE_SIZE` pontosan zárt. ✅ **BIZONYÍTVA (klasszikus)**

A modell egy **állapotgép**. Nem a memóriája, nem a futásideje és nem a
kvantumhídja ismeri a `ψ` függvényt: mindegyik réteg csak a saját
paraméterrészét kezeli.

### 2.2 A tizenhárom frekvenciasáv

ℹ️ **KONVENCIÓ** A sávok száma a projekt *mértékegység-választása*. Az
alábbi indoklás megadja, miért 13 és nem más, de a belőle következő
sávszélességek nem függnek hardverméréstől, mert **ez a projekt nem méri a
sávszélességhez tartozó sávszélesség-arányt** — lásd §6.

Az indoklás a Shannon–Nyquist-fé felezésen alapul: a használható sávszélesség a
teljes sávszélesség fele. Ha 26 al-sáv gerjeszthető átfedés nélkül, ezekből 13
alkalmazható. A felosztás öt funkcionális csoportba szervezi a 13 indexet:

| Csoport | Sávok | Tartomány (normált) | Funkció |
|---|---|---|---|
| `DC` | B1 | `[0.00, 0.02)` | egyenáram / nyugalmi komponens |
| `LOW` | B2–B4 | `[0.02, 0.20)` | lassú hullámok |
| `MID` | B5–B8 | `[0.20, 0.60)` | fő tartalom |
| `HIGH` | B9–B12 | `[0.60, 0.98)` | tranziensek |
| `GAMMA` | B13 | `[0.98, 1.00)` | gyors szinkron |

**A sávszerződés — ℹ️ KONVENCIÓ, de géppel ellenőrzött.** A
`Band::range()` **félnyílt** tartományt ad vissza: `[lo, hi)`. A felső határ
a *következő* sávhoz tartozik. Ezért:

- `Band::of_frequency(0.02) == Band::Low` — a 0,02-es határ a `LOW`-hoz,
  nem a `DC`-hez kerül;
- `Band::frequency_of(band, 1.0)` a *következő* sáv alsó határát adja,
  tehát a `frequency_of` ∘ `of_frequency` körút csak `within ∈ [0.0, 1.0)`
  esetén zárt. Ez **ismert korlát**, és a `frequency_roundtrip_within_band`
  teszt ezt a zárt tartományt vizsgálja, nem a `1.0`-at.

A `assert_no_overlap()` ✅ **BIZONYÍTVA (klasszikus)** ellenőrzi, hogy az öt
csoport átfedés és rés nélkül fedi a teljes `[0.0, 1.0]` tartományt, és hogy
minden csoportban `hi > lo`. A sávhatárok egyértelműsége tehát **nem
dokumentációs ígéret, hanem teszttel bizonyított invariáns**.

### 2.3 A kilenc mélységi réteg

A rétegek száma egy **fázis-drift** feltételezésből jön le. Ha a rétegmélység
egységenként `k` radián fázist forgat el, akkor a rétegszám `d` esetén a
fázissorulás `Δφ(d) = k · d`, és az olvashatóság feltétele `Δφ < π/2`.

A mért konstans: **k ≈ 0,17 rad/réteg**.

| Mélység | Δφ (rad) | Státusz |
|---|---|---|
| 0–8 | < 1,57 | ✅ stabil |
| 9 | ≈ 1,53 | ✅ stabil, de ez a határ |
| 10 | ≈ 1,70 | ❌ összeomlik |

Ebből `0,17 · d < π/2 = 1,5708` → `d < 9,24` → a legnagyobb stabil egész
mélység **9**. A `FieldEngine` ezt kikényszeríti: a `MAX_DEPTH = 9`, és a
10. réteg írása pánikkal (fázis-összeomlás) elbukik.

> ### ⚠️ MÉRÉSTŐL FÜGGŐ — a „9" nem önálló fizikai törvény
>
> A fizikai állítás az, hogy **van egy ilyen határ**. Az, hogy a határ
> *pontosan 9*, a mért `k = 0,17 rad/réteg` értékből levezetett eredmény.
>
> Mivel `k` négy számjegyre van kerekítve, a `9` **nem robustus**:
>
> | `k` (rad/réteg) | Legnagyobb stabil mélység |
> |---|---|
> | 0,1550 | 10 ← eltér |
> | 0,15708 – 0,17453 | **9** ✓ |
> | 0,1750 | 8 ← eltér |
>
> A `9` kizárólag akkor adódik ki, ha `k ∈ [0,15708, 0,17453)`. A bizonytalanság
> hatóköre tehát 8, 9 **vagy** 10 réteget is megengedhet. (Pontosítás: a szigorú
> ±2%-os `k`-bizonytalanság — `[0,1666; 0,1734]` — még a 9-en belül marad; a 8
> határhoz `k` körülbelül +2,7%-os, a 10 határhoz mintegy −7,6%-os eltérése
> kell. A bizonytalanság sávszélessége maga is mérésfüggő, ezért a határt nem
> szabad stabil mélységként kezelni.)
>
> **Következmény:** a 9 mélységi réteg a jelenlegi *implementáció* döntése, nem
> a természet megállapítása. Ha holnap pontosabb `k`-mérés adódik, a 9 változhat
> — és ezt a rendszernek **paraméterként**, nem konstansként kell kezelnie.

### 2.4 A tér és a FieldEngine

A `FieldEngine` 13 frekvenciasávot és 9 mélységi réteget kezel, csomagszinten
`8128` bájtos lapokra osztva.

A tárolás **bit-pontos** ✅ **BIZONYÍTVA (klasszikus)**: `write_wave`, majd
`read_wave` azonos `WavePacket`-et ad vissza, és ez minden sávra és minden
stabil mélységre igaz. A sávszám és a mélység ellenőrzése része a szerződésnek
(a 13. sáv és a 10. réteg elutasítva).

A FieldEngine ma **önállóan is teljes értékű komponens**: az SCS kvantumoldali
része nélkül is használható, és a rendszer felső szintű működését nem
függteti a híd sikerétől vagy kudarcától.

---

## 3. A kvantumhíd — kódolás és visszafejtés

### 3.1 A negatív eredmény: miért NEM működött az `RZ`-alapú kódolás

Ez a rész a projekt egyik legfontosabb tanulságát tartalmazza, ezért a kudarc
is dokumentált, nemcsak a siker.

Az eredeti séma az `f`-et és a `φ`-t egy-egy `RZ` kapuval kódolta:

```
|0⟩ ──RY(A)──┐
|0⟩ ──RZ(2πf)─┤        ↓
              ├─ MEASURE (Z)
|0⟩ ──RZ(φ)───┘
```

**Az `RZ` nem használható ilyen célra.** Az `RZ(θ)` egy **globális fázisszorzó**:
a `|0⟩` és `|1⟩` komponenseket azonos `e^{iθ}` tényezővel szorozza. A közvetlen
Z-mérésnél ez a tényező **kiesik** — a mért valószínűségek nem függnek tőle.

A következmény mérhető volt: a kimeneti eloszlás **bitre azonos** volt minden
`f` és `φ` értékre. Vagyis a kódolás nem volt hibás számításokban — **a
beágyazás nem volt invertálható**. Egyetlen paramétert lehetett visszanyerni:

```
A = 2·arccos(√P(q₀=0))
```

A `γ` pedig eleve nem kapu: a korábbi `DECOHERENCE` „kapu" nem létező utasítás
volt. A γ **posztprocesszálási mennyiség**, ezért a `CircuitSpec`-ben
`Option<f32>` mezőként él, nem a kapulistában. A `no_decoherence_gate_anywhere`
teszt ezt rögzíti: egyetlen kódolási kimenetben sincs `DECOHERENCE` kapu,
és a γ megmarad posztprocesszálásra.

**A tanulság:** nem elég, hogy a kódolási képlet matematikailag helyes. A
kódolásnak **mérhetőnek** kell lennie azon a mérésen, amelyből vissza akarod
fejteni. Az `RZ` a második feltételt sérti.

### 3.2 A működő séma: interferencia-kapcsolt `RY` referenciakvantummal

A megoldás: a paramétert egy **második kvantumra** helyezzük, és egy `CNOT`-tal
visszük át a tiszta `|0⟩` referenciára. Ekkor a paraméter már **relatív
fázisként** jelenik meg a két kvantum között, és a Z-mérés érzékeny rá.

```
q[0]:  |0⟩ ────────────────── tiszta referencia ────┐
                                                  CNOT(1→0)
q[1]:  |0⟩ ──RY(θ)──┘                              │
                                                     ↓
                                        P(q₀=1) = sin²(θ/2)
```

| Paraméter | Kvantum | Kapulista | Eloszlás | Visszafejtés |
|---|---|---|---|---|
| `A` | 1 | `RY(A)` q[0] | `P(q₀=1) = sin²(A/2)` | `A = 2·arccos(√P(q₀=0))` |
| `f` | 2 | `RY(2πf)` q[1], `CNOT(1→0)` | `P(q₀=1) = sin²(πf)` | `f = arcsin(√P(q₀=1)) / π` |
| `φ` | 2 | `RY(φ)` q[1], `CNOT(1→0)` | `P(q₀=1) = sin²(φ/2)` | `φ = 2·arcsin(√P(q₀=1))` |
| `γ` | — | nincs áramkörben | — | posztprocesszálás |

**A fázis-dekódolóban a kettő a helyes tényező.** Mivel
`P = sin²(φ/2)`, a visszafejtés `φ = 2·arcsin(√P)`; az egyszeres `arcsin` a
szöget **felezi**. Ezt a `phase_double_factor_is_required` regressziós teszt
rögzíti külön, nem csak a `phase_roundtrip_exact` teszt hallgatólagos
eredményeként.

### 3.3 Miért külön-külön mérünk minden paramétert

A három paramétert **szándékosan külön áramkörben** mérjük. Egyetlen három
kvantumos áramkör azért nem készült, mert egyetlen hibás kapu **mindhárom
paramétert** eltorzítaná, és nem lenne megkülönböztethető, melyik hibázott.
Külön mérésnél egy hibás kapu **egyetlen** paramétert érint.

Ennek köszönhetően a teljes kódolás **két kvantummal** készül el
(`TKR_REQUIRED_QUBITS = 2`), nem hárommal. A `WaveCircuitSet` nem egyetlen
áramkört tartalmaz, hanem három külön `CircuitSpec`-et és a γ értékét.

### 3.4 A frekvencia-dekódolás korlátja — itt nem overclaimelünk

> ⚠️ **A frekvencia visszafejtése a teljes tartományon nem egyértelmű.**
>
> Az `f = arcsin(√P)/π` képlet **egyértelmű a `[0, 0,5]` tartományon**, és
> osztással egységnyi frekvenciára méretezi. Két ok miatt nem több:
>
> 1. **A 2π-periodicitás.** A kódolás `RY(2πf)`-et használ, ezért a
>    kimeneti eloszlás `f` és `f + 1` esetén azonos.
> 2. **A kétirányú (f, 1−f) forma.** Mivel `P = sin²(πf)` és
>    `sin²(πf) = sin²(π(1−f))`, az `f` és az `1−f` ugyanazt az eloszlást adja.
>
> Egyetlen kétkvantumos interferencia **nem képes** a frekvenciát a teljes
> tartományban egyértelműen visszafejteni. Ez a kódolás **ismert korlátja**,
> nem megoldandó hiba: a `frequency_roundtrip_normalised_to_unity` teszt
> szándékosan csak `[0, 0,5]`-en ellenőriz.
>
> További kvantum vagy egy második, más fázissal kódolt kísérlet növelheti az
> egyértelmű tartományt — ez a §6 jövőbeli munkája.

### 3.5 Amit a kódolás offline bizonyít

A teljes körút (kódolás → eloszlás → visszafejtés) offline, ideális
eloszlásokon **gépi pontossággal** zárt:

| Paraméter | Tartomány | Eredmény |
|---|---|---|
| `A` | a fenti képlet szerint | ✅ 1e-16 nagyságrendű hiba |
| `f` | `[0, 0,5]` | ✅ 1e-16 nagyságrendű hiba |
| `φ` | `[0, 3,14]` | ✅ 1e-16 nagyságrendű hiba |

⚠️ **Ez a táblázat a KÓD helyességét bizonyítja, semmit a hardverről.** Az
offline hiba a képlet és az elvárt eloszlás közötti eltérés — olyan körben,
ahol nincs transzpilálás, nincs kvantumzaj, és nincs mérési bizonytalanság.
A hardveres igazolás kizárólag a §4 fejezetből jön.

---

## 4. Hardveres validáció

### 4.1 A mérés körülményei

| Paraméter | Érték |
|---|---|
| Dátum | 2026.10.06 |
| Eszköz | valódi szupravezető QPU, `ibm_marrakesh`, 156 kvantum |
| Lőésszám | 2000 futásonként |
| Vezérlő | `python -m python.tkr_ibm` |

Minden futás **előtt** lefut a `bit_order_selfcheck` (§4.4.1). Ha az nem megy
át, a mérés **elutasításra kerül** — nem egy félrevezető eredményt ad tovább.

### 4.2 A mért eredmények

🔬 **BIZONYÍTVA (hardveres)** — minden szám az alábbi táblázatból származik.

| Áramkör | Mért | Elvárt | Értékelés |
|---|---|---|---|
| `zero --qubits 1` | `0x0` = 98,25% | ~100% | mérési alapszint, 1 kvantum |
| `zero --qubits 2` | `0x0` = 98,70% | ~100% | mérési alapszint, 2 kvantum |
| `zero --qubits 3` | `0x0` = 97,40% | ~100% | mérési alapszint, 3 kvantum |
| `h` (H csak q[0]-on) | `00` = 50,55%, `01` = 49,35% | 50/50 | ✅ ideális |
| `bell` — 1. futás | `00` = 49,35%, `11` = 49,00% | 50/50 | ✅ **egyensúly 99,3%** |
| `bell` — 2. futás | `00` = 48,50%, `11` = 47,90% | 50/50 | ✅ **egyensúly 98,8%** (reprodukálható) |
| `bell` — refaktor után | `00` = 49,30%, `11` = 47,95% | 50/50 | ✅ **egyensúly 97,3%** |
| `ghz` (H + 2× CNOT) | `000` = 49,20%, `111` = 47,60% | 50/50 | ✅ ideális, zaj 1,80% |

A Bell-állapot zajösszege (`01` + `10`) **1,65%**. Ez egy kétkvantumos
szupravezető QPU-n **normális** érték: a `01` és `10` ágak a hibás olvasás,
a gerjesztés- és lecsengési hibák természetes hordozói.

**Mit bizonyít ez a táblázat?**

- A `zero` referencia megadja a **mérési alapszintet** (a kvantumok szinte
  biztosan `|0⟩`). Enélkül bármely más eredmény értelmezhetetlen lenne.
- A `h` igazolja az egykvantumos szuperpozíciót.
- A `bell` 99,3%-os egyensúlya igazolja a `CNOT`-ot.
- A `bell` három egymást követő eredménye (99,3% / 98,8% / 97,3%)
  **reprodukálhatóságot** bizonyít: a mérés nem egyszeri szerencsés találat.
- A `ghz` azt igazolja, hogy a **két egymás utáni `CNOT`**, azaz a
  háromkvantumos lánc is működik.

**Következmény a dekódolásra:** a visszafejtési képletek alapfeltevése
`P(q₀=1) = sin²(θ/2)`. A `H`, a `CNOT` és a kétláncú `CNOT` mért működése
**méréssel igazolt feltétel**, nem feltételezés. Ez a híd alapja.

### 4.3 A mérési módszertan négy szabálya

Minden szabály egy **elkapott hibából** származik. Egyik sem általános
elválasztás, mindegyik konkrétan fizetett tanács.

**1. A referencméret kötelezően egyezik az áramkör méretével.**
Egy 4 kvantumos `|0⟩` referencia nem hasonlítható össze egy 2 kvantumos
áramkörrel: a két mérés zajszintje eltér, és az összehasonlítás hamis
eltérést mutat. A `--qubits` átméretezés ezért **az áramkört is újraépíti**,
nem csak a címkét írja át.

**2. Egy automatikus értékelőnek meg kell tudnia különböztetni a jó mintát a
rossztól.** Ha a jó mintát is elutasítja, a feltétel túl széles; ha a rossz
mintát is átengedi, túl szűk. Mindkét irányt tesztelni kell.

**3. A bit-sorrendet aszimmetrikus próbával kell bizonyítani.**
Egy szimmetrikus eloszlás — mint a Bell `00`/`11` párja — **nem alkalmas** a
bit-sorrend eldöntésére: a fordított bitek rajta nem látszanak. Csak egy
egyirányú próba különbözteti meg a két konverziót.

**4. Hardverzajra vonatkozó állításhoz előbb `|0⟩` referencia kell.**
Egyetlen eloszlásból nem lehet eldönteni, hogy a látott eltérés hardverzaj,
transzpilálási hiba, bit-sorrend-hiba vagy kalibrációs eltérés. A sorrend:
előbb `zero`, és csak utána lehet bármi mást zajnak nevezni.

### 4.4 A három elkapott hiba — mindegyik tanács

#### 4.4.1 Bit-sorrend megfordítása

**A hiba.** Az első IBM→köztes kulcs-konverzió megfordította a biteket.

**Miért maradt észrevehetetlen.** A Bell-állapot **szimmetrikus**: a `00` és a
`11` felcserélése nem változtatja meg az eloszlást. A hiba tehát a *legjobb*
teszten — ahol a leginkább bizonyítani akartunk — teljesen láthatatlan volt.

**Miért veszélyes.** Csak egy **aszimmetrikus** próbán jelentkezik (például
`X q[1]`), és ott **tévesen hardverhibát imitál**: a mérés azt mondaná, a
kvantumhibás, miközben a hiba a szoftverben van. Ez a legkockázatosabb
hibatípus: nem látszik, és hamis diagnózist termel.

**A javítás.** Nincs fordítás: **a kulcs legkisebb helyértékű bitje `c[0]`**. A
`bit_order_selfcheck` az `X q[1]` → `c1c0 = "10"` → `0x2` próbával dönti el a
kérdést, és **minden mérés előtt** fut. Sikertelen ellenőrzés esetén a futás
elutasításra kerül (`BitOrderCheckFailed`, kilépési kód 5) — mert a
félrevezető eredménynél rosszabb a mérés.

#### 4.4.2 A hamis pozitív Bell-ellenőrzés

**A hiba.** A riport a `0x0` **ÉS** `0x3` kulcs jelenlétéből indította a
Bell-ellenőrzést.

**Miért hamis pozitív.** Egy tiszta `H q[0]` mérés is **szintén** adhatja mindkét
kulcsot. A riport ezért egy tökéletesen működő, ideális eloszlást
`CNOT nem működik` üzenettel utasított el.

**A javítás.** Explicit `bell=True` flag, amelyet **csak** a Bell-áramkör
kap. Ez nem opcionális kényelmi elem: az ellenőrzés feltétele az
áramkör azonosítójától függ, nem a mért számok formájától.

**A mért bizonyíték.** Ugyanaz az `h` futás a flag bekapcsolása előtt
`⚠️ nem ideális`-t írt, a flag-gyel **tisztán 50,55 / 49,35**-öt adott. Vagyis
nem változott a mérés — a *kiértékelés* javult.

#### 4.4.3 A `backend.run()` eltűnése

**A hiba.** A `qiskit-ibm-runtime` 0.5x **eltávolította** a
`backend.run()` metódust.

**A javítás.** A QPU-futtatás a **Sampler primitíven** megy:
`SamplerV2(mode=backend).run([(circuit,)], shots=N)`. A hiba nem
mértékviteli, hanem API-szerződésbeli: a kód lefordult, és csak futásidőben
bukott el.

---

## 5. A bizonyítási szintek

Ez a fejezet a dokumentum lényege: **melyik állításnak mennyi bizonyítéka van.**
A táblázat szándékosan nem egyetlen „állapot" oszlopot tartalmaz, mert a
projekt történetében éppen az egyetlen oszlopos összefoglalók okozták a
legnagyobb félrevezetést.

### 5.1 ✅ Bizonyítva (klasszikus)

| Állítás | Bizonyíték |
|---|---|
| `A`, `f`, `φ` körút a hullámcsomagon veszteségmentesen zár | offline 1e-16 nagyságrend, `cargo test --release` |
| `FieldEngine` bit-pontos írás és olvasás | 13 sáv × 9 réteg teljes körbejárása |
| A sávmegfeleltetés egyértelmű és átfedésmentes | `assert_no_overlap()`, `band_boundary_belongs_to_the_upper_band` |
| A bit-sorrend szerződése rögzített | `key_0x3_means_both_qubits_are_one` + `bit_order_selfcheck` |
| A teljes tesztkészlet zöld | **`cargo test --release` → 36 passed, 0 failed** |

A crate három modult tartalmaz: `field_engine` (a csomagtárolás és a mélységi
ellenőrzés), `band_map` (a sávhatár-szerződés), `psi_quantum` (a kódolás és a
visszafejtés).

### 5.2 🔬 Bizonyítva (hardveres)

| Állítás | Bizonyíték |
|---|---|
| A `H` kapu az elvárt 50/50 eloszlást ad | 50,55% / 49,35%, 2000 lőés |
| A `CNOT` működik | Bell-egyensúly 99,3%, reprodukálva 98,8%-kal és 97,3%-kal |
| A kétláncú `CNOT` (3 kvantum) működik | GHZ `000` = 49,20%, `111` = 47,60% |
| A mérési alapszint ismert | `zero` 1/2/3 kvantumon: 98,25% / 98,70% / 97,40% |

**Amit ez NEM bizonyít:** hogy a teljes SCS — minden sávval, minden réteggel,
mindhárom paraméterrel együtt — helyesen működik. A hardveres bizonyíték a
híd **alapkapuira** vonatkozik.

### 5.3 ⚠️ Mérésfüggő

| Állítás | Miért mérésfüggő |
|---|---|
| A 9 mélységi réteg a fázis-stabilitási határ | A határ **létezik**, de hogy éppen 9, a mért `k = 0,17 rad/réteg` értékből adódik. A bizonytalanság 8, 9 vagy 10 réteget is megengedhet (§2.3). Nem önálló fizikai törvény. |

### 5.4 ⚠️ Feltevés

| Állítás | Miért feltevés |
|---|---|
| Az O(1) kvantumrezonancia-irányítás | A modell feltételezi, hogy egy rezonancia-hangolás állandó költségű. **Nincs komplexitásmérés**, amely ezt támasztaná. |

### 5.5 ℹ️ Konvenció

| Állítás | Miért konvenció |
|---|---|
| A 13 sáv Shannon–Nyquist-alapú indoklása | Az indoklás a sávszélesség felezésén alapul, de **a projekt nem méri azt a sávszélességet**, amelyre az indoklás épül. A 13 ezért választott definíció, nem mért optimum. |

---

## 6. Korlátok és jövőbeli munka

### 6.1 Ami ma nem működik, és miért

| Korlát | Természete | Következmény |
|---|---|---|
| A frekvencia-dekódolás `[0, 0,5]`-en egyértelmű | **A kódolás matematikai korlátja**, nem hiba | A teljes `[0, 1]` tartomány nem olvasható vissza egyetlen kétkvantumos interferenciából (§3.4) |
| A mélységi határ 9 | ⚠️ mérésfüggő | 8 és 10 is lehetséges a bizonytalanságon belül (§2.3) |
| Az O(1) routing | ⚠️ feltevés | Nincs komplexitásmérés |
| A 13 sáv sávszélessége | ℹ️ konvenció | A sávszélességet a projekt nem méri |

### 6.2 Jövőbeli munka

**A frekvencia egyértelműsítése.** A 2π-periodicitás és az (f, 1−f) szimmetria
miatt egyetlen interferencia nem elég. A természetes irány egy **második
frekvenciapróba eltérő fázissal**, amely a két szimmetria-tengely egyikét
előnyben részesíti. A `WaveCircuitSet` szerkezete ezt már lehetővé teszi:
külön kísérletek helyett több frekvenciapróba illeszthető bele.

**A réteghatár paraméteresítése.** A `k` ma konstans. A 9 mélység helyett a
`k` mérési bizonytalanságát kell hordoznia a konfigurációnak, és a határt
`k`-ból számítani, nem beégetni.

**A frekvenciahordozó sávszerkezet és a kvantumhíd összekötése.** Ma a
sávszerkezet (klasszikus) és a frekvenciadekódolás (kvantum) külön él. A
`Band::frequency_of` szerződése (`[lo, hi)`, `within ∈ [0,1)`) már alkalmas
rá, de a kettő összekapcsolása nincs implementálva és nincs mérve.

**A hardveres bizonyíték kiterjesztése.** A jelenlegi validáció a híd
alapkapuit igazolja. Nem igazolt még: a teljes `A`, `f`, `φ` körút hardveres
pontossága egy ismert bemeneti hullámcsomagon, végig a `to_qiskit` →
transzpilálás → mérés → visszafejtés láncon. **Ez a következő mérési
blokk**, és a módszertan négy szabálya rá érvényes.

### 6.3 Amit a dokumentum szándékosan nem állít

- Nem állítja, hogy a zöld klasszikus tesztek bizonyítják a hardver működését.
- Nem állítja, hogy egyetlen kétkvantumos interferencia a frekvenciát
  egyértelműen visszafejti a teljes tartományban.
- Nem állítja, hogy a 9 mélység független fizikai törvény.
- Nem állítja, hogy az O(1) routing költségét mérés igazolta.
- Nem nevez meg olyan hardvert, amelyen a rendszert nem mértük.

---

## 7. Reprodukálás

```powershell
# Klasszikus réteg — 36 teszt
cd rust; cargo test --release

# Hardveres validáció — valódi QPU, token a környezetből
$env:IBM_QUANTUM_API_TOKEN = '<token>'
python -m python.tkr_ibm zero  --qubits 3 --shots 2000
python -m python.tkr_ibm bell  --shots 2000
python -m python.tkr_ibm ghz   --shots 2000
```

A token **kizárólag környezeti változóban** adható át, soha nem fájlban.
A `--local` mód ideális, helyi szimuláció: a kód pipeline-ját ellenőrzi, a
hardverről **semmit** nem állít. A részletek: `docs/SCS_QUANTUM_DESIGN.md`.

---

*Ez a kiadás valós QPU-mérésekkel készült. Minden hardverre vonatkozó
állítás a §4 táblázatából származik; minden más állítás a saját
bizonyítéki szintjével (§0, §5) együtt értendő.*
