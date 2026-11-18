# r4pm

Python bindings for the Rust4PM Project: Process mining in Python with the speed of Rust

This library provides basic import/export of XES/OCEL event data, as well as other exposed functionality from the [Rust4PM project](https://github.com/aarkue/rust4pm) (e.g., process discovery algorithms).

## Features

- **Fast XES/OCEL Import/Export**: Efficient Rust-based import and export of `.xes`, `.xes.gz`, and OCEL2 (`.xml`/`.json`) files
- **Auto-Generated Bindings**: All process_mining functions automatically exposed with full IDE support (autocomplete, type hints, docs)
- **Registry System**: Manage data objects as typed handles and convert between types as needed
- **Conformance Checking**: Alignments, fitness and precision on pm4py or r4pm logs and Petri nets
- **Polars DataFrames**: Polars facilitates the fast transfer of event data from Python to Rust and vice versa

## Installation

```bash
pip install r4pm         # default
pip install r4pm-full    # additionally bundles DuckDB (~15 MiB larger)
```

Both provide the `r4pm` import package, so install one or the other. `pip install r4pm[pm4py]`
adds pm4py for the pm4py interoperability, `pip install r4pm[progress]` adds tqdm for progress
bars. `r4pm-full` adds the DuckDB
bindings (`write_ocel_to_duckdb`, `stream_ocel_file_to_duckdb`, ...) and `.duckdb` import and
export, nothing else differs.

```python
import r4pm

r4pm.__variant__                 # "default" or "full"
r4pm.__features__                # optional features compiled in
r4pm.has_feature("ocel-duckdb")
```

## Quick Start

```python
from r4pm import bindings
import r4pm

# Load an OCEL file into the registry; returns a handle (also usable as the item's ID)
ocel = r4pm.import_item('OCEL', 'data/orders.xml')

# Bindings convert an item to the type they take (most take a SlimLinkedOCEL); convert once
# when calling several of them
locel = ocel.convert('SlimLinkedOCEL')

# Get statistics
num = bindings.num_events(ocel=locel)
print(f"Events: {num}")

# Discover object-centric DFG
dfg = bindings.discover_ocdfg(locel)
print(f"Discovered DFG for {len(dfg['object_type_to_dfg'])} object types")

# For case-centric event logs:
log = r4pm.import_item('EventLog', 'data/log.xes')
case_dfg = bindings.discover_dfg(log)
```

## How It Works

### Auto-Generated Bindings

All functions from the [`process_mining` Rust library](https://docs.rs/process_mining/) are automatically discovered and exposed as Python functions with:
- **Full type hints** for IDE autocomplete
- **Automatic documentation** from Rust docs
- **Type validation** via JSON schemas

The bindings are organized by module (mirroring the Rust crate structure):
```python
from r4pm import bindings

# Top-level access to all functions
bindings.discover_dfg(event_log=log_id)
bindings.num_events(ocel=locel_id)

# Or use submodules for organization
from r4pm.bindings.discovery.case_centric import dfg
dfg.discover_dfg(event_log=log_id)
```

Bindings are automatically generated during the Rust build via `build.rs`.

### Registry System

Data is managed through a registry that holds different object types:
- `OCEL` - Raw OCEL data
- `SlimLinkedOCEL` - Memory-efficient linked OCEL (required by most functions)
- `IndexLinkedOCEL` - Indexed OCEL for analysis
- `EventLog` - Case-centric event log
- `EventLogActivityProjection` - Activity-projected log for discovery
- `PetriNet`, `AcceptingPetriNet`, `ProcessTree`, `ObjectCentricPetriNet`, `OCDeclareModel` - Process models

Registry items are returned as handles (`r4pm.handles`), typed by kind (`handles.OCEL`,
`handles.EventLog`, ...). A handle is a `str` subclass holding the item's ID, so it works
wherever an ID is accepted. A handle returned by r4pm owns its item: the item is removed when the
handle is closed, leaves a `with` block, or is garbage collected. Items created with an
explicit ID (`item_id=...`, `output_id=...`) stay until removed with `r4pm.remove_item`.

```python
# Load files into the registry
ocel = r4pm.import_item('OCEL', 'file.xml')
log = r4pm.import_item('EventLog', 'file.xes')

# Convert between types (either like this or using r4pm.convert_item)
locel = ocel.convert('IndexLinkedOCEL')
proj = log.convert('EventLogActivityProjection')

with r4pm.import_item('AcceptingPetriNet', 'model.pnml') as net:
    print(net.kind, net.to_json()["transitions"][0])
# net is removed from the registry here

# List registry contents
for item in r4pm.list_items():
    print(f"{item['id']}: {item['type']}")
```

Arguments that take a registry item also accept a path to a file, which is loaded only for that
call. Bindings that change an item in place (e.g., `locel_add_event`) need a stored item instead.
Failed calls raise `r4pm.BindingError`.

A binding that returns a value and also produces an item, like `extraction_run`, takes a
required `output_id`. It stores the item under that ID and returns the value (here the
`ExtractionReport`).

### Progress and cancellation

Long-running bindings report their progress. `r4pm.set_progress` shows it for every binding
call, and the `progress=` argument of each binding overrides that for one call:

```python
r4pm.set_progress("tqdm")      # one tqdm bar per stage (pip install r4pm[progress])
r4pm.set_progress(lambda stage, done, total: print(stage, done, total))  # total may be None
r4pm.set_progress(None)        # the default: no progress display

bindings.discover_oc_declare(locel, progress="tqdm")
bindings.discover_oc_declare(locel, progress=False)  # nothing for this call
```

Ctrl+C cancels a running binding, and so does an exception raised by the progress callback. The
call stops at the next cancellation check of the binding and stores nothing, then the
`KeyboardInterrupt` (or the callback's exception) is raised. A binding that never checks runs to
the end first. Warnings of a binding (e.g., skipped rows in an import) become Python
`UserWarning`s.

`r4pm.import_item` and `r4pm.df.import_ocel` (with its `_xml`/`_json`/`_pm4py` variants) work the
same way: they take `progress=` (for most file formats, stage `"Importing"` with the bytes read),
and a cancelled `import_item` adds nothing to the registry.

## Simple Import/Export API

For direct DataFrame operations without the registry, use the `df` submodule.

### XES
```python
import r4pm

# Import returns (DataFrame, log_attributes_json)
xes, attrs = r4pm.df.import_xes("file.xes", date_format="%Y-%m-%d")
r4pm.df.export_xes(xes, "test_data/output.xes")
```

### OCEL
```python
# Returns dict with DataFrames: events, objects, relations, o2o, object_changes
ocel = r4pm.df.import_ocel("file.xml")
print(ocel['events'].shape)
r4pm.df.export_ocel(ocel, "export.xml")

# PM4Py integration (requires pm4py)
ocel_pm4py = r4pm.df.import_ocel_pm4py("file.xml")
print(ocel['events'].shape)
r4pm.df.export_ocel_pm4py(ocel_pm4py, "export.xml")
```

### Petri Nets

A Petri net is a JSON-compatible dict (`r4pm.petri_net.PetriNet`) with places and transitions
in lists, identified by index. Each transition lists its input and output arcs as
`[place index, weight]`, a silent transition has label `None`, and markings map place indices to
token counts:

```python
from r4pm import petri_net

net = petri_net.import_pnml("model.pnml")
# {'places': [{'name': 'source'}, {'name': 'sink'}],
#  'transitions': [{'label': 'a', 'name': 't1', 'inputs': [[0, 1]], 'outputs': [[1, 1]]}],
#  'initial_marking': {'0': 1}, 'final_marking': {'1': 1}}
petri_net.export_pnml(net, "copy.pnml")

pm4py_net, im, fm = petri_net.to_pm4py(net)        # and back with petri_net.from_pm4py
handle = petri_net.to_handle(net)                  # keep it in the registry
```

## Conformance Checking

`r4pm.conformance` computes alignments, fitness and precision with the Rust implementation.
Logs can be pm4py `EventLog`s, pandas or polars DataFrames (pm4py column names), r4pm handles,
XES paths, or lists of activity sequences. Nets can be pm4py nets with markings, Petri net
dicts, handles, or PNML paths.

With pm4py objects, the results have pm4py's shape:

```python
import pm4py
from r4pm import conformance

log = pm4py.read_xes("log.xes")
net, im, fm = pm4py.discover_petri_net_inductive(log, noise_threshold=0.2)

conformance.fitness(log, net, im, fm)
# {'log_fitness': 0.962, 'average_trace_fitness': 0.907, 'percentage_of_fitting_traces': 62.6, ...}
conformance.precision(log, net, im, fm, preset="pm4py")   # as pm4py.precision_alignments

aligned = conformance.alignments(log, net, im, fm)          # one per trace, as pm4py
aligned[0]["alignment"]   # [('A', 'A'), ('>>', None), ('B', '>>'), ...]
conformance.precision(aligned)                              # reuses the alignments
```

With r4pm objects, nothing passes through Python but the result:

```python
import r4pm
from r4pm import bindings, conformance
from r4pm.conformance import AlignmentOptions

log = r4pm.import_item("EventLog", "log.xes")
net = bindings.discover_inductive_net(log)    # Petri net dict

conformance.fitness(log, net)
conformance.fitness(log, net, method="token_based")    # nets without silent transitions
conformance.precision(log, "model.pnml", preset="prom")
conformance.precision_result(log, net, report_escaping_edges=True)

options = AlignmentOptions(log_move_cost=2, max_states=1_000_000, max_memory_mib=4096)
aligned = conformance.alignments(log, net, options=options, num_threads=4)
for variant in aligned.variants:                   # typed, one per trace variant
    print(variant.frequency, variant.fitness, [m.kind for m in variant.moves or []])
```

Precision presets: `"default"` (paths from optimal alignments), `"prom"` (ProM's "Check
Precision using Escaping Edges") and `"pm4py"` (as `pm4py.precision_alignments`).

`r4pm.interop` converts event logs between pm4py and the registry (`log_from_pm4py`,
`log_to_pm4py`), and Petri nets (`net_from_pm4py`, `net_to_pm4py`).

## Reflow

Reflow decides which object types flow through each activity, so discovery runs on a smaller
log. It is available through the generated
bindings:

```python
import r4pm
from r4pm import bindings

with r4pm.import_item("OCEL", "order-management.json") as ocel:
    slim = ocel.convert("SlimLinkedOCEL")
    result = bindings.reflow(slim)                     # flow layer and cell states
    flow = bindings.reflow_flow_projection(bindings.reflow_tag(slim, result))
    model = bindings.reflow_annotated_ocpn(slim, result)
    svg = bindings.reflow_annotated_ocpn_svg(model)    # show in Jupyter with IPython.display.SVG
```

`reflow_tag` returns the log with non-flow participations marked by the qualifier prefix `!`,
readable by any OCEL 2.0 tool; `reflow_flow_projection` keeps only the flowing ones.

## Examples

`examples/` holds short scripts that check their own results: `progress.py` (progress, cancellation,
warnings) and `extraction.py` (OCEL extraction from SQLite with a blueprint). Run one with
`python examples/progress.py`. `pytest tests/test_examples.py` runs all of them.

## Development

### Setup
```bash
# Install Rust: https://rustup.rs/
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Create virtual environment
python -m venv .venv
source .venv/bin/activate

# Install in development mode
pip install maturin
maturin develop --release
```

### How Bindings Are Generated

Python bindings are **automatically generated during the Rust build** via `build.rs`. 
Thus, bindings are always in sync with the Rust code and do not require manual regeneration.

The build script:
1. Reads function metadata from the `process_mining` crate
2. Generates `r4pm/bindings/` with typed Python wrappers and `.pyi` stubs
3. Organizes functions by their Rust module structure

### Building for Release

```bash
maturin build --release  # Creates wheels in target/wheels/
```

The wheel automatically includes the generated bindings.

### Running Tests

```bash
pip install pytest pm4py polars pandas pyarrow
maturin develop
pytest
```

The test suite (`tests/`) covers conformance checking against pm4py (alignment fitness,
per-trace alignments, precision, token-based replay), the input and output forms, registry
handles, Petri net conversion, and the generated bindings and stubs.

## LICENSE
This package is licensed under either Apache License Version 2.0 or MIT License at your option. 