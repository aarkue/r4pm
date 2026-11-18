"""Extract an OCEL from a SQLite database with a blueprint.

The database has an orders and an items table. The blueprint maps their rows to order and item
objects, to place, ship and pick events, and to order-item relations.
"""
import sqlite3
import tempfile
from pathlib import Path

import r4pm
from r4pm import bindings


def col(name):
    return {"type": "column", "column": name}


def const(value):
    return {"type": "constant", "value": value}


def template(text):
    return {"type": "template", "template": text}


def at(column):
    return {"type": "value", "source": col(column)}


blueprint = {
    "version": 1,
    "nodes": [
        {"id": "orders", "op": {"type": "source", "source_id": "shop", "table": "orders"}},
        {"id": "items", "op": {"type": "source", "source_id": "shop", "table": "items"}},
    ],
    "mappings": [
        {"type": "single", "node": "orders", "target": {
            "type": "object", "object_type": const("order"), "id": col("order_id"),
            "attributes": [{"source_column": "customer", "name": "customer"}]}},
        {"type": "single", "node": "items", "target": {
            "type": "object", "object_type": const("item"), "id": col("item_id"),
            "attributes": [{"source_column": "product", "name": "product"},
                           {"source_column": "price", "name": "price"}]}},
        {"type": "single", "node": "orders", "target": {
            "type": "event", "event_type": const("place order"), "id": template("place-{order_id}"),
            "timestamp": at("placed_at"),
            "objects": [{"object": {"id": col("order_id")}, "qualifier": const("order")}]}},
        {"type": "single", "node": "orders",
         "when": {"type": "not", "condition": {"type": "is-null", "column": "shipped_at"}},
         "target": {
            "type": "event", "event_type": const("ship order"), "id": template("ship-{order_id}"),
            "timestamp": at("shipped_at"),
            "objects": [{"object": {"id": col("order_id")}, "qualifier": const("order")}]}},
        {"type": "single", "node": "items", "target": {
            "type": "event", "event_type": const("pick item"), "id": template("pick-{item_id}"),
            "timestamp": at("picked_at"),
            "objects": [{"object": {"id": col("item_id")}, "qualifier": const("item")},
                        {"object": {"id": col("order_id")}, "qualifier": const("order")}]}},
        {"type": "single", "node": "items", "target": {
            "type": "o2o", "source": {"id": col("order_id")}, "target": {"id": col("item_id")},
            "qualifier": const("contains")}},
    ],
}

with tempfile.TemporaryDirectory() as tmp:
    db = Path(tmp) / "shop.sqlite"
    con = sqlite3.connect(db)
    con.executescript("""
        CREATE TABLE orders (order_id TEXT PRIMARY KEY, customer TEXT, placed_at TEXT, shipped_at TEXT);
        CREATE TABLE items (item_id TEXT PRIMARY KEY, order_id TEXT, product TEXT, price REAL, picked_at TEXT);
        INSERT INTO orders VALUES
            ('o1', 'alice', '2024-01-01T09:00:00Z', '2024-01-03T12:00:00Z'),
            ('o2', 'bob',   '2024-01-02T10:00:00Z', NULL),
            ('o3', 'alice', '2024-01-02T11:00:00Z', '2024-01-04T08:00:00Z');
        INSERT INTO items VALUES
            ('i1', 'o1', 'pen', 1.5,  '2024-01-02T08:00:00Z'),
            ('i2', 'o1', 'ink', 4.0,  '2024-01-02T08:30:00Z'),
            ('i3', 'o2', 'pad', 2.25, '2024-01-03T09:00:00Z'),
            ('i4', 'o3', 'pen', 1.5,  '2024-01-03T10:00:00Z');
    """)
    con.commit()
    con.close()

    connections = {"shop": f"sqlite:{db}"}
    catalog = bindings.extraction_discover_catalog(connections)
    print("tables:", sorted(catalog["tables"]["shop"]))
    problems = bindings.extraction_validate(blueprint, catalog)
    assert problems == [], problems

    report = bindings.extraction_run(blueprint, connections, output_id="shop-log")

for stats in report["per_mapping"]:
    print(f"{stats['mapping']['describes']}: {stats['rows_read']} rows, dropped {stats['dropped']}")
assert report["errors"] == []
assert [s["rows_read"] for s in report["per_mapping"]] == [3, 4, 3, 3, 4, 4]
# o2 has no shipped_at, so the ship mapping's condition excludes it
assert report["per_mapping"][3]["dropped"] == {"PredicateExcluded": 1}

counts = bindings.ocel_type_stats("shop-log")
print("events:", sorted(counts["event_type_counts"].items()))
print("objects:", sorted(counts["object_type_counts"].items()))
assert counts["event_type_counts"] == {"place order": 3, "ship order": 2, "pick item": 4}
assert counts["object_type_counts"] == {"order": 3, "item": 4}
assert bindings.num_events("shop-log") == 9 and bindings.num_objects("shop-log") == 7
assert r4pm.item_to_df("shop-log")["o2o"].height == 4

# An explicit output_id is not owned by any handle, so it stays until removed
r4pm.remove_item("shop-log")
