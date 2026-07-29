# r4pm

Python bindings for the Rust4PM Project: Process mining in Python with the speed of Rust

This library provides basic import/export of XES/OCEL event data, as well as other exposed functionality from the [Rust4PM project](https://github.com/aarkue/rust4pm) (e.g., process discovery algorithms).

## Features

- **Fast XES/OCEL Import/Export**: Efficient Rust-based import and export of `.xes`, `.xes.gz`, and OCEL2 (`.xml`/`.json`) files
- **Auto-Generated Bindings**: All process_mining functions automatically exposed with full IDE support (autocomplete, type hints, docs)
- **Registry System**: Manage data objects and convert between types as needed
- **Polars DataFrames**: Polars facilitates the fast transfer of event data from Python to Rust and vice versa

## Installation

```bash
pip install r4pm         # default
pip install r4pm-full    # additionally bundles DuckDB (~15 MiB larger)
```

Both provide the `r4pm` import package, so install one or the other. `r4pm-full` adds the DuckDB
bindings (`stream_ocel_to_duckdb`, `read_consolidated_ocel_from_duckdb`, ...), nothing else differs.

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

# Load an OCEL file - returns a registry ID
ocel_id = r4pm.import_item('OCEL', 'data/orders.xml')

# Convert to SlimLinkedOCEL for analysis functions
locel_id = bindings.slim_link_ocel(ocel=ocel_id)

# Get statistics
num = bindings.num_events(ocel=locel_id)
print(f"Events: {num}")

# Discover object-centric DFG
dfg = bindings.discover_dfg_from_ocel(locel_id)
print(f"Discovered DFG for {len(dfg['object_type_to_dfg'])} object types")

# For case-centric event logs:
log_id = r4pm.import_item('EventLog', 'data/log.xes')
case_dfg = bindings.discover_dfg(log_id)
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

```python
# Load files into registry
ocel_id = r4pm.import_item('OCEL', 'file.xml')
log_id = r4pm.import_item('EventLog', 'file.xes')

# Convert between types (either like this or using r4pm.convert_item)
locel_id = bindings.index_link_ocel(ocel=ocel_id)
proj_id = bindings.log_to_activity_projection(log=log_id)

# List registry contents
for item in r4pm.list_items():
    print(f"{item['id']}: {item['type']}")
```


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

### Petri Nets & Alignments

A Petri net is a plain JSON-compatible dict (`r4pm.petri_net.PetriNet`). Import/export
PNML, convert to/from PM4Py, and compute alignment-based fitness with the fast Rust
alignment implementation.

```python
import pm4py
import r4pm
from r4pm import petri_net
from r4pm.bindings.conformance.case_centric.alignments import align_variants, compute_fitness

LOG = "test_data/Sepsis Cases - Event Log.xes.gz"

# 1. Discover a Petri net with PM4Py (Inductive Miner infrequent, 0.2 noise threshold)
log = pm4py.read_xes(LOG)
net, im, fm = pm4py.discover_petri_net_inductive(log, noise_threshold=0.2)

# 2. Convert the PM4Py net (+ markings) to an r4pm Petri net dict
rnet = petri_net.from_pm4py(net, im, fm)
# petri_net.export_pnml(rnet, "model.pnml")     # write PNML
# rnet = petri_net.import_pnml("model.pnml")    # or read PNML directly

# 3. Load the log into the registry
log_id = r4pm.import_item("EventLog", LOG)

# 4. Align all variants with the Rust binding and compute fitness
#    (the EventLog id is auto-projected to activity variants)
align_res = align_variants(rnet, log_id)
fitness = compute_fitness(align_res, rnet)
print(fitness)
# {'log_fitness': 0.962, 'average_fitness': 0.907,
#  'perfectly_fitting_frac': 0.626, 'total_costs': 573}
```

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
# Run comprehensive test suite
python test_all.py

# Run simple example
python example.py
```

The test suite (`test_all.py`) covers:
- Automatic type conversion (positional & keyword arguments)
- Process discovery (DFG, OC-Declare)
- Registry operations (CRUD, DataFrames, export)
- Simple Import/Export DataFrame (`df`) API
- Edge cases and conversion caching


## LICENSE
This package is licensed under either Apache License Version 2.0 or MIT License at your option. 