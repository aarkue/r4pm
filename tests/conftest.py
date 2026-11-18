import json
import warnings
from pathlib import Path

import pandas as pd
import pytest

warnings.filterwarnings("ignore", category=DeprecationWarning)


def make_df(traces, prefix="c"):
    """An event DataFrame in pm4py's column names, one case per trace"""
    start = pd.Timestamp("2024-01-01", tz="UTC")
    rows = [
        {
            "case:concept:name": f"{prefix}{i:03d}",
            "concept:name": activity,
            "time:timestamp": start + pd.Timedelta(minutes=100 * i + j),
        }
        for i, trace in enumerate(traces)
        for j, activity in enumerate(trace)
    ]
    return pd.DataFrame(rows)


# Fits the sequence a, (b || c), d, with some deviations
TRACES = (
    [["a", "b", "c", "d"]] * 6
    + [["a", "c", "b", "d"]] * 4
    + [["a", "b", "d"]] * 2
    + [["a", "e", "c", "d"]]
    + [["a", "b", "c", "d", "d"]]
    + [["b", "c", "d"]]
)

# A model with choices, a loop and silent transitions, so that precision is below 1
NOISY_TRACES = (
    [["a", "b", "d", "e"]] * 5
    + [["a", "c", "d", "e"]] * 3
    + [["a", "b", "d", "b", "d", "e"]] * 2
    + [["a", "e"]]
    + [["a", "c", "e"]]
    + [["a", "d", "b", "e"]]
)


@pytest.fixture
def df():
    return make_df(TRACES)


@pytest.fixture
def noisy_df():
    return make_df(NOISY_TRACES)


@pytest.fixture
def pm4py():
    return pytest.importorskip("pm4py")


@pytest.fixture
def ocel_path(tmp_path: Path) -> str:
    """A tiny OCEL 2.0 JSON file"""
    ocel = {
        "objectTypes": [{"name": "order", "attributes": []}, {"name": "item", "attributes": []}],
        "eventTypes": [{"name": "place", "attributes": []}, {"name": "ship", "attributes": []}],
        "objects": [
            {"id": "o1", "type": "order", "attributes": [], "relationships": [{"objectId": "i1", "qualifier": "contains"}]},
            {"id": "i1", "type": "item", "attributes": [], "relationships": []},
        ],
        "events": [
            {"id": "e1", "type": "place", "time": "2024-01-01T00:00:00Z", "attributes": [], "relationships": [{"objectId": "o1", "qualifier": ""}, {"objectId": "i1", "qualifier": ""}]},
            {"id": "e2", "type": "ship", "time": "2024-01-02T00:00:00Z", "attributes": [], "relationships": [{"objectId": "o1", "qualifier": ""}]},
        ],
    }
    path = tmp_path / "tiny.jsonocel"
    path.write_text(json.dumps(ocel))
    return str(path)
