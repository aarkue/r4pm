import polars as pl
import pytest

import r4pm
from r4pm import bindings


@pytest.fixture
def log(tmp_path, df):
    path = str(tmp_path / "log.xes")
    r4pm.df.export_xes(pl.from_pandas(df), path)
    with r4pm.import_item("EventLog", path) as log:
        yield log


def test_drawing_bindings_return_svg(log, tmp_path):
    net = bindings.discover_inductive_net(log)
    svg = bindings.petri_net_svg(net)
    assert isinstance(svg, r4pm.Svg) and isinstance(svg, str)
    assert "<svg" in svg
    assert svg._repr_svg_() == str(svg)
    assert repr(svg) == f"<Svg, {len(svg)} characters>"

    path = tmp_path / "net.svg"
    svg.save(path)
    assert path.read_text(encoding="utf-8") == svg
