import signal
import sys
import threading
import time
import warnings
from pathlib import Path

import pytest

import r4pm
from r4pm import bindings
from r4pm.bindings.conformance.case_centric.alignments import align_variants

ORDER_MANAGEMENT = str(Path(__file__).parent.parent / "test_data" / "order-management.xml")


def ids():
    return {item["id"] for item in r4pm.list_items()}


@pytest.fixture
def log(tmp_path, df):
    path = str(tmp_path / "log.xes")
    r4pm.df.export_xes(__import__("polars").from_pandas(df), path)
    with r4pm.import_item("EventLog", path) as log:
        yield log


@pytest.fixture(autouse=True)
def reset_progress():
    yield
    r4pm.set_progress(None)


def test_callback_receives_first_and_last_report_of_a_stage(log):
    net = bindings.discover_inductive_net(log)
    reports = []
    align_variants(net, log, progress=lambda *r: reports.append(r))
    stages = [s for s, _, _ in reports]
    assert stages, reports
    stage = stages[-1]
    of_stage = [r for r in reports if r[0] == stage]
    total = of_stage[0][2]
    assert isinstance(total, int) and total > 0
    assert of_stage[0][1] == 0
    assert of_stage[-1][1] == total


def test_global_setting_and_per_call_override(log):
    net = bindings.discover_inductive_net(log)
    global_reports, call_reports = [], []
    r4pm.set_progress(lambda *r: global_reports.append(r))
    assert r4pm.get_progress() is not None

    align_variants(net, log)
    assert global_reports
    global_reports.clear()

    align_variants(net, log, progress=False)
    assert global_reports == []

    align_variants(net, log, progress=lambda *r: call_reports.append(r))
    assert global_reports == [] and call_reports

    r4pm.set_progress(None)
    assert r4pm.get_progress() is None


def test_invalid_progress_value():
    with pytest.raises(ValueError):
        r4pm.set_progress("bars")  # type: ignore[arg-type]
    with pytest.raises(ValueError):
        bindings.num_events(ORDER_MANAGEMENT, progress=1)  # type: ignore[arg-type]


@pytest.mark.parametrize("error", [KeyboardInterrupt, ValueError])
def test_exception_in_callback_cancels_the_call(error):
    def callback(stage, done, total):
        raise error("stop")

    before = ids()
    start = time.monotonic()
    with pytest.raises(error, match="stop"):
        bindings.discover_ocpn(ORDER_MANAGEMENT, progress=callback)
    assert time.monotonic() - start < 5
    assert ids() == before


def test_sigint_cancels_a_call_without_progress():
    before = ids()
    finished = False
    timer = threading.Timer(0.05, signal.raise_signal, (signal.SIGINT,))
    with pytest.raises(KeyboardInterrupt):
        timer.start()
        bindings.discover_ocpn(ORDER_MANAGEMENT)
        finished = True
        time.sleep(5)  # the signal is raised here if the call ended before it arrived
    timer.join()
    if finished:
        pytest.skip("the call finished before the signal arrived")
    assert ids() == before
    # The interrupted call left nothing behind that breaks the next one
    assert bindings.num_events(ORDER_MANAGEMENT) == 21008


def test_importer_warnings_become_python_warnings(tmp_path):
    path = tmp_path / "bad.xml"
    path.write_text(
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
    with warnings.catch_warnings(record=True) as record:
        warnings.simplefilter("always")
        assert bindings.num_events(str(path)) == 1
    assert any(
        w.category is UserWarning and "not a number" in str(w.message) for w in record
    ), record


def test_tqdm_bars(log, capsys):
    pytest.importorskip("tqdm")
    net = bindings.discover_inductive_net(log)
    align_variants(net, log, progress="tqdm")
    err = capsys.readouterr().err
    assert "Aligning" in err and "100%" in err

    r4pm.set_progress("tqdm")
    align_variants(net, log)
    assert "100%" in capsys.readouterr().err


def test_tqdm_bars_are_closed_on_cancel(monkeypatch):
    tqdm_auto = pytest.importorskip("tqdm.auto")
    bars = []

    class Bar(tqdm_auto.tqdm):
        def __init__(self, *args, **kwargs):
            super().__init__(*args, **kwargs)
            self.was_closed = False
            bars.append(self)

        def update(self, n=1):
            super().update(n)
            raise KeyboardInterrupt

        def close(self):
            self.was_closed = True
            super().close()

    monkeypatch.setattr(tqdm_auto, "tqdm", Bar)
    with pytest.raises(KeyboardInterrupt):
        bindings.num_events(ORDER_MANAGEMENT, progress="tqdm")
    assert bars and all(bar.was_closed for bar in bars)


def test_missing_tqdm_is_explained(monkeypatch):
    monkeypatch.setitem(sys.modules, "tqdm.auto", None)
    with pytest.raises(ImportError, match=r"r4pm\[progress\]"):
        r4pm.set_progress("tqdm")
    with pytest.raises(ImportError, match=r"r4pm\[progress\]"):
        bindings.num_events(ORDER_MANAGEMENT, progress="tqdm")
