"""
Petri net (PNML) import/export and pm4py interoperability.
"""
from typing import Any, Tuple

# Reuse the alignment bindings' PetriNet type so a net produced here is accepted
# directly by align_trace / align_variants / compute_fitness.
from .bindings.conformance.case_centric.alignments import PetriNet as PetriNet

def import_pnml(path: str) -> PetriNet:
    """Import a Petri net from a PNML file."""
    ...

def export_pnml(net: PetriNet, path: str) -> None:
    """Export a Petri net dict to a PNML file."""
    ...

def to_pm4py(net: PetriNet) -> Tuple[Any, Any, Any]:
    """Convert a Petri net dict to a pm4py net; returns (net, initial_marking, final_marking).

    Uses the first of `final_markings` as the pm4py final marking.
    """
    ...

def from_pm4py(pn: Any, initial_marking: Any = None, final_marking: Any = None) -> PetriNet:
    """Convert a pm4py net (with optional markings) to a Petri net dict.

    Fresh UUIDs are assigned to places/transitions (pm4py names are not UUIDs);
    transition labels are preserved.
    """
    ...
