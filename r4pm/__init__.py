"""
r4pm - Rust for process mining

Simple DataFrame API
    - `df` with `import_xes`, `export_xes`, `import_ocel`, `import_ocel_pm4py`, etc.

Registry (for use with bindings):
    - import_item, import_item_from_df, import_item_from_json, convert_item, export_item
    - list_items, remove_item, item_to_df, item_to_json
    - items are returned as handles (see `r4pm.handles`), which are also their registry IDs

Conformance checking:
    - `conformance` with `alignments`, `fitness`, `precision` (accepts pm4py objects)

Petri nets:
    - `petri_net` with PNML import/export and pm4py conversion

pm4py interoperability:
    - `interop` with log and Petri net conversion

Progress and cancellation:
    - `set_progress` shows the progress of binding calls and imports (tqdm bars or a callback);
      Ctrl+C cancels a running call

Auto-generated bindings organized by module:
    - Import from r4pm.bindings
    - Submodules: bindings.discovery, bindings.core, etc.
"""
from __future__ import annotations

import json as _json
from typing import Any, Optional

from . import r4pm as _native
from .r4pm import (
    list_bindings,
    call_binding,
    list_items,
    list_item_kinds,
    remove_item,
    item_kind,
    BindingError,
    __version__,
    __variant__,
    __features__,
    has_feature,
)
from . import _progress, handles
from .handles import Handle
from ._progress import ProgressCallback, ProgressOption, get_progress, set_progress


def import_item(
    item_type: str,
    file_path: str,
    item_id: Optional[str] = None,
    *,
    progress: ProgressOption = None,
) -> Handle:
    """Load a file into the registry.

    Reports its progress (for most file formats, stage "Importing" with the bytes read). Ctrl+C
    (or an exception from the progress callback) cancels the import and leaves the registry
    unchanged; importer warnings are issued as Python warnings.

    Args:
        item_type: Type name (e.g., "OCEL", "EventLog", "SlimLinkedOCEL", "AcceptingPetriNet").
        file_path: Path to the file to load.
        item_id: Optional ID. Without one, an ID is generated and the returned handle owns the
            item (it is removed from the registry with the handle).
        progress: Progress display for this import; see `r4pm.set_progress`. None uses the
            global setting, False shows none.

    Returns:
        Handle to the item (a `str` subclass holding the registry ID).
    """
    callback, done = _progress.resolve(progress)
    try:
        item = _native.import_item(item_type, file_path, item_id, callback)
    finally:
        done()
    return handles.wrap(item, item_type, owned=item_id is None)


def import_item_from_json(item_type: str, data: Any, item_id: Optional[str] = None) -> Handle:
    """Add a registry item from its JSON form (a JSON string or a JSON-compatible object).

    Args:
        item_type: Type name (e.g., "AcceptingPetriNet").
        data: The item's JSON form, as `item_to_json` or the bindings return it.
        item_id: Optional ID; see `import_item`.
    """
    text = data if isinstance(data, str) else _json.dumps(data)
    return handles.wrap(
        _native.import_item_from_json(item_type, text, item_id), item_type, owned=item_id is None
    )


def convert_item(item_id: str, target_type: str, new_item_id: Optional[str] = None) -> Handle:
    """Convert a registry item to another type.

    Args:
        item_id: Handle or ID of the item to convert.
        target_type: Target type name (e.g., "SlimLinkedOCEL").
        new_item_id: Optional ID for the converted item; see `import_item`.
    """
    return handles.wrap(
        _native.convert_item(item_id, target_type, new_item_id),
        target_type,
        owned=new_item_id is None,
    )


def export_item(item_id: str, file_path: str) -> None:
    """Export a registry item to a file, in the format the path names."""
    _native.export_item(item_id, file_path)


def item_to_json(item_id: str) -> Any:
    """A registry item in its JSON form (a full serialization for event data)."""
    return _json.loads(_native.item_to_json(item_id))


def item_to_df(item_id: str) -> Any:
    """A registry item as Polars DataFrame(s). Requires polars.

    Returns:
        One DataFrame for an EventLog, a dict of DataFrames (events, objects, relations, o2o,
        object_changes) for the OCEL types.
    """
    _require_polars()
    return _native.item_to_df(item_id)


def import_item_from_df(item_type: str, data: Any, item_id: Optional[str] = None) -> Handle:
    """Create a registry item from Polars DataFrame(s). Requires polars.

    Args:
        item_type: "EventLog" (one DataFrame) or "OCEL" (dict with 'events', 'objects',
            'relations', 'o2o', 'object_changes' DataFrames).
        data: The DataFrame(s).
        item_id: Optional ID; see `import_item`.
    """
    _require_polars()
    return handles.wrap(
        _native.import_item_from_df(item_type, data, item_id), item_type, owned=item_id is None
    )


def _require_polars() -> None:
    # pyo3-polars panics if the polars Python package is missing
    try:
        import polars  # noqa: F401
    except ImportError as e:
        raise ImportError(
            "This function requires polars. Install with: pip install r4pm[polars]"
        ) from e


from . import petri_net
from . import conformance
from . import interop

try:
    from . import df
except ImportError:

    class _DfNotAvailable:
        def __getattr__(self, name: str) -> Any:
            raise ImportError("r4pm.df requires polars. Install with: pip install r4pm[polars]")

    df = _DfNotAvailable()  # type: ignore[assignment]

# Generated by build.rs during the Rust build
try:
    from . import bindings
except ImportError:

    class _BindingsNotGenerated:
        def __getattr__(self, name: str) -> Any:
            raise ImportError("Bindings not available. Rebuild with: maturin develop --release")

    bindings = _BindingsNotGenerated()  # type: ignore[assignment]

__all__ = [
    # Registry
    "import_item",
    "import_item_from_df",
    "import_item_from_json",
    "convert_item",
    "export_item",
    "item_to_df",
    "item_to_json",
    "item_kind",
    "list_items",
    "list_item_kinds",
    "remove_item",
    "Handle",
    "handles",
    "BindingError",
    # Progress
    "set_progress",
    "get_progress",
    "ProgressCallback",
    # Bindings introspection
    "list_bindings",
    "call_binding",
    # Submodules
    "df",
    "petri_net",
    "conformance",
    "interop",
    "bindings",
    # Build information
    "__version__",
    "__variant__",
    "__features__",
    "has_feature",
]
