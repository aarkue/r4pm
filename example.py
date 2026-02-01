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
locel_id = bindings.index_link_ocel(ocel=ocel_id)
print(f"✓ Created: {locel_id[:30]}...")

# Get statistics
num = bindings.num_events(ocel=locel_id)
print(f"✅ Number of events: {num}")

# Check what's in the registry now
items = r4pm.list_items()
print(f"\n📋 Registry now has {len(items)} items:")
for item in items:
    print(f"   - {item['type']}: {item['id'][:30]}...")

# Process discovery
print("\n🔍 Discovering DFG...")
dfg = bindings.discover_dfg_from_locel(locel=locel_id)
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
