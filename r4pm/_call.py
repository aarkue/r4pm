"""Calling a binding from Python: argument encoding and handle wrapping."""
from __future__ import annotations

import json
from typing import Any, Collection, Dict, Optional

from . import _progress
from . import r4pm as _native
from ._progress import ProgressOption
from .handles import Handle, wrap


def call(
    function_id: str,
    args: Dict[str, Any],
    registry_args: Collection[str] = (),
    returns: Optional[str] = None,
    num_threads: Optional[int] = None,
    progress: ProgressOption = None,
) -> Any:
    """Call a binding with Python arguments.

    A handle passed for an argument that is not a registry reference (e.g., a Petri net handle
    for a binding taking the net as JSON) is replaced by the item's JSON form. A result stored in
    the registry comes back as a handle that owns the item, unless `output_id` chose its ID.

    Args:
        function_id: The binding's ID.
        args: Arguments by name.
        registry_args: Names of the arguments that are registry references.
        returns: The registry kind of the result, if the binding stores it in the registry.
        num_threads: Number of worker threads (default: all cores).
        progress: Progress display for this call; see `r4pm.set_progress`. None uses the global
            setting, False shows none.
    """
    encoded = {
        name: value.to_json()
        if isinstance(value, Handle) and name not in registry_args
        else value
        for name, value in args.items()
    }
    callback, done = _progress.resolve(progress)
    try:
        raw = _native.call_binding(function_id, json.dumps(encoded), num_threads, callback)
    finally:
        done()
    result = json.loads(raw)
    if returns is not None:
        return wrap(result, returns, owned="output_id" not in args)
    return result
