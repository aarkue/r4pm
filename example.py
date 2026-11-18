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
ocel_id = r4pm.convert_item(ocel_id, 'SlimLinkedOCEL')
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
dfg = bindings.discover_ocdfg(ocel_id)
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
from r4pm import conformance

LOG = "test_data/Sepsis Cases - Event Log.xes.gz"

# 1. Discover a Petri net with PM4Py (Inductive Miner infrequent, 0.2 noise threshold)
print("\n🔍 Discovering Petri net with IMf (noise_threshold=0.2)...")
log = pm4py.read_xes(LOG)
net, im, fm = pm4py.discover_petri_net_inductive(log, noise_threshold=0.2)
print(f"✓ Discovered net: {len(net.places)} places, {len(net.transitions)} transitions")

# 2. Align all traces with the Rust implementation (pm4py objects are converted)
print("🧮 Aligning all traces with the Rust implementation...")
aligned = conformance.alignments(log, net, im, fm)
print(f"✓ Aligned {len(aligned)} traces ({len(aligned.variants)} variants)")

# 3. Fitness and precision, reusing the alignments
for key, value in conformance.fitness(aligned).items():
    print(f"✅ {key}: {value}")
print(f"✅ precision: {conformance.precision(aligned)}")
