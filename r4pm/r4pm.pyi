"""Native extension module of r4pm (built from src/lib.rs)."""
from typing import Any, Callable, Dict, List, Optional, Tuple

import polars

__version__: str
__variant__: str
"""`"full"` (r4pm-full) or `"default"` (r4pm)."""
__features__: List[str]
"""Optional features compiled in, e.g. `["full", "ocel-duckdb"]`."""

class BindingError(TypeError):
    """A binding call failed in the Rust library."""

def has_feature(name: str) -> bool:
    """Whether an optional feature (e.g., `"ocel-duckdb"`) is compiled into this build."""
    ...

def import_xes_rs(
    path: str, date_format: Optional[str] = None, print_debug: Optional[bool] = None
) -> Tuple[polars.DataFrame, str]:
    """Import an XES file as a DataFrame and the JSON of its log-level data."""
    ...

def export_xes_rs(df: polars.DataFrame, path: str) -> None:
    """Export an event DataFrame as XES."""
    ...

def import_ocel_rs(
    path: str, progress: Optional[Callable[[str, int, Optional[int]], None]] = None
) -> Dict[str, polars.DataFrame]:
    """Import an OCEL 2.0 file (format by extension) as DataFrames.

    Progress, warnings and cancellation as for `call_binding`.
    """
    ...

def import_ocel_xml_rs(
    path: str, progress: Optional[Callable[[str, int, Optional[int]], None]] = None
) -> Dict[str, polars.DataFrame]: ...
def import_ocel_json_rs(
    path: str, progress: Optional[Callable[[str, int, Optional[int]], None]] = None
) -> Dict[str, polars.DataFrame]: ...
def export_ocel_rs(ocel: Dict[str, polars.DataFrame], path: str) -> None:
    """Export OCEL DataFrames (as returned by `import_ocel_rs`) to a file."""
    ...

def import_pnml_rs(path: str) -> str:
    """JSON of the Petri net in a PNML file, with `null` for a missing marking."""
    ...

def export_pnml_rs(net_json: str, path: str) -> None:
    """Write the JSON of a Petri net (markings optional) as PNML."""
    ...

def list_bindings() -> List[Dict[str, Any]]:
    """Metadata of all bindings: id, name, description, module, arguments,
    required_arguments, return_schema (JSON schemas as strings)."""
    ...

def call_binding(
    function_id: str,
    args_json: str,
    num_threads: Optional[int] = None,
    progress: Optional[Callable[[str, int, Optional[int]], None]] = None,
) -> str:
    """Call a binding with JSON arguments and return its JSON result.

    Registry arguments may be IDs, file paths, `{"path": ...}`, `{"bytes": ..., "format": ...}`
    or inline JSON; items loaded only for the call are removed afterwards. Raises
    `BindingError` if the call fails. `progress` receives `(stage, done, total)` reports, and
    binding warnings are issued as Python warnings. Ctrl+C (or an exception from `progress`)
    cancels the call and is raised once it stopped.
    """
    ...

def import_item(
    item_type: str,
    file_path: str,
    item_id: Optional[str] = None,
    progress: Optional[Callable[[str, int, Optional[int]], None]] = None,
) -> str:
    """Load a file into the registry and return the item's ID.

    Progress, warnings and cancellation as for `call_binding`; a cancelled import adds no item.
    """
    ...

def import_item_from_json(item_type: str, json: str, item_id: Optional[str] = None) -> str:
    """Add an item from its JSON form and return its ID."""
    ...

def import_item_from_df(item_type: str, data: Any, item_id: Optional[str] = None) -> str:
    """Add an EventLog (DataFrame) or OCEL (dict of DataFrames) and return its ID."""
    ...

def convert_item(item_id: str, target_type: str, new_item_id: Optional[str] = None) -> str:
    """Convert an item to another type and return the new item's ID."""
    ...

def export_item(item_id: str, file_path: str) -> None:
    """Export an item to a file, in the format the path names."""
    ...

def item_to_df(item_id: str) -> Any:
    """An EventLog as DataFrame, or an OCEL type as dict of DataFrames."""
    ...

def item_to_json(item_id: str) -> str:
    """The JSON form of an item."""
    ...

def item_kind(item_id: str) -> Optional[str]:
    """The type name of an item, or `None` if no item has this ID."""
    ...

def list_items() -> List[Dict[str, str]]:
    """All items in the registry, as dicts with 'id' and 'type'."""
    ...

def list_item_kinds() -> List[str]:
    """The names of all registry item types."""
    ...

def remove_item(item_id: str) -> bool:
    """Remove an item; `False` if no item has this ID."""
    ...

def event_log_traces(item_id: str) -> List[List[str]]:
    """The activity sequence of every trace of an EventLog (or EventLogActivityProjection)."""
    ...
