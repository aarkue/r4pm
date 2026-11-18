"""
Conversion between pm4py and r4pm objects.

- Event logs: pm4py `EventLog` or pandas DataFrame <-> r4pm `EventLog` handle
- Petri nets: pm4py `(net, initial_marking, final_marking)` <-> Petri net dict or handle

Needs pandas and polars for logs, and pm4py for pm4py objects.
"""
from __future__ import annotations

from typing import Any, Optional, Tuple

from . import handles, petri_net
from . import r4pm as _native

__all__ = [
    "log_from_pm4py",
    "log_to_pm4py",
    "net_from_pm4py",
    "net_to_pm4py",
]

_CASE = "case:concept:name"
_ACTIVITY = "concept:name"
_TIMESTAMP = "time:timestamp"


def log_from_pm4py(
    log: Any,
    item_id: Optional[str] = None,
    *,
    case_id_key: str = _CASE,
    activity_key: str = _ACTIVITY,
    timestamp_key: str = _TIMESTAMP,
) -> handles.EventLog:
    """Store a pm4py event log (`EventLog` or pandas DataFrame) in the registry.

    Other columns become event attributes; `case:`-prefixed columns become trace attributes.

    Args:
        log: The pm4py `EventLog` or a pandas DataFrame.
        item_id: Optional ID. Without one, the handle owns the item.
        case_id_key: Case ID column.
        activity_key: Activity column.
        timestamp_key: Timestamp column.
    """
    import polars as pl

    if type(log).__module__.startswith("pm4py"):
        import pm4py

        log = pm4py.convert_to_dataframe(log)
    renames = {case_id_key: _CASE, activity_key: _ACTIVITY, timestamp_key: _TIMESTAMP}
    df = log.rename(columns={k: v for k, v in renames.items() if k != v and k in log.columns})
    new_id = _native.import_item_from_df("EventLog", pl.from_pandas(df), item_id)
    return handles.EventLog(new_id, owned=item_id is None)


def log_to_pm4py(log: str) -> Any:
    """An r4pm `EventLog` (handle, ID or XES path) as a pandas DataFrame in pm4py's format."""
    if _native.item_kind(log) is None:
        with handles.EventLog(_native.import_item("EventLog", log), owned=True) as loaded:
            return _native.item_to_df(loaded.id).to_pandas()
    return _native.item_to_df(log).to_pandas()


def net_from_pm4py(
    net: Any, initial_marking: Any, final_marking: Any, as_handle: bool = False
) -> Any:
    """A pm4py accepting Petri net as Petri net dict, or as registry handle with `as_handle`."""
    converted = petri_net.from_pm4py(net, initial_marking, final_marking)
    return petri_net.to_handle(converted) if as_handle else converted


def net_to_pm4py(net: Any) -> Tuple[Any, Any, Any]:
    """A Petri net dict, handle or PNML path as pm4py `(net, initial_marking, final_marking)`."""
    if isinstance(net, str):
        if _native.item_kind(net) is None:
            net = petri_net.import_pnml(net)
        else:
            net = petri_net.from_handle(net)
    return petri_net.to_pm4py(net)
