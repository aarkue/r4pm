#!/usr/bin/env python3
"""
Comprehensive test of all r4pm features.
Run this to verify everything works correctly.
"""

from r4pm import bindings
import r4pm

def test_basic_workflow():
    """Test basic load, convert, analyze workflow."""
    print("\n" + "="*60)
    print("TEST 1: Basic Workflow")
    print("="*60)
    
    # Load OCEL
    ocel_id = r4pm.import_item('OCEL', 'test_data/order-management.xml')
    print(f"✓ Loaded OCEL: {ocel_id[:30]}...")
    
    # Convert to IndexLinkedOCEL
    locel_id = bindings.index_link_ocel(ocel=ocel_id)
    print(f"✓ Converted to IndexLinkedOCEL: {locel_id[:30]}...")
    
    # Use analysis functions
    num = bindings.num_events(ocel=locel_id)
    print(f"✓ num_events: {num}")
    
    num2 = bindings.num_objects(ocel=locel_id)
    print(f"✓ num_objects: {num2}")
    
    assert num == 21008, f"Event count mismatch: expected 21008, got {num}"
    
    # Check registry has both items
    items = r4pm.list_items()
    assert len(items) == 2, f"Expected 2 items, got {len(items)}"
    types = {item['type'] for item in items}
    assert 'OCEL' in types and 'IndexLinkedOCEL' in types, f"Wrong types: {types}"
    print(f"✓ Registry has {len(items)} items: {', '.join(types)}")
    
    # Cleanup
    for item in items:
        r4pm.remove_item(item['id'])
    print("✓ Cleaned up\n")


def test_process_discovery():
    """Test process discovery algorithms."""
    print("="*60)
    print("TEST 2: Process Discovery")
    print("="*60)
    
    # Load and convert OCEL
    ocel_id = r4pm.import_item('OCEL', 'test_data/order-management.xml')
    locel_id = bindings.index_link_ocel(ocel=ocel_id)
    
    # DFG from IndexLinkedOCEL
    dfg = bindings.discover_dfg_from_locel(locel=locel_id)
    object_types = list(dfg['object_type_to_dfg'].keys())
    print(f"✓ DFG discovered for {len(object_types)} object types")
    print(f"  Types: {', '.join(object_types)}")
    
    # OC-Declare needs SlimLinkedOCEL - load directly as that type
    slim_id = r4pm.import_item('SlimLinkedOCEL', 'test_data/order-management.xml')
    
    # Test with defaults (no options) - Rust provides default values
    constraints = bindings.discover_oc_declare(locel=slim_id)
    print(f"✓ Discovered {len(constraints)} OC-Declare constraints (using defaults)")
    
    # Also test with explicit options to verify both work
    options = {
        "noise_threshold": 0.0,
        "o2o_mode": "None",
        "counts_for_generation": [None, None],
        "counts_for_filter": [None, None],
        "reduction": "None",
        "refinement": False,
        "considered_arrow_types": ["AS"]
    }
    constraints_explicit = bindings.discover_oc_declare(locel=slim_id, options=options)
    print(f"✓ Discovered {len(constraints_explicit)} OC-Declare constraints (with explicit options)")
    
    # Cleanup
    for item in r4pm.list_items():
        r4pm.remove_item(item['id'])
    print("✓ Cleaned up\n")


def test_registry_operations():
    """Test registry CRUD operations."""
    print("="*60)
    print("TEST 3: Registry Operations")
    print("="*60)
    
    # Load with custom ID
    ocel_id = r4pm.import_item('OCEL', 'test_data/order-management.xml', 'my-ocel')
    assert ocel_id == 'my-ocel', f"Custom ID failed: {ocel_id}"
    print(f"✓ Loaded with custom ID: {ocel_id}")
    
    # Convert
    linked_id = r4pm.convert_item(ocel_id, 'IndexLinkedOCEL', 'my-linked')
    assert linked_id == 'my-linked', f"Custom conversion ID failed: {linked_id}"
    print(f"✓ Converted to IndexLinkedOCEL: {linked_id}")
    
    # Get as DataFrames
    dfs = r4pm.item_to_df(ocel_id)
    assert 'events' in dfs and 'objects' in dfs
    print(f"✓ Got DataFrames: events={dfs['events'].shape}, objects={dfs['objects'].shape}")
    
    # Export
    r4pm.export_item(ocel_id, '/tmp/test-export.xml')
    print(f"✓ Exported to /tmp/test-export.xml")
    
    # List
    items = r4pm.list_items()
    assert len(items) == 2
    print(f"✓ Listed {len(items)} items")
    
    # Remove
    for item in items:
        r4pm.remove_item(item['id'])
    assert len(r4pm.list_items()) == 0
    print("✓ Removed all items\n")


def test_simple_import_export_api():
    """Test simple DataFrame import/export."""
    print("="*60)
    print("TEST 4: Simple DataFrame API")
    print("="*60)
    
    # XES import/export
    xes, attrs = r4pm.import_xes('test.xes')
    print(f"✓ Imported XES: {xes.shape}")
    
    r4pm.export_xes(xes, '/tmp/test-export.xes')
    print(f"✓ Exported XES")
    
    # Verify round-trip
    xes2, _ = r4pm.import_xes('/tmp/test-export.xes')
    assert xes.shape == xes2.shape
    print(f"✓ Round-trip successful")
    
    # OCEL import
    ocel = r4pm.import_ocel_xml('test_data/order-management.xml')
    assert 'events' in ocel and 'objects' in ocel
    print(f"✓ Imported OCEL: events={ocel['events'].shape}, objects={ocel['objects'].shape}")
    print()


def test_edge_cases():
    """Test edge cases and error handling."""
    print("="*60)
    print("TEST 5: Edge Cases")
    print("="*60)
    
    # Auto-generated ID
    ocel_id = r4pm.import_item('OCEL', 'test_data/order-management.xml')
    assert len(ocel_id) == 36  # UUID length
    print(f"✓ Auto-generated ID: {ocel_id}")
    
    # Convert and verify
    locel_id = bindings.index_link_ocel(ocel=ocel_id)
    num1 = bindings.num_events(ocel=locel_id)
    num2 = bindings.num_events(ocel=locel_id)
    assert num1 == num2
    print(f"✓ Consistent results: {num1}")
    
    # Cleanup
    for item in r4pm.list_items():
        r4pm.remove_item(item['id'])
    print("✓ Cleaned up\n")


def test_dataframe_roundtrip():
    """Test bidirectional DataFrame conversion for OCEL and EventLog."""
    print("="*60)
    print("TEST 6: DataFrame Round-Trip Conversion")
    print("="*60)
    
    # OCEL round-trip
    ocel_id = r4pm.import_item('OCEL', 'test_data/order-management.xml')
    dfs = r4pm.item_to_df(ocel_id)
    print(f"✓ OCEL to DataFrames: {list(dfs.keys())}")
    
    events_before = dfs['events'].shape[0]
    objects_before = dfs['objects'].shape[0]
    
    new_ocel_id = r4pm.import_item_from_df('OCEL', dfs)
    new_dfs = r4pm.item_to_df(new_ocel_id)
    
    events_after = new_dfs['events'].shape[0]
    objects_after = new_dfs['objects'].shape[0]
    
    assert events_before == events_after, f"Events mismatch: {events_before} vs {events_after}"
    assert objects_before == objects_after, f"Objects mismatch: {objects_before} vs {objects_after}"
    print(f"✓ OCEL round-trip: {events_before} events, {objects_before} objects preserved")
    
    # EventLog round-trip
    log_id = r4pm.import_item('EventLog', 'test.xes')
    df = r4pm.item_to_df(log_id)
    shape_before = df.shape
    
    new_log_id = r4pm.import_item_from_df('EventLog', df)
    new_df = r4pm.item_to_df(new_log_id)
    shape_after = new_df.shape
    
    assert shape_before == shape_after, f"EventLog shape mismatch: {shape_before} vs {shape_after}"
    print(f"✓ EventLog round-trip: {shape_before} preserved")
    
    # Cleanup
    for item in r4pm.list_items():
        r4pm.remove_item(item['id'])
    print("✓ Cleaned up\n")


if __name__ == '__main__':
    print("\n" + "█"*60)
    print("█" + " "*58 + "█")
    print("█" + "  r4pm - Comprehensive Test Suite".center(58) + "█")
    print("█" + " "*58 + "█")
    print("█"*60)
    
    try:
        test_basic_workflow()
        test_process_discovery()
        test_registry_operations()
        test_simple_import_export_api()
        test_edge_cases()
        test_dataframe_roundtrip()
        
        print("="*60)
        print("✅ ALL TESTS PASSED!")
        print("="*60)
        print()
        
    except Exception as e:
        print("\n" + "="*60)
        print("❌ TEST FAILED!")
        print("="*60)
        print(f"Error: {e}")
        import traceback
        traceback.print_exc()
        exit(1)
