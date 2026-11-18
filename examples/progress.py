"""Progress reports, cancellation and warnings of binding calls and imports.

A callback and tqdm bars for progress, `progress=False` for one quiet call, cancelling a call or
an import by raising from its callback, and a library warning arriving as a Python warning.
"""
import contextlib
import importlib.util
import io
import os
import tempfile
import warnings
from pathlib import Path

import r4pm
from r4pm import bindings

ORDERS = str(Path(__file__).resolve().parent.parent / "test_data" / "order-management.xml")


def registry_ids():
    return {item["id"] for item in r4pm.list_items()}


slim = r4pm.import_item("SlimLinkedOCEL", ORDERS)
log = bindings.flatten_ocel_on(slim, "items")
net = bindings.discover_inductive_net(log)

reports = []
r4pm.set_progress(lambda stage, done, total: reports.append((stage, done, total)))
bindings.align_variants(net, log)
stage, done, total = reports[-1]
print(f"{len(reports)} reports, last: {stage!r} {done}/{total}")
assert reports[0][1] == 0
assert done == total == bindings.num_variants(log)

reports.clear()
bindings.align_variants(net, log, progress=False)
assert reports == []
print("progress=False: no reports")
r4pm.set_progress(None)

if importlib.util.find_spec("tqdm") is None:
    print("tqdm is not installed, skipping the progress bars")
else:
    err = io.StringIO()
    with contextlib.redirect_stderr(err):
        bindings.align_variants(net, log, progress="tqdm")
    bar = err.getvalue().strip().split("\r")[-1].strip()
    print(f"tqdm: {bar}")
    assert "100%" in bar


def stop(stage, done, total):
    raise KeyboardInterrupt


before = registry_ids()
try:
    bindings.discover_ocpn(ORDERS, progress=stop)
except KeyboardInterrupt:
    print("discover_ocpn cancelled at its first progress report")
else:
    raise AssertionError("discover_ocpn was not cancelled")
# The log loaded from the path for that call is gone again
assert registry_ids() == before

reports = []
with r4pm.import_item("OCEL", ORDERS, progress=lambda *r: reports.append(r)):
    pass
size = os.path.getsize(ORDERS)
importing = [r for r in reports if r[0] == "Importing"]
print(f"import_item: {len(importing)} 'Importing' reports, last: {importing[-1]}")
assert importing[0] == ("Importing", 0, size), importing[0]
assert importing[-1] == ("Importing", size, size), importing[-1]

try:
    r4pm.import_item("OCEL", ORDERS, progress=stop)
except KeyboardInterrupt:
    print("import_item cancelled at its first progress report")
else:
    raise AssertionError("import_item was not cancelled")
assert registry_ids() == before

with tempfile.TemporaryDirectory() as tmp:
    bad = Path(tmp) / "bad-price.xml"
    bad.write_text(
        '<?xml version="1.0" encoding="UTF-8"?><log><object-types>'
        '<object-type name="order"><attributes><attribute name="price" type="float"/>'
        "</attributes></object-type></object-types><event-types>"
        '<event-type name="place"><attributes/></event-type></event-types><objects>'
        '<object id="o1" type="order"><attributes>'
        '<attribute name="price" time="2024-01-01T00:00:00Z">not a number</attribute>'
        "</attributes><objects/></object></objects><events>"
        '<event id="e1" type="place" time="2024-01-01T00:00:00Z"><attributes/><objects>'
        '<relationship object-id="o1" qualifier=""/></objects></event></events></log>'
    )
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        assert bindings.num_events(str(bad)) == 1
messages = [str(w.message) for w in caught if w.category is UserWarning]
print(f"warning: {messages[0]}" if messages else "no warning")
assert any("not a number" in m for m in messages), messages
