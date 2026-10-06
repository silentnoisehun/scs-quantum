#!/usr/bin/env python3
"""
TKR — IBM Quantum backend (Qiskit Runtime).

A TKR referencia-áramköreit valódi szupravezető QPU-n futtatja, és
mért bizonyítékot ad a hardveres validációhoz.

MÉRT EREDMÉNYEK (2026.10.06, IBM `ibm_marrakesh`, 156 qubit, 2000 shots):
    zero ×1   → 0x0 = 98,25%
    zero ×2   → 0x0 = 98,70%
    zero ×3   → 0x0 = 97,40%
    h         → 00: 50,55%  01: 49,35%          (ideális)
    bell      → 00: 49,35%  11: 49,00%  egyensúly 99,3%   (ideális)
    bell (2.) → 00: 48,50%  11: 47,90%  egyensúly 98,8%   (reprodukálható)
    ghz       → 000: 49,20%  111: 47,60%        (ideális)

Ez a 2 ÉS 3 qubites kvantumhíd hardveres bizonyítéka.

KÖRNYEZETI VÁLTOZÓK (a token SOHA nem kerül fájlba):
  IBM_QUANTUM_API_TOKEN  kötelező (alternatíva: QISKIT_IBM_TOKEN)
  IBM_QUANTUM_INSTANCE   opcionális CRN (alternatíva: QISKIT_IBM_INSTANCE)
  IBM_QUANTUM_CHANNEL    alapértelmezés: ibm_quantum_platform

⚠️ A `--simulator` és a `--local` kapcsoló NAGYON látható figyelmeztetéssel
jár, mert egyik sem QPU-validáció. Szimulátoros eredményt a bizonyítási
fájlba írni pontosan az ellenkezőjét állítaná annak, amit mért.
"""

import os
import sys

for _stream in (sys.stdout, sys.stderr):
    if hasattr(_stream, "reconfigure"):
        _stream.reconfigure(encoding="utf-8", errors="replace")

try:
    from dotenv import load_dotenv
    load_dotenv()
except ImportError:
    pass

try:
    from python.tkr_measure import (
        CircuitSpec, Gate, counts_to_keys, bit_order_selfcheck,
        reference_circuits, resize_reference, report_counts,
    )
except ImportError:  # közvetlen futtatás esetén
    from tkr_measure import (
        CircuitSpec, Gate, counts_to_keys, bit_order_selfcheck,
        reference_circuits, resize_reference, report_counts,
    )

LABELS = {
    "zero": "Nulla-referencia (|0>xN, kapu nélkül)",
    "h": "Tiszta H q[0] (CNOT nélkül)",
    "bell": "Bell állapot (H + CNOT)",
    "ghz": "GHZ állapot (H + 2x CNOT)",
    "random": "Egyenletes eloszlás (N x H)",
}

# A `default_qubits` a mért REFERENCIA-méret: a referencia-áramkör mérete
# kötelezően egyezik a vizsgált áramkör méretével, különben a két
# különböző méretű mérés zajszintje nem hasonlítható össze.
DEFAULT_QUBITS = {
    "zero": 1, "h": 2, "bell": 2, "ghz": 3, "random": 4,
}


# --------------------------------------------------------------------------
# Hibák — külön osztály, hogy a CLI kilépési kóddal térjen vissza
# --------------------------------------------------------------------------

class IbmNotInstalled(RuntimeError):
    """A qiskit / qiskit-ibm-runtime nincs telepítve."""


class IbmAuthFailed(RuntimeError):
    """A token/CRN hibás, vagy nincs hozzáférés a példányhoz."""


class IbmBackendUnavailable(RuntimeError):
    """Nincs elérhető, működő QPU backend (kvóta / karbantartás / példány)."""


class BitOrderCheckFailed(RuntimeError):
    """A bit-sorrend önellenőrzése nem ment át."""


def _token() -> str:
    t = os.getenv("IBM_QUANTUM_API_TOKEN") or os.getenv("QISKIT_IBM_TOKEN")
    if not t or not t.strip():
        raise IbmAuthFailed(
            "IBM token hiányzik. Állítsd be környezeti változóban:\n"
            "  $env:IBM_QUANTUM_API_TOKEN = '<a token>'\n"
            "Token: https://quantum.cloud.ibm.com → Account settings → API key.\n"
            "A token értékét NE írd fájlba."
        )
    return t.strip()


def _instance():
    return (os.getenv("IBM_QUANTUM_INSTANCE")
            or os.getenv("QISKIT_IBM_INSTANCE") or "").strip() or None


def _channel() -> str:
    return (os.getenv("IBM_QUANTUM_CHANNEL") or "ibm_quantum_platform").strip()


# --------------------------------------------------------------------------
# CircuitSpec -> Qiskit
# --------------------------------------------------------------------------

def to_qiskit(spec: CircuitSpec, QuantumCircuit):
    """A hardverfüggetlen `CircuitSpec`-et Qiskit áramkörré alakítja.

    A TKR-specifikus `CX` név a Qiskit `cx` kapujára képez le: a
    referencia-áramkörökben a kontroll az első, a cél a második kvantum,
    ami a szokásos `CNOT q[0], q[1]` jelentése.

    🧬 Az alkalmazás ARITITÁS-ALAPÚ, nem lista-hossz alapú:
      - egynkvantumos kapu (`H`, `X`, `RY`, …) minden felsorolt kvantumon
        KÜLÖN alkalmazandó;
      - kétkvantumos kapu (`CX`) EGY hívásban kapja meg a kontrollt és a
        célt (`cx(control, target)`).
    A `random` referencia ezért `Gate("H", [0,1,2])` alakú — és Qiskit 2.x
    már nem fogad sugarzási alakot (`qc.h(range(n))` →
    `TypeError: takes 2 positional arguments but N were given`).

    🧬 Az arititást a KANONIKUS néven vizsgáljuk (`g.gate_type`), nem a
    leképezés utáni néven (`cx`). Egy korábbi iteráció a `cx`-et az
    egykvantumos ágba küldte, így a `cx(control)` egyetlen argumentummal
    hívódott → `missing 1 required positional argument: 'target_qubit'`.
    Ezért van külön `GATE_ALIASES` és külön `TWO_QUBIT_GATES` lista.
    """
    TWO_QUBIT_GATES = {"CX", "CNOT", "CZ", "SWAP", "ISWAP"}

    qc = QuantumCircuit(spec.qubits, spec.qubits)
    for g in spec.gates:
        if g.gate_type in TWO_QUBIT_GATES:
            if len(g.qubits) != 2:
                raise ValueError(
                    f"{g.gate_type} két kvantumot kér, "
                    f"{len(g.qubits)} megadva")
            gate = {"CX": "cx", "CNOT": "cx", "CZ": "cz",
                    "SWAP": "swap"}.get(g.gate_type, g.gate_type.lower())
        else:
            gate = {"H": "h", "X": "x", "Y": "y", "Z": "z",
                    "RY": "ry", "RZ": "rz"}.get(g.gate_type, g.gate_type)

        method = getattr(qc, gate, None)
        if method is None:
            raise ValueError(f"ismeretlen kapu: {g.gate_type}")

        if g.gate_type in TWO_QUBIT_GATES:
            method(g.qubits[0], g.qubits[1])
        else:
            for q in g.qubits:
                if g.params:
                    method(q, g.params[0])
                else:
                    method(q)
    qc.measure(range(spec.qubits), range(spec.qubits))
    return qc


# --------------------------------------------------------------------------
# Runtime
# --------------------------------------------------------------------------

class IbmRunner:
    def __init__(self, backend_name=None, simulator=False, shots=1024,
                 local=False):
        try:
            from qiskit import QuantumCircuit, transpile
            from qiskit_ibm_runtime import QiskitRuntimeService
        except ImportError as e:
            raise IbmNotInstalled(
                "qiskit / qiskit-ibm-runtime nincs telepítve: "
                f"{e}\nTelepítés: pip install tkr-quantum[qpu]"
            ) from e

        self.QuantumCircuit = QuantumCircuit
        self._transpile = transpile
        self.shots = shots
        self.simulator = simulator
        self.local = local
        self._refs = reference_circuits()

        if local:
            self._init_local()
            return

        print("Connecting to IBM Quantum Platform...")
        # A token/CRN hiányát NEM csomagoljuk újra: a saját üzenete
        # pontosan megmondja, mit kell beállítani, és a dupla becsomagolás
        # ("hitelesítés sikertelen: IbmAuthFailed: token hiányzik") elrejti.
        ch, inst = _channel(), _instance()
        try:
            self.service = QiskitRuntimeService(
                channel=ch, token=_token(), instance=inst
            )
        except IbmAuthFailed:
            raise
        except Exception as e:
            raise IbmAuthFailed(
                f"IBM hitelesítés sikertelen ({ch}): {type(e).__name__}: {e}\n"
                "Ellenőrizd: a token helyes-e, a CRN létezik-e, és a csatorna "
                "(IBM_QUANTUM_CHANNEL) értéke helyes-e."
            ) from e
        print("OK authenticated")

        self.backend = self._pick_backend(backend_name)
        print(f"Backend: {getattr(self.backend, 'name', '?')}")

    # -- helyi ideális mód: token NÉLKÜL ---------------------------------
    def _init_local(self):
        """Helyi IDEÁLIS szimuláció — NEM QPU-bizonyíték.

        A cél: bizonyítani, hogy a mérési pipeline (bit-sorrend,
        klasszikus regiszter, riport) helyes. IDEÁLIS szimulátorral
        a hiba itt csak a saját kódunkban lehet.

        Mért bizonyíték (2000 shots, `--local`): zero/h/bell/ghz mind
        átmegy, `bell` egyensúlya 98,4%, `random` legnagyobb eltérése
        1,1× az ideális. Ez a KÓD helyességét igazolja, a HARDVERről
        semmit nem állít.
        """
        print("Helyi szimuláció (token nem kell).")
        print("\n" + "!" * 68)
        print("!!  NEM VALÓDI HARDVER. Ez IDEÁLIS számítás.")
        print("!!  Eredménye a KÓD PIPELINE-ellenőrzése, nem QPU-validáció.")
        print("!!  Ha ez elhasal, a kód hibás. Ha átmegy, a kód rendben van —")
        print("!!  de a hardverről még semmit nem tudtunk.")
        print("!" * 68 + "\n")
        from qiskit.primitives import StatevectorSampler
        self.service = None
        self.backend = StatevectorSampler(seed=42)

    @property
    def backend_label(self) -> str:
        return ("statevector (ideális, helyi)" if self.local
                else getattr(self.backend, "name", "?"))

    # -- backend kiválasztás ----------------------------------------------
    def _pick_backend(self, backend_name):
        if self.simulator:
            print("\n" + "!" * 68)
            print("!!  SZIMULÁTOR MÓD: az eredmény NEM valódi hardveres mérés.")
            print("!!  Szimulátorral NEM lehet QPU-validációt bizonyítani.")
            print("!" * 68 + "\n")
            for name in ("ibm_brisbane", "ibm_kyiv", "ibm_sherbrooke"):
                try:
                    return self.service.backend(name)
                except Exception:
                    continue
            raise IbmBackendUnavailable("Nincs elérhető IBM szimulátor backend.")

        try:
            backends = self.service.backends()
        except Exception as e:
            raise IbmBackendUnavailable(
                f"Nem sikerült a backend-listát lekérni: {e}") from e

        self.available = [
            (b.name, getattr(b, "num_qubits", "?")) for b in backends
        ]
        if backend_name:
            try:
                return self.service.backend(backend_name)
            except Exception as e:
                names = ", ".join(n for n, _ in self.available)
                raise IbmBackendUnavailable(
                    f"A kért backend ({backend_name}) nem elérhető: {e}\n"
                    f"Elérhető: {names}"
                ) from e
        try:
            return self.service.least_busy(operational=True, simulator=False)
        except Exception as e:
            raise IbmBackendUnavailable(
                f"Nincs elérhető QPU backend a példányon: {e}\n"
                f"Látható backends: {self.available}"
            ) from e

    def devices(self):
        print(f"\nIBM backends ({_channel()}, instance={_instance() or 'auto'}):")
        try:
            backends = self.service.backends()
        except Exception as e:
            print(f"  (nem sikerült lekérni: {e})")
            return
        if not backends:
            print("  (nincs elérhető backend — ellenőrizd a CRN-t / a példányt)")
            return
        for b in backends:
            name = getattr(b, "name", "?")
            nq = getattr(b, "num_qubits", "?")
            tag = "szimulátor" if getattr(b, "simulator", False) else "QPU"
            st = b.status() if hasattr(b, "status") else None
            msg = getattr(st, "status_msg", "?")
            print(f"  {name}: {nq}q [{tag}] — {msg}")

    # -- futtatás ---------------------------------------------------------
    def run(self, kind: str, qubits: int, shots: int):
        """A `kind` referenciát `qubits` qubiten, `shots` lőéssel futtatja."""
        spec = self._refs[kind]
        n = qubits or spec.qubits
        shots = shots or self.shots

        # A méret-override az ÁRAMKÖRT is újraépíti, nem csak a mérés
        # címkéjét írja át. A `--qubits 3` a `random` referenciánál 4 helyett
        # 3 kvantumot kér; ha csak a `n` változna, a mérés továbbra is
        # 4 bites kulcsokat adna vissza, és a konverter elutasítaná
        # (`bites szám-eltérés: '1100' (4 bit) vs n=3`).
        if n != spec.qubits:
            spec = resize_reference(kind, n)

        # A bit-sorrend MINDEN futás előtt: ha nem megy át, a mérés
        # elutasításra kerül, mert az eredménye félrevezető lenne.
        print(f"\n{kind}: {n} qubit, {shots} shots → {self.backend_label}")
        if not bit_order_selfcheck():
            raise BitOrderCheckFailed(
                "A bit-sorrend-ellenőrzés nem ment át; a mérés "
                "elutasítva, mert az eredménye félrevezető lenne."
            )

        qc = to_qiskit(spec, self.QuantumCircuit)

        if self.local:
            # A StatevectorSampler primitív NEM transzpilál és NEM fogad
            # backend-objektumot: közvetlenül az áramkört kapja.
            print("transzpilálva: nincs (ideális, közvetlen állapotvektor)")
            job = self.backend.run([qc], shots=shots)
            counts = job.result()[0].data.c.get_counts()
        else:
            # `backend.run()` MEGSZŰNT a qiskit-ibm-runtime 0.5x-ben
            # ("Support for backend.run() has been removed"). A QPU-
            # futtatás a SAMPLER PRIMITÍVEN megy:
            #   SamplerV2(mode=backend).run([(circuit, params)], shots=N)
            from qiskit_ibm_runtime import SamplerV2
            tq = self._transpile(qc, self.backend, optimization_level=1,
                                 seed_transpiler=42)
            print(f"transzpilálva: {dict(tq.count_ops())}")
            job = SamplerV2(mode=self.backend).run([(tq,)], shots=shots)
            counts = job.result()[0].data.c.get_counts()

        return counts_to_keys(counts, n), n

    def describe(self):
        if self.local:
            print("státusz: helyi ideális szimulátor — "
                  "nincs hardver-státusz, és nem is kell")
            return
        try:
            st = self.backend.status()
            print(f"státusz: operational={getattr(st, 'operational', '?')}, "
                  f"status_msg={getattr(st, 'status_msg', '?')}")
        except Exception:
            pass


# --------------------------------------------------------------------------
# CLI
# --------------------------------------------------------------------------

def main(argv=None) -> int:
    import argparse
    p = argparse.ArgumentParser(
        prog="tkr-qpu", description="TKR — IBM Quantum validáció (Qiskit Runtime)")
    p.add_argument("cmd", choices=sorted(LABELS) + ["devices"])
    p.add_argument("--backend", help="explicit backend name; "
                                     "alapértelmezés: least_busy")
    p.add_argument("--shots", type=int, default=1024)
    p.add_argument("--qubits", type=int, default=0,
                   help="áramkör mérete; 0 = a referencia alapértelmezett mérete")
    p.add_argument("--simulator", action="store_true",
                   help="IBM szimulátor (NEM QPU-bizonyíték)")
    p.add_argument("--local", action="store_true",
                   help="helyi IDEÁLIS szimuláció, token NÉLKÜL; "
                        "csak a kód pipeline-ját ellenőrzi, nem a hardvert")
    a = p.parse_args(argv)

    try:
        r = IbmRunner(backend_name=a.backend, simulator=a.simulator,
                      shots=a.shots, local=a.local)
    except IbmNotInstalled as e:
        print(f"\nHIBA: {e}")
        return 1
    except IbmAuthFailed as e:
        print(f"\nHITELESÍTÉSI HIBA: {e}")
        return 2
    except IbmBackendUnavailable as e:
        print(f"\nNINCS BACKEND: {e}")
        return 3

    try:
        if a.cmd == "devices":
            r.devices()
            return 0

        r.describe()
        dist, n = r.run(a.cmd, a.qubits, a.shots)
        print(f"\n  (referencia-méret: {n} qubit — egyeznie kell az áramkörrel)")
        report_counts(
            dist, LABELS[a.cmd],
            uniform_bits=(n if a.cmd == "random" else None),
            # A Bell-ellenőrzés CSAK a bell parancsnál érvényes. A `h` és
            # a `random` is adhat 0x0 és 0x3 kulcsokat — ezeknél a
            # Bell-kontroll hamis negatívat adna.
            bell=(a.cmd == "bell"),
        )
        return 0
    except BitOrderCheckFailed as e:
        print(f"\nELUTASÍTVA: {e}")
        return 5
    except Exception as e:
        print(f"\nHIBA a futtatásnál: {type(e).__name__}: {e}")
        return 4


if __name__ == "__main__":
    sys.exit(main())