"""
Petri net (PNML) import/export and pm4py interoperability.

A Petri net is a JSON-compatible dict with places/transitions keyed by UUID;
a transition label of None marks a silent transition.
"""
from __future__ import annotations

import json
from typing import Any, Dict
from uuid import uuid4

from .r4pm import import_pnml_rs, export_pnml_rs

# Runtime alias; the rich TypedDict is defined in petri_net.pyi for type checkers.
PetriNet = Dict[str, Any]


def import_pnml(path: str) -> "PetriNet":
    """Import a Petri net from a PNML file."""
    return json.loads(import_pnml_rs(path))


def export_pnml(net: "PetriNet", path: str) -> None:
    """Export a Petri net dict to a PNML file."""
    export_pnml_rs(json.dumps(net), path)


def _require_pm4py():
    try:
        from pm4py.objects.petri_net.obj import PetriNet as Pm4pyPetriNet, Marking
        from pm4py.objects.petri_net.utils import petri_utils
    except ImportError as e:
        raise ImportError(
            "pm4py is required for Petri net conversion. Install with: pip install pm4py"
        ) from e
    return Pm4pyPetriNet, Marking, petri_utils


def to_pm4py(net: "PetriNet"):
    """Convert a Petri net dict to a pm4py net; returns (net, initial_marking, final_marking).

    Uses the first of `final_markings` as the pm4py final marking.
    """
    Pm4pyPetriNet, Marking, petri_utils = _require_pm4py()

    pn = Pm4pyPetriNet("net")
    places = {}
    for pid in net.get("places", {}):
        p = Pm4pyPetriNet.Place(pid)
        pn.places.add(p)
        places[pid] = p

    transitions = {}
    for tid, t in net.get("transitions", {}).items():
        tr = Pm4pyPetriNet.Transition(tid, t.get("label"))
        pn.transitions.add(tr)
        transitions[tid] = tr

    for arc in net.get("arcs", []):
        from_to = arc["from_to"]
        src_id, tgt_id = from_to["nodes"]
        weight = arc.get("weight", 1)
        if from_to["type"] == "PlaceTransition":
            petri_utils.add_arc_from_to(places[src_id], transitions[tgt_id], pn, weight=weight)
        else:  # TransitionPlace
            petri_utils.add_arc_from_to(transitions[src_id], places[tgt_id], pn, weight=weight)

    im = Marking()
    for pid, count in (net.get("initial_marking") or {}).items():
        im[places[pid]] = count

    fm = Marking()
    finals = net.get("final_markings") or []
    if finals:
        for pid, count in finals[0].items():
            fm[places[pid]] = count

    return pn, im, fm


def from_pm4py(pn, initial_marking=None, final_marking=None) -> "PetriNet":
    """Convert a pm4py net (with optional markings) to a Petri net dict.

    Fresh UUIDs are assigned to places/transitions (pm4py names are not UUIDs);
    transition labels are preserved.
    """
    Pm4pyPetriNet = _require_pm4py()[0]

    place_id: Dict[Any, str] = {}
    places: Dict[str, Any] = {}
    for p in pn.places:
        pid = str(uuid4())
        place_id[p] = pid
        places[pid] = {"id": pid}

    trans_id: Dict[Any, str] = {}
    transitions: Dict[str, Any] = {}
    for t in pn.transitions:
        tid = str(uuid4())
        trans_id[t] = tid
        transitions[tid] = {"id": tid, "label": t.label}

    arcs = []
    for a in pn.arcs:
        weight = getattr(a, "weight", 1)
        if isinstance(a.source, Pm4pyPetriNet.Place):
            arcs.append({
                "from_to": {
                    "type": "PlaceTransition",
                    "nodes": [place_id[a.source], trans_id[a.target]],
                },
                "weight": weight,
            })
        else:
            arcs.append({
                "from_to": {
                    "type": "TransitionPlace",
                    "nodes": [trans_id[a.source], place_id[a.target]],
                },
                "weight": weight,
            })

    return {
        "places": places,
        "transitions": transitions,
        "arcs": arcs,
        "initial_marking": (
            {place_id[p]: int(c) for p, c in initial_marking.items()}
            if initial_marking
            else None
        ),
        "final_markings": (
            [{place_id[p]: int(c) for p, c in final_marking.items()}]
            if final_marking
            else None
        ),
    }


__all__ = ["PetriNet", "import_pnml", "export_pnml", "to_pm4py", "from_pm4py"]
