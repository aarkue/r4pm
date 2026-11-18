"""
Typed handles for items in the r4pm registry.

A handle is the registry ID of an item (it subclasses `str`, so it works wherever an ID string
is accepted) together with the item's type. Handles returned by r4pm own their item: the item is
removed from the registry when the handle is closed, used as a context manager, or garbage
collected. Handles created for an explicitly chosen ID (e.g., `import_item(..., item_id="log")`)
or with `Handle(id)` do not own their item.

    with r4pm.import_item("EventLog", "log.xes") as log:
        print(log.kind, bindings.num_variants(log))
    # the log is removed from the registry here
"""
from __future__ import annotations

import json
from typing import TYPE_CHECKING, Any, Dict, Optional, Type

from . import r4pm as _native

if TYPE_CHECKING:
    from typing_extensions import Self

__all__ = [
    "Handle",
    "OCEL",
    "SlimLinkedOCEL",
    "IndexLinkedOCEL",
    "EventLog",
    "EventLogActivityProjection",
    "TabularSource",
    "PetriNet",
    "AcceptingPetriNet",
    "ProcessTree",
    "ObjectCentricPetriNet",
    "OCDeclareModel",
    "ReflowResult",
    "AnnotatedOcpn",
    "wrap",
]


class Handle(str):
    """Registry ID of an item, with its type.

    Args:
        item_id: The registry ID.
        owned: Whether to remove the item from the registry when this handle is closed or
            garbage collected.
    """

    kind_name: str = ""
    _owned: bool
    _closed: bool

    def __new__(cls, item_id: str, owned: bool = False) -> Self:
        handle = super().__new__(cls, item_id)
        handle._owned = owned
        handle._closed = False
        return handle

    @property
    def id(self) -> str:
        """The registry ID as a plain string."""
        return str.__str__(self)

    @property
    def kind(self) -> str:
        """The item's type name (e.g., `"EventLog"`), looked up in the registry if unknown."""
        if self.kind_name:
            return self.kind_name
        return _native.item_kind(self.id) or ""

    @property
    def owned(self) -> bool:
        """Whether this handle removes its item when closed."""
        return self._owned

    @property
    def closed(self) -> bool:
        """Whether `close` was called."""
        return self._closed

    def exists(self) -> bool:
        """Whether the registry still holds the item."""
        return _native.item_kind(self.id) is not None

    def keep(self) -> Self:
        """Stop owning the item, so it stays in the registry until removed explicitly."""
        self._owned = False
        return self

    def close(self) -> None:
        """Remove the item from the registry (also for handles that do not own it)."""
        if not self._closed:
            self._closed = True
            _native.remove_item(self.id)

    def convert(self, target_type: str, new_item_id: Optional[str] = None) -> "Handle":
        """Convert the item to another type, returning a handle to the new item."""
        new_id = _native.convert_item(self.id, target_type, new_item_id)
        return wrap(new_id, target_type, owned=new_item_id is None)

    def export(self, path: str) -> None:
        """Export the item to a file, in the format the path's extension names."""
        _native.export_item(self.id, path)

    def to_json(self) -> Any:
        """The item in its JSON form (a full serialization for logs)."""
        return json.loads(_native.item_to_json(self.id))

    def to_df(self) -> Any:
        """The item as Polars DataFrame(s), see `r4pm.item_to_df`."""
        return _native.item_to_df(self.id)

    def __enter__(self) -> Self:
        return self

    def __exit__(self, *exc: Any) -> None:
        self.close()

    def __del__(self) -> None:
        if getattr(self, "_owned", False) and not getattr(self, "_closed", True):
            try:
                _native.remove_item(self.id)
            except Exception:
                # The extension module may already be torn down at interpreter exit
                pass

    def __repr__(self) -> str:
        kind = type(self).__name__ if self.kind_name else "Handle"
        return f"{kind}({self.id!r})"

    def __reduce__(self) -> Any:
        # Pickling copies the ID only; the copy must not remove the item a second time
        return (type(self), (self.id, False))


class OCEL(Handle):
    """Handle to an `OCEL` (object-centric event log)."""

    kind_name = "OCEL"


class SlimLinkedOCEL(Handle):
    """Handle to a `SlimLinkedOCEL` (compact, linked object-centric event log)."""

    kind_name = "SlimLinkedOCEL"


class IndexLinkedOCEL(Handle):
    """Handle to an `IndexLinkedOCEL` (indexed object-centric event log)."""

    kind_name = "IndexLinkedOCEL"


class EventLog(Handle):
    """Handle to an `EventLog` (case-centric event log)."""

    kind_name = "EventLog"


class EventLogActivityProjection(Handle):
    """Handle to an `EventLogActivityProjection` (trace variants with frequencies)."""

    kind_name = "EventLogActivityProjection"


class TabularSource(Handle):
    """Handle to a `TabularSource` (a data source for OCEL extraction)."""

    kind_name = "TabularSource"


class PetriNet(Handle):
    """Handle to a `PetriNet` (structure only, no markings)."""

    kind_name = "PetriNet"


class AcceptingPetriNet(Handle):
    """Handle to an `AcceptingPetriNet` (Petri net with initial and final marking)."""

    kind_name = "AcceptingPetriNet"


class ProcessTree(Handle):
    """Handle to a `ProcessTree`."""

    kind_name = "ProcessTree"


class ObjectCentricPetriNet(Handle):
    """Handle to an `ObjectCentricPetriNet`."""

    kind_name = "ObjectCentricPetriNet"


class OCDeclareModel(Handle):
    """Handle to an `OCDeclareModel` (list of OC-DECLARE constraints)."""

    kind_name = "OCDeclareModel"


class ReflowResult(Handle):
    """Handle to a `ReflowResult` (the flow layer and cell states found by Reflow)."""

    kind_name = "ReflowResult"


class AnnotatedOcpn(Handle):
    """Handle to an `AnnotatedOcpn` (object-centric Petri net with Reflow annotations)."""

    kind_name = "AnnotatedOcpn"


_KINDS: Dict[str, Type[Handle]] = {
    cls.kind_name: cls
    for cls in (
        OCEL,
        SlimLinkedOCEL,
        IndexLinkedOCEL,
        EventLog,
        EventLogActivityProjection,
        TabularSource,
        PetriNet,
        AcceptingPetriNet,
        ProcessTree,
        ObjectCentricPetriNet,
        OCDeclareModel,
        ReflowResult,
        AnnotatedOcpn,
    )
}


def wrap(item_id: str, kind: Optional[str] = None, owned: bool = False) -> Handle:
    """Wrap a registry ID in the handle class of its type.

    Args:
        item_id: The registry ID.
        kind: The item's type name; looked up in the registry if not given.
        owned: Whether the handle removes the item when closed or garbage collected.
    """
    if kind is None:
        kind = _native.item_kind(item_id)
    cls = _KINDS.get(kind or "", Handle)
    return cls(item_id, owned=owned)
