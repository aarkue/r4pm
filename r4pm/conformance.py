"""
Conformance checking of case-centric event logs against accepting Petri nets.

- `alignments`: optimal alignments, one per trace, shaped like pm4py's
  (`pm4py.conformance_diagnostics_alignments`)
- `fitness`: alignment-based fitness (pm4py's `fitness_alignments` keys), or token-based replay
  for nets without silent transitions and duplicate labels
- `precision`: escaping-edges precision, with presets following ProM and pm4py

Logs can be given as an r4pm `EventLog`/`EventLogActivityProjection` handle or registry ID, a
path to an XES file, a pm4py `EventLog`, a pandas or polars DataFrame (pm4py column names by
default), or a list of activity sequences. Nets can be given as a Petri net dict
(`r4pm.petri_net.PetriNet`), an `AcceptingPetriNet` or `PetriNet` handle, a PNML path, or a pm4py
net with its initial and final marking:

    import pm4py
    from r4pm import conformance

    log = pm4py.read_xes("log.xes")
    net, im, fm = pm4py.discover_petri_net_inductive(log)
    conformance.fitness(log, net, im, fm)          # {'log_fitness': ..., ...}
    conformance.precision(log, net, im, fm, preset="pm4py")
    aligned = conformance.alignments(log, net, im, fm)
    aligned[0]["alignment"]                         # [('a', 'a'), ('>>', None), ('b', '>>'), ...]

pm4py is optional and only imported for converting pm4py nets.
"""
from __future__ import annotations

import json
import os
from collections import Counter
from dataclasses import dataclass, field
from typing import (
    TYPE_CHECKING,
    Any,
    Dict,
    Iterable,
    List,
    Literal,
    Optional,
    Sequence,
    Tuple,
    Union,
    overload,
)

from . import handles, petri_net
from . import r4pm as _native
from .r4pm import BindingError
from ._call import call as _call

if TYPE_CHECKING:
    from typing_extensions import NotRequired, TypedDict
else:
    from typing import TypedDict

__all__ = [
    "AlignmentOptions",
    "Move",
    "VariantAlignment",
    "Alignments",
    "TraceAlignment",
    "AlignmentFitness",
    "TokenBasedFitness",
    "PrecisionResult",
    "ConformanceError",
    "alignments",
    "fitness",
    "precision",
    "precision_result",
]

SKIP = ">>"
"""The placeholder for "no move" in pm4py-shaped alignments."""

_MOD = "process_mining"
_ALIGN_VARIANTS = f"{_MOD}::align_variants"
_ALIGN_TRACE = f"{_MOD}::align_trace"
_COMPUTE_FITNESS = f"{_MOD}::compute_fitness"
_PRECISION = f"{_MOD}::escaping_edges_precision"
_PRECISION_FROM_ALIGNMENTS = f"{_MOD}::escaping_edges_precision_from_alignments"
_TOKEN_REPLAY = f"{_MOD}::apply_token_based_replay"

LogLike = Any
"""An event log: see the module documentation for the accepted forms."""
NetLike = Any
"""A Petri net: see the module documentation for the accepted forms."""

Preset = Literal["default", "prom", "pm4py"]


class ConformanceError(RuntimeError):
    """A conformance result cannot be computed (e.g., an alignment search hit its limit).

    Errors reported by the Rust implementation are raised as `r4pm.BindingError`.
    """


@dataclass(frozen=True)
class AlignmentOptions:
    """Costs and limits of alignment searches.

    Attributes:
        model_move_cost: Cost of a visible transition firing without a matching event.
        log_move_cost: Cost of an event the model does not follow.
        sync_move_cost: Cost of a synchronous move.
        silent_move_cost: Cost of firing a silent transition.
        max_states: States one search may visit before giving up (`None`: no limit).
        max_memory_mib: Memory in MiB all searches of one call may hold together, split
            evenly across the searches running at once (`None`: no limit).
        retry_out_of_memory: Align traces that ran out of their memory share again, one at a
            time with the whole budget.
    """

    model_move_cost: int = 1
    log_move_cost: int = 1
    sync_move_cost: int = 0
    silent_move_cost: int = 0
    max_states: Optional[int] = None
    max_memory_mib: Optional[int] = 6 * 1024
    retry_out_of_memory: bool = False

    def to_json(self) -> Dict[str, Any]:
        """The `AlignmentOptions` JSON the bindings take."""
        return {
            "cost_fn": {
                "model_move_cost": self.model_move_cost,
                "log_move_cost": self.log_move_cost,
                "sync_move_cost": self.sync_move_cost,
                "silent_move_cost": self.silent_move_cost,
            },
            "max_states": self.max_states,
            "max_memory_mib": self.max_memory_mib,
            "retry_out_of_memory": self.retry_out_of_memory,
        }


@dataclass(frozen=True)
class Move:
    """One step of an alignment.

    Attributes:
        kind: `"sync"` (log and model agree), `"log"` (event without model move), `"model"`
            (visible transition without event), or `"silent"` (silent transition).
        activity: The event's activity, for sync and log moves.
        label: The transition's label, for sync and model moves.
        transition: Index of the fired transition in the net, for all but log moves.
        event_index: Index of the event in the trace, for sync and log moves.
    """

    kind: Literal["sync", "log", "model", "silent"]
    activity: Optional[str] = None
    label: Optional[str] = None
    transition: Optional[int] = None
    event_index: Optional[int] = None

    @property
    def is_deviation(self) -> bool:
        """Whether this is a log move or a model move of a visible transition."""
        return self.kind in ("log", "model")

    def as_pm4py(self) -> Tuple[str, Optional[str]]:
        """The `(log, model)` pair pm4py uses, with `">>"` for no move and `None` for silent."""
        if self.kind == "log":
            return (self.activity or "", SKIP)
        if self.kind == "sync":
            return (self.activity or "", self.label)
        return (SKIP, self.label)


@dataclass
class VariantAlignment:
    """The alignment of one trace variant.

    Attributes:
        activities: The variant's activity sequence.
        frequency: Number of traces of this variant.
        moves: The optimal alignment, `None` if it could not be computed.
        cost: Its cost under the cost function.
        fitness: `1 - cost / (len(activities) * log_move_cost + cost of the empty trace)`.
        states_visited: States visited by the search.
        worst_cost: Cost of the worst alignment: every event a log move, then the cheapest run
            of the model.
        error: Why the alignment could not be computed, e.g., `{"SearchError": "LimitReached"}`.
    """

    activities: List[str]
    frequency: int
    moves: Optional[List[Move]] = None
    cost: Optional[int] = None
    fitness: Optional[float] = None
    states_visited: Optional[int] = None
    worst_cost: Optional[int] = None
    error: Any = None

    def as_pm4py(self) -> Optional["TraceAlignment"]:
        """The alignment in pm4py's shape, or `None` if it failed."""
        if self.moves is None or self.cost is None or self.fitness is None:
            return None
        return {
            "alignment": [m.as_pm4py() for m in self.moves],
            "cost": self.cost,
            "fitness": self.fitness,
            "visited_states": self.states_visited or 0,
            "bwc": self.worst_cost or 0,
        }


class TraceAlignment(TypedDict):
    """The alignment of one trace, in pm4py's shape.

    Unlike pm4py, `cost` and `bwc` are in the units of the cost function (1 per log or model
    move by default), not scaled by 10000.
    """

    alignment: List[Tuple[str, Optional[str]]]
    """`(log, model)` moves, with `">>"` for no move and `None` for a silent transition."""
    cost: int
    fitness: float
    visited_states: int
    bwc: int
    """Cost of the worst alignment: all events as log moves, then the cheapest model run."""


class AlignmentFitness(TypedDict):
    """Alignment-based fitness, with the keys of `pm4py.fitness_alignments`.

    Percentages are in `[0, 100]`, as in pm4py.
    """

    log_fitness: float
    average_trace_fitness: float
    percentage_of_fitting_traces: float
    percFitTraces: float
    averageFitness: float
    total_cost: int


class TokenBasedFitness(TypedDict):
    """Token-based replay fitness, summed over the whole log."""

    log_fitness: float
    produced: int
    consumed: int
    missing: int
    remaining: int


class PrecisionResult(TypedDict):
    """Escaping-edges precision with the counts behind it."""

    precision: float
    weighted_available: int
    """Sum over all states of their weight times the number of available activities."""
    weighted_escaping: int
    """Sum over all states of their weight times the number of escaping edges."""
    num_states: int
    unaligned_traces: int
    """Traces left out because their alignment failed."""
    truncated_traces: int
    """Traces only measured up to the first step the model could not follow."""
    search_limit_reached: bool
    escaping_edges: NotRequired[Optional[List[Dict[str, Any]]]]
    """Every escaping edge (`prefix`, `escaping`, `weight`), if requested."""


class Alignments(List[Optional[TraceAlignment]]):
    """Per-trace alignments in pm4py's shape (in log order), with the variant-level results.

    A trace whose alignment failed is `None`, as in pm4py. Pass this object to `fitness` or
    `precision` as `alignments=...` to reuse it instead of aligning again.

    Attributes:
        variants: One typed result per trace variant.
        net: The aligned net, as a Petri net dict.
        options: The alignment options used.
    """

    variants: List[VariantAlignment]
    net: petri_net.PetriNet
    options: AlignmentOptions
    _raw: List[Dict[str, Any]]

    def fitness(self, num_threads: Optional[int] = None) -> AlignmentFitness:
        """Alignment-based fitness from these alignments."""
        return _fitness_from_raw(self._raw, self.net, self.options, num_threads)


# Inputs


def _module_of(obj: Any) -> str:
    return type(obj).__module__ or ""


def _traces_from_object(
    log: Any, activity_key: str, case_id_key: str
) -> Optional[List[List[str]]]:
    """Activity sequences of an in-memory log, or `None` if `log` is not one."""
    module = _module_of(log)
    if module.startswith("pandas"):
        if not hasattr(log, "groupby"):
            return None
        # Sorted by case ID with events in row order, as pm4py orders its alignments
        grouped = log.groupby(case_id_key)[activity_key].agg(list)
        return [[str(a) for a in trace] for trace in grouped.tolist()]
    if module.startswith("polars"):
        import polars as pl

        if isinstance(log, pl.LazyFrame):
            log = log.collect()
        if not isinstance(log, pl.DataFrame):
            return None
        grouped = log.group_by(case_id_key, maintain_order=True).agg(
            pl.col(activity_key).cast(pl.String)
        )
        return [list(trace) for trace in grouped.get_column(activity_key).to_list()]
    if module.startswith("pm4py"):
        # pm4py EventLog: a sequence of traces of events (dicts)
        return [[str(event[activity_key]) for event in trace] for trace in log]
    if isinstance(log, (list, tuple)) and all(
        isinstance(t, (list, tuple)) and not isinstance(t, str) for t in log
    ):
        return [[str(a) for a in trace] for trace in log]
    return None


@dataclass
class _Log:
    """A resolved log: a registry reference for the bindings, and the traces when known"""

    ref: Any
    traces: Optional[List[List[str]]] = None
    owned: List[handles.Handle] = field(default_factory=list)

    def get_traces(self) -> List[List[str]]:
        if self.traces is None:
            self.traces = _native.event_log_traces(str(self.ref))
        return self.traces

    def close(self) -> None:
        for h in self.owned:
            h.close()


def _projection(traces: Iterable[Sequence[str]]) -> Dict[str, Any]:
    """The JSON of an `EventLogActivityProjection` of `traces`"""
    counts = Counter(tuple(t) for t in traces)
    activities = sorted({a for t in counts for a in t})
    index = {a: i for i, a in enumerate(activities)}
    variants = sorted(counts.items(), key=lambda kv: (-kv[1], kv[0]))
    return {
        "activities": activities,
        "act_to_index": index,
        "traces": [[[index[a] for a in t], n] for t, n in variants],
    }


def _resolve_log(
    log: LogLike, activity_key: str, case_id_key: str, need_traces: bool
) -> _Log:
    traces = _traces_from_object(log, activity_key, case_id_key)
    if traces is not None:
        return _Log({"inline": _projection(traces)}, traces)
    if isinstance(log, str):
        kind = _native.item_kind(log)
        if kind in ("EventLog", "EventLogActivityProjection"):
            resolved = _Log(log)
        elif kind is not None:
            raise TypeError(f"Expected an event log, but '{log}' is a {kind}")
        elif os.path.exists(log):
            handle = handles.EventLog(_native.import_item("EventLog", log), owned=True)
            resolved = _Log(handle, owned=[handle])
        else:
            raise FileNotFoundError(f"'{log}' is neither a registry ID nor an existing file")
        if need_traces:
            traces = resolved.get_traces()
            resolved.ref = {"inline": _projection(traces)}
        return resolved
    raise TypeError(
        "Unsupported log type "
        f"{type(log).__name__}: expected an r4pm handle or ID, an XES path, a pm4py EventLog, "
        "a pandas or polars DataFrame, or a list of activity sequences"
    )


def _resolve_net(
    net: NetLike, initial_marking: Any = None, final_marking: Any = None
) -> petri_net.PetriNet:
    """The accepting Petri net dict for any accepted net form"""
    if isinstance(net, tuple) and len(net) == 3 and initial_marking is None:
        net, initial_marking, final_marking = net
    if petri_net.is_pm4py_net(net):
        if initial_marking is None or final_marking is None:
            raise ValueError("A pm4py net needs its initial and final marking")
        return petri_net.from_pm4py(net, initial_marking, final_marking)
    if isinstance(net, str):
        kind = _native.item_kind(net)
        if kind in ("AcceptingPetriNet", "PetriNet"):
            resolved: petri_net.PetriNet = json.loads(_native.item_to_json(net))
        elif kind is not None:
            raise TypeError(f"Expected a Petri net, but '{net}' is a {kind}")
        elif os.path.exists(net):
            resolved = petri_net.import_pnml(net)
        else:
            raise FileNotFoundError(f"'{net}' is neither a registry ID nor an existing file")
    elif isinstance(net, dict):
        resolved = dict(net)  # type: ignore[assignment]
    else:
        raise TypeError(
            f"Unsupported net type {type(net).__name__}: expected a Petri net dict, an r4pm "
            "handle or ID, a PNML path, or a pm4py net with markings"
        )
    for key, given in (("initial_marking", initial_marking), ("final_marking", final_marking)):
        if given is not None:
            resolved[key] = given  # type: ignore[literal-required]
        if resolved.get(key) is None:
            raise ValueError(f"The net has no {key.replace('_', ' ')}")
    return resolved


# Alignments


def _ok(result: Any) -> Tuple[Any, Any]:
    """`(value, error)` of a serialized Rust `Result`"""
    if isinstance(result, dict) and len(result) == 1:
        if "Ok" in result:
            return result["Ok"], None
        if "Err" in result:
            return None, result["Err"]
    return result, None


def _moves(raw_moves: List[Dict[str, Any]], activities: List[str]) -> List[Move]:
    moves = []
    for m in raw_moves:
        (kind, data), = m.items()
        if kind == "SyncMove":
            i = data["trace_event_index"]
            moves.append(Move("sync", activities[i], data["label"], data["transition"], i))
        elif kind == "LogMove":
            i = data["trace_event_index"]
            moves.append(Move("log", activities[i], event_index=i))
        else:
            label = data.get("label")
            moves.append(Move("model" if label is not None else "silent", None, label, data["transition"]))
    return moves


def _align_raw(
    log: _Log, net: petri_net.PetriNet, options: AlignmentOptions, num_threads: Optional[int]
) -> List[Dict[str, Any]]:
    return _call(
        _ALIGN_VARIANTS,
        {"net": net, "projection": log.ref, "options": options.to_json()},
        registry_args=("projection",),
        num_threads=num_threads,
    )


def _empty_trace_cost(net: petri_net.PetriNet, options: AlignmentOptions) -> Optional[int]:
    try:
        return _call(_ALIGN_TRACE, {"net": net, "trace": [], "options": options.to_json()})["cost"]
    except _native.BindingError:
        # Then no trace aligns either, and each variant carries the error
        return None


def _variants(
    raw: List[Dict[str, Any]], model_min: Optional[int], options: AlignmentOptions
) -> List[VariantAlignment]:
    variants = []
    for v in raw:
        result, error = _ok(v["result"])
        variant = VariantAlignment(v["activities"], v["frequency"], error=error)
        if result is not None:
            cost: int = result["cost"]
            worst = len(variant.activities) * options.log_move_cost + (model_min or 0)
            variant.moves = _moves(result["moves"], variant.activities)
            variant.cost = cost
            variant.states_visited = result["states_visited"]
            variant.worst_cost = worst
            variant.fitness = 1.0 if worst == 0 else 1.0 - cost / worst
        variants.append(variant)
    return variants


def alignments(
    log: LogLike,
    net: NetLike,
    initial_marking: Any = None,
    final_marking: Any = None,
    *,
    options: Optional[AlignmentOptions] = None,
    num_threads: Optional[int] = None,
    activity_key: str = "concept:name",
    case_id_key: str = "case:concept:name",
) -> Alignments:
    """Optimal alignments of every trace of `log` on `net`.

    Each distinct trace (variant) is aligned once, in parallel.

    Args:
        log: The event log (see the module documentation for accepted forms).
        net: The accepting Petri net (see the module documentation for accepted forms).
        initial_marking: Initial marking, for a pm4py net (or to override the net's own).
        final_marking: Final marking, for a pm4py net (or to override the net's own).
        options: Costs and search limits.
        num_threads: Worker threads (default: all cores).
        activity_key: Activity column or attribute, for pm4py logs and DataFrames.
        case_id_key: Case ID column, for DataFrames.

    Returns:
        A list with one pm4py-shaped alignment per trace (`None` where the search failed), with
        the typed per-variant results in `.variants`.
    """
    options = options or AlignmentOptions()
    resolved_net = _resolve_net(net, initial_marking, final_marking)
    resolved_log = _resolve_log(log, activity_key, case_id_key, need_traces=True)
    try:
        raw = _align_raw(resolved_log, resolved_net, options, num_threads)
        variants = _variants(raw, _empty_trace_cost(resolved_net, options), options)
        by_trace = {tuple(v.activities): v for v in variants}
        result = Alignments(by_trace[tuple(t)].as_pm4py() for t in resolved_log.get_traces())
    finally:
        resolved_log.close()
    result.variants = variants
    result.net = resolved_net
    result.options = options
    result._raw = raw
    return result


# Fitness


def _fitness_from_raw(
    raw: List[Dict[str, Any]],
    net: petri_net.PetriNet,
    options: AlignmentOptions,
    num_threads: Optional[int],
) -> AlignmentFitness:
    failed = next((v for v in raw if "Err" in v["result"]), None)
    if failed is not None:
        raise ConformanceError(
            f"Aligning the variant {failed['activities']} failed: {failed['result']['Err']}. "
            "Raise `max_states`/`max_memory_mib` or set `retry_out_of_memory`."
        )
    fit = _call(
        _COMPUTE_FITNESS,
        {"net": net, "alignments": raw, "options": options.to_json()},
        num_threads=num_threads,
    )
    percentage = 100.0 * fit["perfectly_fitting_frac"]
    return {
        "log_fitness": fit["log_fitness"],
        "average_trace_fitness": fit["average_fitness"],
        "percentage_of_fitting_traces": percentage,
        "percFitTraces": percentage,
        "averageFitness": fit["average_fitness"],
        "total_cost": fit["total_costs"],
    }


@overload
def fitness(
    log: LogLike,
    net: NetLike = ...,
    initial_marking: Any = ...,
    final_marking: Any = ...,
    *,
    method: Literal["alignments"] = ...,
    options: Optional[AlignmentOptions] = ...,
    alignments: Optional[Alignments] = ...,
    num_threads: Optional[int] = ...,
    activity_key: str = ...,
    case_id_key: str = ...,
) -> AlignmentFitness: ...


@overload
def fitness(
    log: LogLike,
    net: NetLike = ...,
    initial_marking: Any = ...,
    final_marking: Any = ...,
    *,
    method: Literal["token_based"],
    options: Optional[AlignmentOptions] = ...,
    alignments: Optional[Alignments] = ...,
    num_threads: Optional[int] = ...,
    activity_key: str = ...,
    case_id_key: str = ...,
) -> TokenBasedFitness: ...


def fitness(
    log: LogLike,
    net: NetLike = None,
    initial_marking: Any = None,
    final_marking: Any = None,
    *,
    method: Literal["alignments", "token_based"] = "alignments",
    options: Optional[AlignmentOptions] = None,
    alignments: Optional[Alignments] = None,
    num_threads: Optional[int] = None,
    activity_key: str = "concept:name",
    case_id_key: str = "case:concept:name",
) -> Union[AlignmentFitness, TokenBasedFitness]:
    """Fitness of `log` on `net`.

    With `method="alignments"` (default), the result has the keys of `pm4py.fitness_alignments`
    (`log_fitness`, `average_trace_fitness`, `percentage_of_fitting_traces`, ...). With
    `method="token_based"`, it is token-based replay, which requires a net without silent
    transitions and without duplicate labels; its `log_fitness` equals pm4py's token-based
    replay fitness on such nets.

    Args:
        log: The event log (see the module documentation), or an `Alignments` result.
        net: The accepting Petri net (not needed when `log` is an `Alignments` result).
        initial_marking: Initial marking, for a pm4py net.
        final_marking: Final marking, for a pm4py net.
        method: `"alignments"` or `"token_based"`.
        options: Costs and search limits of the alignments.
        alignments: Alignments computed before for this log and net, to reuse.
        num_threads: Worker threads (default: all cores).
        activity_key: Activity column or attribute, for pm4py logs and DataFrames.
        case_id_key: Case ID column, for DataFrames.
    """
    if isinstance(log, Alignments) and alignments is None:
        alignments = log
    if method == "token_based":
        return _token_based_fitness(
            log, net, initial_marking, final_marking, activity_key, case_id_key, num_threads
        )
    if method != "alignments":
        raise ValueError(f"Unknown fitness method {method!r}")
    if alignments is not None:
        return alignments.fitness(num_threads)
    options = options or AlignmentOptions()
    resolved_net = _resolve_net(net, initial_marking, final_marking)
    resolved_log = _resolve_log(log, activity_key, case_id_key, need_traces=False)
    try:
        raw = _align_raw(resolved_log, resolved_net, options, num_threads)
    finally:
        resolved_log.close()
    return _fitness_from_raw(raw, resolved_net, options, num_threads)


def _token_based_fitness(
    log: LogLike,
    net: NetLike,
    initial_marking: Any,
    final_marking: Any,
    activity_key: str,
    case_id_key: str,
    num_threads: Optional[int],
) -> TokenBasedFitness:
    resolved_net = _resolve_net(net, initial_marking, final_marking)
    resolved_log = _resolve_log(log, activity_key, case_id_key, need_traces=False)
    try:
        result = _call(
            _TOKEN_REPLAY,
            {"net": resolved_net, "event_log": resolved_log.ref},
            registry_args=("event_log",),
            num_threads=num_threads,
        )
    except BindingError as e:
        if "duplicate labels or silent transitions" in str(e):
            raise ConformanceError(
                "Token-based replay needs a net without silent transitions and duplicate labels; "
                "use method='alignments' for this net"
            ) from e
        raise ConformanceError(f"Token-based replay failed: {e}") from e
    finally:
        resolved_log.close()

    def ratio(part: int, total: int) -> float:
        return 0.0 if total == 0 else part / total

    return {
        "log_fitness": 0.5 * (1 - ratio(result["missing"], result["consumed"]))
        + 0.5 * (1 - ratio(result["remaining"], result["produced"])),
        "produced": result["produced"],
        "consumed": result["consumed"],
        "missing": result["missing"],
        "remaining": result["remaining"],
    }


# Precision

_PRESET_OPTIONS = f"{_MOD}::escaping_edges_options"


def precision_result(
    log: LogLike,
    net: NetLike = None,
    initial_marking: Any = None,
    final_marking: Any = None,
    *,
    preset: Preset = "default",
    options: Optional[AlignmentOptions] = None,
    alignments: Optional[Alignments] = None,
    silent_transitions: Optional[Literal["Skip", "AsActivities"]] = None,
    trace_ends: Optional[Literal["Ignore", "Weigh", "AsActivity"]] = None,
    count_escaping_edges_at_start: Optional[bool] = None,
    exclude_dead_ends: Optional[bool] = None,
    search_limit: Optional[int] = 10_000,
    report_escaping_edges: bool = False,
    num_threads: Optional[int] = None,
    activity_key: str = "concept:name",
    case_id_key: str = "case:concept:name",
) -> PrecisionResult:
    """Escaping-edges precision of `net` with respect to `log`, with the counts behind it.

    See `precision` for the arguments. `report_escaping_edges=True` also lists every escaping
    edge.
    """
    if isinstance(log, Alignments) and alignments is None:
        alignments = log
    settings: Dict[str, Any] = _call(_PRESET_OPTIONS, {"preset": preset})
    for key, value in (
        ("silent_transitions", silent_transitions),
        ("trace_ends", trace_ends),
        ("count_escaping_edges_at_start", count_escaping_edges_at_start),
        ("exclude_dead_ends", exclude_dead_ends),
    ):
        if value is not None:
            settings[key] = value
    settings["search_limit"] = search_limit
    settings["report_escaping_edges"] = report_escaping_edges
    replay = settings.get("log_paths") == "ReplayUntilDeviation"

    if alignments is not None:
        if replay:
            raise ValueError(f"The {preset!r} preset replays the log, so it cannot reuse alignments")
        settings["log_paths"] = {"Aligned": {"options": alignments.options.to_json()}}
        result = _call(
            _PRECISION_FROM_ALIGNMENTS,
            {"net": alignments.net, "alignments": alignments._raw, "options": settings},
            num_threads=num_threads,
        )
        return result

    align_options = options or AlignmentOptions()
    if not replay:
        settings["log_paths"] = {"Aligned": {"options": align_options.to_json()}}
    resolved_net = _resolve_net(net, initial_marking, final_marking)
    resolved_log = _resolve_log(log, activity_key, case_id_key, need_traces=False)
    try:
        return _call(
            _PRECISION,
            {"net": resolved_net, "log": resolved_log.ref, "options": settings},
            registry_args=("log",),
            num_threads=num_threads,
        )
    finally:
        resolved_log.close()


def precision(
    log: LogLike,
    net: NetLike = None,
    initial_marking: Any = None,
    final_marking: Any = None,
    *,
    preset: Preset = "default",
    options: Optional[AlignmentOptions] = None,
    alignments: Optional[Alignments] = None,
    silent_transitions: Optional[Literal["Skip", "AsActivities"]] = None,
    trace_ends: Optional[Literal["Ignore", "Weigh", "AsActivity"]] = None,
    count_escaping_edges_at_start: Optional[bool] = None,
    exclude_dead_ends: Optional[bool] = None,
    search_limit: Optional[int] = 10_000,
    num_threads: Optional[int] = None,
    activity_key: str = "concept:name",
    case_id_key: str = "case:concept:name",
) -> float:
    """Escaping-edges precision of `net` with respect to `log`.

    Presets:
        - `"default"`: Paths from optimal alignments; activities reachable through silent
          transitions are available; traces ending in a state count towards its weight.
        - `"prom"`: As ProM's "Check Precision using Escaping Edges" (silent transitions are
          symbols of their own, no escaping edges counted at the start).
        - `"pm4py"`: As `pm4py.precision_alignments` (replays each trace up to its first
          deviation; trace ends do not count).

    Args:
        log: The event log (see the module documentation), or an `Alignments` result to reuse.
        net: The accepting Petri net (not needed when reusing alignments).
        initial_marking: Initial marking, for a pm4py net.
        final_marking: Final marking, for a pm4py net.
        preset: Which tool's variant of the measure to follow.
        options: Costs and limits of the alignments (not used by the `"pm4py"` preset).
        alignments: Alignments computed before for this log and net, to reuse (not with the
            `"pm4py"` preset).
        silent_transitions: Override the preset: `"Skip"` or `"AsActivities"`.
        trace_ends: Override the preset: `"Ignore"`, `"Weigh"` or `"AsActivity"`.
        count_escaping_edges_at_start: Override the preset.
        exclude_dead_ends: Only count activities after which the final marking can still be
            reached.
        search_limit: Markings visited per search over silent transitions (`None`: no limit).
        num_threads: Worker threads (default: all cores).
        activity_key: Activity column or attribute, for pm4py logs and DataFrames.
        case_id_key: Case ID column, for DataFrames.
    """
    return precision_result(
        log,
        net,
        initial_marking,
        final_marking,
        preset=preset,
        options=options,
        alignments=alignments,
        silent_transitions=silent_transitions,
        trace_ends=trace_ends,
        count_escaping_edges_at_start=count_escaping_edges_at_start,
        exclude_dead_ends=exclude_dead_ends,
        search_limit=search_limit,
        num_threads=num_threads,
        activity_key=activity_key,
        case_id_key=case_id_key,
    )["precision"]
