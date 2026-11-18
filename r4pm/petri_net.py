"""
Petri nets: PNML import/export and pm4py interoperability.

A Petri net is a JSON-compatible dict in the shape the bindings use:

    {
        "places": [{"name": "p0"}, {"name": "p1"}],
        "transitions": [
            {"label": "a", "name": "t0", "inputs": [[0, 1]], "outputs": [[1, 1]]},
            {"label": None, "inputs": [[1, 1]], "outputs": []},   # silent
        ],
        "initial_marking": {"0": 1},
        "final_marking": {"1": 1},
    }

Places and transitions are identified by their index in their list. `inputs`/`outputs` list
`[place index, arc weight]` pairs, and markings map place indices (as strings) to token counts.
`name` is an optional external identifier (e.g., the PNML or pm4py name). A net with both
markings is an accepting Petri net, which alignments, precision and token-based replay take.
"""
from __future__ import annotations

import json
from typing import TYPE_CHECKING, Any, Dict, List, Optional, Tuple, Union

from . import handles
from .bindings._types import AcceptingPetriNet, Place, Transition
from .r4pm import export_pnml_rs, import_item_from_json, import_pnml_rs, item_to_json

if TYPE_CHECKING:
    from typing_extensions import NotRequired, TypedDict
else:
    from typing import TypedDict

__all__ = [
    "Place",
    "Transition",
    "Marking",
    "PetriNet",
    "AcceptingPetriNet",
    "import_pnml",
    "export_pnml",
    "to_pm4py",
    "from_pm4py",
    "to_handle",
    "from_handle",
    "is_pm4py_net",
]

Marking = Dict[str, int]
"""Tokens per place, keyed by the place's index as a string (e.g., `{"0": 1}`)."""


class PetriNet(TypedDict):
    """A Petri net, with optional initial and final marking.

    With both markings, it is an `AcceptingPetriNet`, as alignments, precision and token-based
    replay take.
    """

    places: List[Place]
    transitions: List[Transition]
    initial_marking: NotRequired[Optional[Marking]]
    final_marking: NotRequired[Optional[Marking]]


AnyNet = Union[PetriNet, AcceptingPetriNet]


def import_pnml(path: str) -> PetriNet:
    """Import a Petri net from a PNML file.

    Markings missing from the file are `None`. A node's PNML id becomes its `name`.
    """
    return json.loads(import_pnml_rs(path))


def export_pnml(net: AnyNet, path: str) -> None:
    """Export a Petri net dict (with optional markings) to a PNML file."""
    export_pnml_rs(json.dumps(net), path)


def to_handle(net: AnyNet, item_id: Optional[str] = None) -> handles.AcceptingPetriNet:
    """Store an accepting Petri net (both markings set) in the registry.

    Args:
        net: The net.
        item_id: Optional ID. Without one, the handle owns the item.
    """
    new_id = import_item_from_json("AcceptingPetriNet", json.dumps(net), item_id)
    return handles.AcceptingPetriNet(new_id, owned=item_id is None)


def from_handle(handle: str) -> PetriNet:
    """The Petri net dict of a `PetriNet` or `AcceptingPetriNet` registry item."""
    return json.loads(item_to_json(handle))


def _require_pm4py() -> Any:
    try:
        from pm4py.objects.petri_net import obj
        from pm4py.objects.petri_net.utils import petri_utils
    except ImportError as e:
        raise ImportError(
            "pm4py is required for this function. Install with: pip install r4pm[pm4py]"
        ) from e
    return obj, petri_utils


def is_pm4py_net(obj: Any) -> bool:
    """Whether `obj` is a pm4py Petri net (checked without importing pm4py)."""
    return type(obj).__module__.startswith("pm4py") and hasattr(obj, "transitions")


def to_pm4py(net: AnyNet) -> Tuple[Any, Any, Any]:
    """Convert a Petri net dict to pm4py.

    Places and transitions keep their `name` (or get `p<i>`/`t<i>`), and silent transitions have
    label `None`.

    Returns:
        `(net, initial_marking, final_marking)` as pm4py objects; a missing marking is empty.
    """
    obj, petri_utils = _require_pm4py()
    pn = obj.PetriNet("net")
    places = []
    for i, p in enumerate(net["places"]):
        place = obj.PetriNet.Place(p.get("name") or f"p{i}")
        pn.places.add(place)
        places.append(place)
    for i, t in enumerate(net["transitions"]):
        tr = obj.PetriNet.Transition(t.get("name") or f"t{i}", t.get("label"))
        pn.transitions.add(tr)
        for p, weight in t.get("inputs", []):
            petri_utils.add_arc_from_to(places[p], tr, pn, weight=weight)
        for p, weight in t.get("outputs", []):
            petri_utils.add_arc_from_to(tr, places[p], pn, weight=weight)

    def marking(m: Optional[Marking]) -> Any:
        result = obj.Marking()
        for p, count in (m or {}).items():
            result[places[int(p)]] = count
        return result

    return pn, marking(net.get("initial_marking")), marking(net.get("final_marking"))


def from_pm4py(pn: Any, initial_marking: Any = None, final_marking: Any = None) -> PetriNet:
    """Convert a pm4py Petri net (with optional markings) to a Petri net dict.

    Places and transitions are ordered by name, which keeps the result (and alignments computed on
    it) the same across runs. pm4py names become `name`, and labels are kept (`None` is silent).
    """
    obj, _ = _require_pm4py()
    places = sorted(pn.places, key=lambda p: str(p.name))
    place_index = {p: i for i, p in enumerate(places)}
    transitions = sorted(pn.transitions, key=lambda t: str(t.name))

    def arcs(arc_set: Any, end: str) -> List[Any]:
        weights: Dict[int, int] = {}
        for a in arc_set:
            node = getattr(a, end)
            if isinstance(node, obj.PetriNet.Place):
                i = place_index[node]
                weights[i] = weights.get(i, 0) + int(getattr(a, "weight", 1))
        # Lists, not tuples, as in the JSON the bindings return
        return [[p, w] for p, w in sorted(weights.items())]

    def marking(m: Any) -> Optional[Marking]:
        if m is None:
            return None
        return {str(place_index[p]): int(c) for p, c in m.items() if c}

    return {
        "places": [{"name": str(p.name)} for p in places],
        "transitions": [
            {
                "label": t.label,
                "name": str(t.name),
                "inputs": arcs(t.in_arcs, "source"),
                "outputs": arcs(t.out_arcs, "target"),
            }
            for t in transitions
        ],
        "initial_marking": marking(initial_marking),
        "final_marking": marking(final_marking),
    }
