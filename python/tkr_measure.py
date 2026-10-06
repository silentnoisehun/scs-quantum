#!/usr/bin/env python3
"""
TKR — közös mérési és kódolási réteg (hardverfüggetlen).

Ez a modul NEM tartalmaz gyártótól függő kódot: egyetlen kvantumszállító
vagy hálózati réteget sem. Kizárólag azokat a részeket tartalmazza,
amelyek MINDEN kvantumhardverrel érvényesek, és amelyek a QPU-mérések
értelmezéséhez kellenek:

  - `WavePacket`      — a TKR hullámcsomag (A, γ, f, φ)
  - `CircuitSpec`      — hardverfüggetlen áramkör-leírás
  - `counts_to_keys` — bitstring → `{"0x..": db}` forma
  - `bit_order_selfcheck` — a bit-sorrend IGAZOLÁSA méréssel
  - `report_counts`    — a mérési eloszlás kiírása + Bell-ellenőrzés

A bit-sorrend és a Bell-feltétel két korábban kimutatott hiba helyét
őrzi; lásd a docstringeket. Ne töröld ki őket „egyszerűsítés" kedvéért.
"""

import sys

for _stream in (sys.stdout, sys.stderr):
    if hasattr(_stream, "reconfigure"):
        _stream.reconfigure(encoding="utf-8", errors="replace")

from dataclasses import dataclass, field
from typing import Dict, Iterable, List, Optional

__all__ = [
    "WavePacket", "CircuitSpec", "Gate", "Counts",
    "counts_to_keys", "bit_order_selfcheck", "report_counts",
    "reference_circuits", "resize_reference",
]


Counts = Dict[str, int]


# --------------------------------------------------------------------------
# A TKR hullámcsomag
# --------------------------------------------------------------------------

@dataclass
class WavePacket:
    """TKR WavePacket: psi(t) = A * exp(-gamma*t) * cos(2*pi*f*t + phi)"""

    amplitude: float    # A - intenzitás
    gamma: float        # γ - csillapítás
    frequency: float    # f - tartalom (frekvencia)
    phase: float        # φ - kontextus (fázis)

    def to_dict(self) -> Dict[str, float]:
        return {
            "amplitude": self.amplitude,
            "gamma": self.gamma,
            "frequency": self.frequency,
            "phase": self.phase,
        }


# --------------------------------------------------------------------------
# Hardverfüggetlen áramkör-leírás
# --------------------------------------------------------------------------

@dataclass
class Gate:
    """Egyetlen kvantumkapu. A `qubits` a célkvantumokat adja meg."""

    gate_type: str
    qubits: List[int] = field(default_factory=list)
    params: List[float] = field(default_factory=list)

    @classmethod
    def new(cls, gate_type: str, qubits: List[int]) -> "Gate":
        """Paraméter nélküli kapu (H, CNOT, …)."""
        return cls(gate_type, list(qubits), [])

    @classmethod
    def with_param(cls, gate_type: str, qubits: List[int],
                    param: float) -> "Gate":
        """Egyparaméteres forgó kapu (RY, RZ, …)."""
        return cls(gate_type, list(qubits), [float(param)])


@dataclass
class CircuitSpec:
    """Hardverfüggetlen áramkör.

    A `decoherence` NEM egy kapu: a γ (csillapítás) paramétert hordozza,
    amit a jelenlegi sémában posztprocesszálunk. A korábbi `DECOHERENCE`
    „kapu" nem létező IR utasítás volt.
    """

    qubits: int
    gates: List[Gate] = field(default_factory=list)
    decoherence: Optional[float] = None

    def op_counts(self) -> Dict[str, int]:
        counts: Dict[str, int] = {}
        for g in self.gates:
            counts[g.gate_type] = counts.get(g.gate_type, 0) + 1
        return counts

    def uses_gate(self, gate_type: str) -> bool:
        return any(g.gate_type == gate_type for g in self.gates)


# --------------------------------------------------------------------------
# Referencia-áramkörök — a QPU-validáció mérőszerszámai
# --------------------------------------------------------------------------
#
# Ezek NEM a TKR kódolás, hanem a HARDVER referenciamérései. Egy
# referencia-áramkör ismert elvárt eredménnyel rendelkezik; ha a
# hardver nem adja, a hiba a hardverben (vagy a mérési útban) van.
#
# A sorrend NEM véletlen: a REFERENCIA megy először, mindig azonos
# méretű áramkörrel, mint amit vizsgál.

def ref_zero(n: int = 1) -> CircuitSpec:
    """|0> referencia: egyetlen kapu nélkül, minden qubit biztosan 0.

    Elvárt: a 0x0 kimenet 100% (a mérési alapszint). Ha nem az, akkor
    a hardver mérési hibája mérhető — és CSAK ezután lehet a többi
    eredményt zajnak tekinteni.
    """
    return CircuitSpec(qubits=n)


def ref_h(n: int = 2) -> CircuitSpec:
    """Tiszta H q[0] — 50/50 kontroll, CNOT nélkül.

    Elvárt: 00 és 01 egyenlő súlyú (≈50/50), 10 és 11 elenyésző.
    """
    return CircuitSpec(qubits=n, gates=[Gate("H", [0])])


def ref_bell(n: int = 2) -> CircuitSpec:
    """Bell: H q[0] + CNOT q[0]->q[1]. Elvárt: 00 ≈ 11 ≈ 50%."""
    return CircuitSpec(qubits=n, gates=[Gate("H", [0]), Gate("CX", [0, 1])])


def ref_ghz(n: int = 3) -> CircuitSpec:
    """GHZ: H q[0] + CNOT q[0]->q[1] + CNOT q[1]->q[2]. Elvárt 000 ≈ 111 ≈ 50%."""
    return CircuitSpec(
        qubits=n,
        gates=[Gate("H", [0]), Gate("CX", [0, 1]), Gate("CX", [1, 2])],
    )


def ref_random(n: int = 4) -> CircuitSpec:
    """Egyenletes eloszlás: H minden qubiten. Elvárt 100/2^n kimenetenként."""
    return CircuitSpec(qubits=n, gates=[Gate("H", list(range(n)))])


def reference_circuits() -> Dict[str, CircuitSpec]:
    """A teljes referencia-szett, a mért referencia-mérettel együtt."""
    return {
        "zero": ref_zero(1),
        "h": ref_h(2),
        "bell": ref_bell(2),
        "ghz": ref_ghz(3),
        "random": ref_random(4),
    }


def resize_reference(kind: str, n: int) -> CircuitSpec:
    """Ugyanaz a referencia-áramkör `n` kvantumon.

    🧬 A méret-override NEM csak a címkét írja át: az áramkört is újra
    kell építeni. Ha a mérési réteg 3 kvantum, de az áramkör még mindig
    4 `H`-t tartalmaz, a mérés 4 bites kulcsokat ad vissza, és a
    bit-konverter elutasítja (`bites szám-eltérés: '1100' (4 bit) vs
    n=3`) — vagy ami rossabb, csendden elfogadná.

    A `zero` bármely méretre érvényes (nincs kapuja); a `h`, `bell` és
    `ghz` viszont EGYETLEN q[0]-ra érvényes, ezért az átméretezés
    minimum 2 (a `ghz` minimum 3) kvantumot követel.
    """
    if kind == "zero":
        return ref_zero(n)
    if kind == "h":
        return ref_h(max(n, 2))
    if kind == "bell":
        return ref_bell(max(n, 2))
    if kind == "ghz":
        return ref_ghz(max(n, 3))
    if kind == "random":
        return ref_random(max(n, 1))
    raise ValueError(f"ismeretlen referencia: {kind}")


# --------------------------------------------------------------------------
# Eredmény-normalizálás
# --------------------------------------------------------------------------

def counts_to_keys(counts: Dict[str, int], n_bits: int) -> Counts:
    """Bitstring-titkeket `{"0x..": darabszám}` formára hozza.

    A BIT-SORREND A LÉNYEG, és itt nincs átfordítás:

      - a Qiskit a klasszikus regisztert balról jobbra írja ki, nagyobb
        sorszámmal ELŐBB, tehát a kulcs `c[n-1] ... c[1] c[0]` — a `c[0]`
        (jobban) már a LEGKISEBB bit;
      - a `0x3` kulcs pedig azt jelenti, hogy q[0]=1 ÉS q[1]=1, vagyis
        szintén a legkisebb sorszám a legkisebb bit.

    A két formátum tehát azonos sorrendet használ: NEM kell megfordítani.

    🧬 A mérési tanulság: egy KORÁBBI implementáció megfordította a
    biteket, és ez HANGOS hibát nem adott — a Bell-állapot szimmetrikus,
    tehát a fordított bitek rajta nem látszanak. Csak az ASIMMETRIKUS
    áramkörön (pl. az `X q[1]` próbán, vagy a fordított Bell-en) jelentkezik,
    ahol pedig tévesen HARDVERHIBÁT imitál. Ezért létezik a
    `bit_order_selfcheck`.
    """
    dist: Counts = {}
    for bits, count in counts.items():
        bits = str(bits).replace(" ", "")
        if len(bits) < n_bits:
            bits = bits.rjust(n_bits, "0")      # balra nullával kitöltve
        if len(bits) > n_bits:
            raise ValueError(
                f" bites szám-eltérés: {bits!r} ({len(bits)} bit) "
                f"vs n={n_bits}"
            )
        key = "0x%x" % int(bits, 2)            # c[0] = LSB, nincs fordítás
        dist[key] = dist.get(key, 0) + count
    return dist


def bit_order_selfcheck(verbose: bool = True) -> bool:
    """Bizonyítja a bit-sorrendet egy ISMERETT, egyirányú próbával.

    A próba: `X q[1]` esetén a kulcs `c1c0 = "10"`, ami q[1]=1-t és
    q[0]=0-t jelent → `0x2`. Ez a konverzió helyességének eldöntő
    tesztje.

    🧬 FONTOS: egy SZIMMETRIKUS eloszlás (a Bell `00`/`11` párja) NEM
    alkalmas erre — abból a bit-sorrend nem olvasható ki, tehát a
    fordított konverzió is átmenne rajta. Csak az egyirányú próba
    különbözteti meg a két konverziót.
    """
    # X q[1] -> c1c0 = '10' -> q1=1, q0=0 -> 0x2
    probe = {"10": 100}
    right = counts_to_keys(probe, 2)
    ok = right == {"0x2": 100}
    if verbose:
        print(f"bit-sorrend önellenőrzés: X q[1] -> "
              f"{sorted(right)[0]} (várt 0x2) {'✓' if ok else '✗ HIBA'}")
        if not ok:
            print("  A konverzió megfordítja a biteket — az aszimmetrikus")
            print("  áramkörök eredménye a másik ágra cserezne át,")
            print("  ami tévesen HARDVERHIBÁT imitál.")
    return ok


# --------------------------------------------------------------------------
# Riport
# --------------------------------------------------------------------------

def report_counts(dist: Counts, label: str, uniform_bits: Optional[int] = None,
                  bell: bool = False) -> None:
    """Kiírja a mérési eloszlást és az ellenőrző számításokat.

    `uniform_bits` — ha megadott, az eloszlás elvártan egyenletes ezen a
    bites számon (a hívó adja át, NEM a megjelent kulcsokból becsüljük:
    ha a szerver nem küldi az összes kimenetet, a legnagyobb kulcs
    kevesebb bitet hordozhat, és az ideális érték hamisan magasabb lenne).

    `bell=True` — a Bell-ellenőrzést CSAK a Bell-áramkörnél szabad
    bekapcsolni.

    🧬 A `bell` flag NEM opcionális kényelmi elem: a Bell-ellenőrzés
    korábban a `0x0` ÉS `0x3` kulcs jelenlétéből indult, ami hamis
    pozitív. A tiszta `H q[0]` mérés is adhatja mindkét kulcsot
    (pl. 50,55% / 0,05%), és a riport ilyenkor azt írta, hogy „a CNOT
    nem működik" — egy működő, ideális mérésre. Mért bizonyíték: a
    `h` parancs a flag nélkül `⚠️ nem ideális`-t írt, a flag-gyel
    tisztán 50,55/49,35-öt adott ugyanarról a mérésről.

    🧬 Általános szabály, amit ez a függvény követ: egy automatikus
    értékelőnek meg kell tudnia különböztetni a JÓ mintát a ROSSZtól.
    Ha a jó mintát is elutasítja, a feltétel túl széles; ha a rossz
    mintát is átengedi, túl szűk.
    """
    if not dist:
        print(f"\n{label} — a mérés nem adott számottevő eredményt")
        return

    total = sum(dist.values())
    print(f"\n{label} — mérési eloszlás ({len(dist)} mért kimenet):")
    for k in sorted(dist, key=lambda x: -dist[x]):
        print(f"    {k:<8} {100 * dist[k] / total:6.2f}%")

    if uniform_bits:
        ideal = 100.0 / (2 ** uniform_bits) if uniform_bits else 100.0
        worst = max(dist, key=dist.get)
        pct = 100 * dist[worst] / total
        print(f"\n    Elvárt egyenletes ({uniform_bits} kvantum): "
              f"{ideal:.2f}% kimenetenként")
        print(f"    Legnagyobb eltérés: {worst} = {pct:.2f}% "
              f"({pct / ideal:.1f}× az ideális)")
        if pct / ideal > 3:
            print("    ⚠️ erős egyenlőtlenség az ELVÁRT egyenletességhez képest.")
            print("    ⚠️ HARDVERZAJ CSAK IGAZOLT REFERENCIA UTÁN állítható:")
            print("       a |0> referencia (minden qubit biztosan 0) adja a")
            print("       mérési alapszintet. Enélkül a kimenet lehet transzpilálási")
            print("       hiba vagy bit-sorrend probléma is.")
        return

    if bell and "0x0" in dist and "0x3" in dist:
        a, b = dist["0x0"], dist["0x3"]
        print("\n    Bell-ellenőrzés:")
        print(f"      00 : {100 * a / total:5.2f}%")
        print(f"      11 : {100 * b / total:5.2f}%")
        if a > 0 and b > 0:
            ar = min(a, b) / max(a, b)
            print(f"      egyensúly: {100 * ar:.1f}%")
            if ar > 0.8:
                print("      ✓ közel ideális — a H és a CNOT működik")
            else:
                print("      ⚠️ nem ideális. A 00/11 arány eltérése önmagában")
                print("         NEM bizonyít hardverzajat: lehet transzpilálási")
                print("         hiba, bit-sorrend, vagy kalibráció is.")
                print("         Elkülönítéshez |0> referencia kell (zero parancs).")
        err = sum(v for k, v in dist.items() if k in ("0x1", "0x2"))
        print(f"      01+10 (zaj): {100 * err / total:.2f}%")