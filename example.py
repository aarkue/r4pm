#!/usr/bin/env python3
"""
Simple example demonstrating r4pm key features
"""

from r4pm import bindings
import r4pm

print("=" * 60)
print("r4pm - Process Mining Demo")
print("=" * 60)

# Load OCEL file
ocel_id = r4pm.import_item('OCEL', 'test_data/order-management.xml')
print(f"\n✓ Loaded OCEL: {ocel_id[:30]}...")

# Convert to IndexLinkedOCEL for analysis
print("\n🔄 Converting to IndexLinkedOCEL...")
ocel_id = bindings.slim_link_ocel(ocel_id)
print(f"✓ Created: {ocel_id[:30]}...")

# Get statistics
num = bindings.num_events(ocel_id)
print(f"✅ Number of events: {num}")

# Check what's in the registry now
items = r4pm.list_items()
print(f"\n📋 Registry now has {len(items)} items:")
for item in items:
    print(f"   - {item['type']}: {item['id'][:30]}...")

# Process discovery
print("\n🔍 Discovering DFG...")
dfg = bindings.discover_dfg_from_ocel(ocel_id)
object_types = list(dfg['object_type_to_dfg'].keys())
print(f"✓ DFG discovered for {len(object_types)} object types: {', '.join(object_types)}")

# Get as DataFrames
print("\n📊 Getting as DataFrames...")
dfs = r4pm.item_to_df(ocel_id)
print(f"✓ Events: {dfs['events'].shape}")
print(f"✓ Objects: {dfs['objects'].shape}")

# Cleanup
print("\n🧹 Cleaning up registry...")
for item in items:
    r4pm.remove_item(item['id'])
print("✅ Done!")


# ---------------------------------------------------------------------------
# Petri net discovery (PM4Py) + alignment-based fitness (Rust)
# ---------------------------------------------------------------------------
print("\n" + "=" * 60)
print("Petri net alignment demo (PM4Py discovery + Rust alignment)")
print("=" * 60)

import pm4py
from r4pm import petri_net
from r4pm.bindings.conformance.case_centric.alignments import (
    align_variants,
    compute_fitness,
)

LOG = "test_data/Sepsis Cases - Event Log.xes.gz"

# 1. Discover a Petri net with PM4Py (Inductive Miner infrequent, 0.2 noise threshold)
print("\n🔍 Discovering Petri net with IMf (noise_threshold=0.2)...")
log = pm4py.read_xes(LOG)
net, im, fm = pm4py.discover_petri_net_inductive(log, noise_threshold=0.2)
print(f"✓ Discovered net: {len(net.places)} places, {len(net.transitions)} transitions")

# 2. Convert the PM4Py net to an r4pm Petri net dict
rnet = petri_net.from_pm4py(net, im, fm)

# 3. Load the log into the registry
log_id = r4pm.import_item("EventLog", LOG)

# 4. Align all variants with the Rust binding and compute fitness
#    (the EventLog id is auto-projected to activity variants)
print("🧮 Aligning all variants with the Rust binding...")
align_res = align_variants(rnet, log_id)
fitness = compute_fitness(align_res, rnet)
print(f"✓ Aligned {len(align_res)} variants")
for key, value in fitness.items():
    print(f"✅ {key}: {value}")

r4pm.remove_item(log_id)
