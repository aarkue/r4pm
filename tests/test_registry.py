import gc
import json
import pickle

import pytest

import r4pm
from r4pm import bindings, handles, petri_net
from r4pm.bindings.conformance.case_centric.alignments import align_variants, compute_fitness

from conftest import TRACES


def ids():
    return {item["id"] for item in r4pm.list_items()}


@pytest.fixture
def xes_path(tmp_path, df):
    path = str(tmp_path / "log.xes")
    r4pm.df.export_xes(__import__("polars").from_pandas(df), path)
    return path


def test_import_item_returns_owning_handle(xes_path):
    log = r4pm.import_item("EventLog", xes_path)
    assert isinstance(log, handles.EventLog) and isinstance(log, str)
    assert log.kind == "EventLog" and log.owned and log.exists()
    assert repr(log) == f"EventLog({log.id!r})"
    assert log in ids()
    log_id = log.id
    del log
    gc.collect()
    assert log_id not in ids()


def test_context_manager_and_explicit_ids(xes_path):
    with r4pm.import_item("EventLog", xes_path) as log:
        assert log.exists()
    assert log.closed and not log.exists()

    named = r4pm.import_item("EventLog", xes_path, item_id="my-log")
    assert named == "my-log" and not named.owned
    del named
    gc.collect()
    assert "my-log" in ids()
    plain = handles.Handle("my-log")
    assert plain.kind == "EventLog"
    assert handles.wrap("my-log").__class__ is handles.EventLog
    assert r4pm.remove_item("my-log")


def test_handles_work_as_plain_ids(xes_path):
    with r4pm.import_item("EventLog", xes_path) as log:
        assert bindings.num_cases(log) == len(TRACES)
        assert bindings.num_cases(log.id) == len(TRACES)
        assert r4pm.item_kind(log) == "EventLog"
        copy = pickle.loads(pickle.dumps(log))
        assert copy == log and not copy.owned
        assert {"id": log.id, "type": "EventLog"} in r4pm.list_items()


def test_bindings_return_handles(ocel_path):
    with r4pm.import_item("OCEL", ocel_path) as ocel:
        before = ids()
        log = bindings.flatten_ocel_on(ocel, "order")
        assert isinstance(log, handles.EventLog) and log.owned
        assert bindings.num_cases(log) == 1
        gc.collect()
        assert ids() == before | {log.id}
        named = bindings.flatten_ocel_on(ocel, "order", output_id="flat")
        assert named == "flat" and not named.owned
        named.close()
        slim = ocel.convert("SlimLinkedOCEL")
        assert isinstance(slim, handles.SlimLinkedOCEL) and slim.owned
        assert bindings.num_events(slim) == 2
        assert bindings.num_events(ocel) == 2
        assert slim.convert("IndexLinkedOCEL").kind == "IndexLinkedOCEL"
        assert slim.to_df()["events"].shape[0] == 2


def test_path_arguments_are_not_leaked(xes_path):
    before = ids()
    assert bindings.num_cases(xes_path) == len(TRACES)
    assert ids() == before


def test_binding_errors():
    with pytest.raises(r4pm.BindingError):
        bindings.num_cases("no-such-id-or-file")
    assert issubclass(r4pm.BindingError, TypeError)


def test_list_item_kinds():
    kinds = r4pm.list_item_kinds()
    for kind in ["EventLog", "OCEL", "SlimLinkedOCEL", "AcceptingPetriNet", "ProcessTree"]:
        assert kind in kinds
        if kind in handles._KINDS:
            assert handles._KINDS[kind].kind_name == kind


def test_discovered_net_flows_through_bindings(xes_path, tmp_path):
    with r4pm.import_item("EventLog", xes_path) as log:
        net = bindings.discover_inductive_net(log)
        assert set(net) >= {"places", "transitions", "initial_marking", "final_marking"}
        variants = align_variants(net, log)
        fit = compute_fitness(net, variants)
        assert 0 < fit["log_fitness"] <= 1

        # A net handle is resolved to its JSON for bindings that take the net as JSON
        with petri_net.to_handle(net) as net_handle:
            assert align_variants(net_handle, log) == variants
            pnml = str(tmp_path / "net.pnml")
            net_handle.export(pnml)
        loaded = petri_net.import_pnml(pnml)
        assert len(loaded["transitions"]) == len(net["transitions"])
        assert loaded.get("initial_marking") == net["initial_marking"]


def test_petri_net_pm4py_round_trip(pm4py, df, tmp_path):
    net, im, fm = pm4py.discover_petri_net_inductive(df, noise_threshold=0.2)
    as_dict = petri_net.from_pm4py(net, im, fm)
    assert {t.get("name") for t in as_dict["transitions"]} == {t.name for t in net.transitions}
    assert sorted(t.get("label") or "" for t in as_dict["transitions"]) == sorted(
        t.label or "" for t in net.transitions
    )
    back, im2, fm2 = petri_net.to_pm4py(as_dict)
    assert {(t.name, t.label) for t in back.transitions} == {(t.name, t.label) for t in net.transitions}
    assert {p.name for p in back.places} == {p.name for p in net.places}
    arcs = lambda n: {(a.source.name, a.target.name, a.weight) for a in n.arcs}
    assert arcs(back) == arcs(net)
    assert {p.name: c for p, c in im2.items()} == {p.name: c for p, c in im.items()}
    assert {p.name: c for p, c in fm2.items()} == {p.name: c for p, c in fm.items()}

    path = str(tmp_path / "net.pnml")
    petri_net.export_pnml(as_dict, path)
    # PNML export may give nodes new ids (names) where the names are not valid PNML ids
    unnamed = lambda n: {**n, "places": [{}] * len(n["places"]), "transitions": [{k: v for k, v in t.items() if k != "name"} for t in n["transitions"]]}
    assert unnamed(petri_net.import_pnml(path)) == unnamed(as_dict)
    pnet, pim, pfm = pm4py.read_pnml(path)
    assert {t.label for t in pnet.transitions} == {t.label for t in net.transitions}
    assert len(pim) == 1 and len(pfm) == 1


def test_pnml_without_markings(tmp_path):
    net: petri_net.PetriNet = {
        "places": [{"name": "start"}, {"name": "end"}],
        "transitions": [{"label": "a", "inputs": [(0, 1)], "outputs": [(1, 2)]}],
    }
    path = str(tmp_path / "plain.pnml")
    petri_net.export_pnml(net, path)
    loaded = petri_net.import_pnml(path)
    assert loaded.get("final_marking") is None
    assert loaded["transitions"][0]["outputs"] == [[1, 2]]
    with pytest.raises(ValueError, match="marking"):
        petri_net.to_handle(loaded)


def test_log_interop(pm4py, df, tmp_path):
    from r4pm import interop

    with interop.log_from_pm4py(df) as log:
        assert bindings.num_cases(log) == len(TRACES)
        back = interop.log_to_pm4py(log)
    assert back["case:concept:name"].nunique() == len(TRACES)
    assert list(back["concept:name"]) == list(df["concept:name"])
    assert pm4py.fitness_token_based_replay(back, *pm4py.discover_petri_net_alpha(df))["log_fitness"] > 0

    renamed = df.rename(columns={"case:concept:name": "case", "concept:name": "act"})
    with interop.log_from_pm4py(renamed, case_id_key="case", activity_key="act") as log:
        assert bindings.num_cases(log) == len(TRACES)
    with interop.log_from_pm4py(pm4py.convert_to_event_log(df)) as log:
        assert bindings.num_variants(log) == len({tuple(t) for t in TRACES})

    net, im, fm = pm4py.discover_petri_net_inductive(df)
    with interop.net_from_pm4py(net, im, fm, as_handle=True) as handle:
        pn, im2, fm2 = interop.net_to_pm4py(handle)
    assert len(pn.transitions) == len(net.transitions) and len(im2) == len(im)


def test_every_registry_kind_has_a_handle_class():
    missing = [k for k in r4pm.list_item_kinds() if handles.wrap("x", k).__class__ is handles.Handle]
    assert missing == []


def test_reflow_through_bindings(ocel_path):
    with r4pm.import_item("OCEL", ocel_path) as ocel:
        slim = ocel.convert("SlimLinkedOCEL")
        result = bindings.reflow(slim)
        assert isinstance(result, handles.ReflowResult)
        assert "cells" in bindings.reflow_cells(result)
        tagged = bindings.reflow_tag(slim, result)
        flow = bindings.reflow_flow_projection(tagged)
        assert bindings.num_events(flow) == bindings.num_events(slim)
        model = bindings.reflow_annotated_ocpn(slim, result)
        assert isinstance(model, handles.AnnotatedOcpn)
        assert bindings.reflow_annotated_ocpn_svg(model).lstrip().startswith("<")
