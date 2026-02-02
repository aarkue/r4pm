# r4pm

Python bindings for the Rust4PM Project: Process mining in Python with the speed of Rust

This library provides basic import/export of XES/OCEL event data, as well as other exposed functionality from the [Rust4PM project](https://github.com/aarkue/rust4pm) (e.g., process discovery algorithms).

## Features

- **Fast XES/OCEL Import/Export**: Efficient Rust-based import and export of `.xes`, `.xes.gz`, and OCEL2 (`.xml`/`.json`) files
- **Auto-Generated Bindings**: All process_mining functions automatically exposed with full IDE support (autocomplete, type hints, docs)
- **Registry System**: Manage data objects and convert between types as needed
- **Polars DataFrames**: Polars facilitates the fast transfer of event data from Python to Rust and vice versa

## Quick Start

```python
from r4pm import bindings
import r4pm

# Load an OCEL file - returns a registry ID
ocel_id = r4pm.import_item('OCEL', 'data/orders.xml')

# Convert to IndexLinkedOCEL for analysis functions
locel_id = bindings.index_link_ocel(ocel=ocel_id)

# Get statistics
num = bindings.num_events(ocel=locel_id)
print(f"Events: {num}")

# Discover object-centric DFG
dfg = bindings.discover_dfg_from_locel(locel=locel_id)
print(f"Discovered DFG for {len(dfg['object_type_to_dfg'])} object types")

# For case-centric event logs:
log_id = r4pm.import_item('EventLog', 'data/log.xes')
case_dfg = bindings.discover_dfg(event_log=log_id)
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
- `IndexLinkedOCEL` - Indexed OCEL for analysis (required by most functions)
- `SlimLinkedOCEL` - Memory-efficient linked OCEL
- `EventLog` - Case-centric event log
- `EventLogActivityProjection` - Activity-projected log for discovery

```python
# Load files into registry
ocel_id = r4pm.import_item('OCEL', 'file.xml')
log_id = r4pm.import_item('EventLog', 'file.xes')

# Convert between types
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