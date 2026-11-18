import polars as pl
import pytest

import r4pm
from r4pm import conformance, petri_net
from r4pm.conformance import AlignmentOptions, ConformanceError

from conftest import NOISY_TRACES, TRACES, make_df


def handmade_net(pm4py):
    """a, then (b | c | skip), then (d | skip), repeatable by a silent loop, then e"""
    from pm4py.objects.petri_net.obj import Marking, PetriNet
    from pm4py.objects.petri_net.utils.petri_utils import add_arc_from_to

    net = PetriNet("handmade")
    p = [PetriNet.Place(f"p{i}") for i in range(5)]
    for place in p:
        net.places.add(place)
    for name, label, src, dst in [
        ("ta", "a", 0, 1),
        ("tb", "b", 1, 2),
        ("tc", "c", 1, 2),
        ("skip1", None, 1, 2),
        ("td", "d", 2, 3),
        ("skip2", None, 2, 3),
        ("loop", None, 3, 1),
        ("te", "e", 3, 4),
    ]:
        t = PetriNet.Transition(name, label)
        net.transitions.add(t)
        add_arc_from_to(p[src], t, net)
        add_arc_from_to(t, p[dst], net)
    return net, Marking({p[0]: 1}), Marking({p[4]: 1})


def nets(pm4py):
    """(name, log df, net, im, fm) of a few logs and models with silent transitions"""
    df = make_df(TRACES)
    noisy = make_df(NOISY_TRACES)
    return [
        ("im-on-fitting-part", df, *pm4py.discover_petri_net_inductive(df.iloc[:40])),
        ("imf-0.2", noisy, *pm4py.discover_petri_net_inductive(noisy, noise_threshold=0.2)),
        ("im", noisy, *pm4py.discover_petri_net_inductive(noisy)),
        ("handmade", noisy, *handmade_net(pm4py)),
        ("handmade-on-other-log", df, *handmade_net(pm4py)),
    ]


def test_alignment_fitness_matches_pm4py(pm4py):
    for name, df, net, im, fm in nets(pm4py):
        ours = conformance.fitness(df, net, im, fm)
        theirs = pm4py.fitness_alignments(df, net, im, fm)
        for key in ["log_fitness", "average_trace_fitness", "percentage_of_fitting_traces"]:
            assert ours[key] == pytest.approx(theirs[key], abs=1e-3), (name, key, ours, theirs)
        assert ours["percFitTraces"] == ours["percentage_of_fitting_traces"]
        assert ours["averageFitness"] == ours["average_trace_fitness"]


def test_per_trace_alignments_match_pm4py(pm4py):
    for name, df, net, im, fm in nets(pm4py):
        ours = conformance.alignments(df, net, im, fm)
        theirs = pm4py.conformance_diagnostics_alignments(df, net, im, fm)
        assert len(ours) == len(theirs) == df["case:concept:name"].nunique()
        for a, b in zip(ours, theirs):
            assert a is not None
            assert a["fitness"] == pytest.approx(b["fitness"], abs=1e-9), name
            # pm4py costs are scaled by 10000, plus 1 per silent move
            assert a["cost"] == b["cost"] // 10000, name
            assert a["bwc"] == b["bwc"] // 10000, name
            log_side = [l for l, _ in a["alignment"] if l != ">>"]
            assert log_side == [l for l, _ in b["alignment"] if l != ">>"]


def test_precision_pm4py_preset_matches_pm4py(pm4py):
    results = []
    for name, df, net, im, fm in nets(pm4py):
        ours = conformance.precision(df, net, im, fm, preset="pm4py")
        theirs = pm4py.precision_alignments(df, net, im, fm)
        results.append((name, ours, theirs))
        assert ours == pytest.approx(theirs, abs=1e-3), (name, ours, theirs)
    assert any(ours < 1 for _, ours, _ in results)


def test_precision_presets_and_details(pm4py, noisy_df):
    net, im, fm = handmade_net(pm4py)
    presets: "list[conformance.Preset]" = ["default", "prom", "pm4py"]
    values = {p: conformance.precision(noisy_df, net, im, fm, preset=p) for p in presets}
    assert all(0 < v < 1 for v in values.values()), values
    details = conformance.precision_result(noisy_df, net, im, fm, report_escaping_edges=True)
    assert details["precision"] == values["default"]
    assert details["weighted_escaping"] > 0
    assert details.get("escaping_edges")
    assert 0 < details["weighted_escaping"] < details["weighted_available"]
    stricter = conformance.precision(noisy_df, net, im, fm, trace_ends="AsActivity")
    assert stricter != values["default"]


def test_token_based_fitness_matches_pm4py(pm4py, df):
    # Alpha nets have neither silent transitions nor duplicate labels
    net, im, fm = pm4py.discover_petri_net_alpha(df)
    ours = conformance.fitness(df, net, im, fm, method="token_based")
    theirs = pm4py.fitness_token_based_replay(df, net, im, fm)
    assert ours["log_fitness"] == pytest.approx(theirs["log_fitness"], abs=1e-9)
    assert ours["missing"] > 0


def test_token_based_rejects_silent_transitions(pm4py, df):
    net, im, fm = handmade_net(pm4py)
    with pytest.raises(ConformanceError, match="silent"):
        conformance.fitness(df, net, im, fm, method="token_based")


def test_log_and_net_forms_agree(pm4py, df, tmp_path):
    net, im, fm = pm4py.discover_petri_net_inductive(df, noise_threshold=0.2)
    expected = conformance.fitness(df, net, im, fm)["log_fitness"]

    pnml = str(tmp_path / "net.pnml")
    pm4py.write_pnml(net, im, fm, pnml)
    xes = str(tmp_path / "log.xes")
    pm4py.write_xes(df, xes)
    net_dict = petri_net.from_pm4py(net, im, fm)

    logs = {
        "pandas": df,
        "polars": pl.from_pandas(df),
        "pm4py EventLog": pm4py.convert_to_event_log(df),
        "traces": [list(t) for t in df.groupby("case:concept:name")["concept:name"].agg(list)],
        "xes path": xes,
        "handle": r4pm.import_item("EventLog", xes),
        "projection handle": r4pm.import_item("EventLogActivityProjection", xes),
    }
    nets = {
        "pm4py tuple": ((net, im, fm),),
        "pm4py": (net, im, fm),
        "dict": (net_dict,),
        "handle": (petri_net.to_handle(net_dict),),
        "pnml handle": (r4pm.import_item("AcceptingPetriNet", pnml),),
        "pnml path": (pnml,),
    }
    for log_name, log in logs.items():
        for net_name, net_args in nets.items():
            got = conformance.fitness(log, *net_args)["log_fitness"]
            assert got == pytest.approx(expected, abs=1e-12), (log_name, net_name)
    # Neither the logs loaded from paths nor converted copies stay in the registry
    ids = {item["id"] for item in r4pm.list_items()}
    assert ids == {logs["handle"], logs["projection handle"], nets["handle"][0], nets["pnml handle"][0]}


def test_alignments_reuse(pm4py, noisy_df):
    net, im, fm = handmade_net(pm4py)
    aligned = conformance.alignments(noisy_df, net, im, fm)
    assert conformance.fitness(aligned) == conformance.fitness(noisy_df, net, im, fm)
    assert conformance.precision(aligned) == conformance.precision(noisy_df, net, im, fm)
    assert conformance.precision(aligned, preset="prom") == conformance.precision(noisy_df, net, im, fm, preset="prom")
    with pytest.raises(ValueError):
        conformance.precision(aligned, preset="pm4py")


def test_typed_variants(pm4py, df):
    net, im, fm = pm4py.discover_petri_net_inductive(df.iloc[:40])
    aligned = conformance.alignments(df, net, im, fm)
    assert sum(v.frequency for v in aligned.variants) == len(aligned)
    deviating = [v for v in aligned.variants if v.fitness is not None and v.fitness < 1]
    assert deviating
    for v in aligned.variants:
        assert v.moves is not None and v.error is None
        assert [m.activity for m in v.moves if m.kind in ("sync", "log")] == v.activities
        kinds = {m.kind for m in v.moves}
        assert kinds <= {"sync", "log", "model", "silent"}
        assert (v.fitness == 1) == (not any(m.is_deviation for m in v.moves))
    silent = [m for v in aligned.variants for m in v.moves or [] if m.kind == "silent"]
    assert all(m.as_pm4py() == (">>", None) for m in silent)


def test_options(pm4py, df):
    net, im, fm = pm4py.discover_petri_net_inductive(df.iloc[:40])
    base = conformance.alignments(df, net, im, fm)
    expensive = conformance.alignments(
        df, net, im, fm, options=AlignmentOptions(log_move_cost=5, model_move_cost=3), num_threads=1
    )
    assert sum(a["cost"] for a in expensive if a) > sum(a["cost"] for a in base if a)

    limited = conformance.alignments(df, net, im, fm, options=AlignmentOptions(max_states=1))
    assert any(a is None for a in limited)
    assert any(v.error == {"SearchError": "LimitReached"} for v in limited.variants)
    with pytest.raises(ConformanceError, match="LimitReached"):
        conformance.fitness(df, net, im, fm, options=AlignmentOptions(max_states=1))


def test_errors(pm4py, df):
    net, im, fm = pm4py.discover_petri_net_inductive(df)
    with pytest.raises(ValueError, match="marking"):
        conformance.fitness(df, net)
    with pytest.raises(FileNotFoundError):
        conformance.fitness("does/not/exist.xes", net, im, fm)
    with pytest.raises(TypeError):
        conformance.fitness(42, net, im, fm)
    with r4pm.import_item_from_json("OCDeclareModel", []) as model:
        with pytest.raises(TypeError, match="OCDeclareModel"):
            conformance.fitness(model, net, im, fm)
