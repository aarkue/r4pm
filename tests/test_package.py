import ast
import importlib
import inspect
import pkgutil
from pathlib import Path

import r4pm
from r4pm import bindings
from r4pm import r4pm as native

PACKAGE = Path(r4pm.__file__).parent


def test_all_names_exist():
    for module in [r4pm, bindings, r4pm.handles, r4pm.petri_net, r4pm.conformance, r4pm.interop]:
        for name in module.__all__:
            assert hasattr(module, name), (module.__name__, name)
        assert len(module.__all__) == len(set(module.__all__)), module.__name__


def test_generated_modules_import_and_export_their_functions():
    for info in pkgutil.walk_packages(bindings.__path__, "r4pm.bindings."):
        module = importlib.import_module(info.name)
        for name in getattr(module, "__all__", []):
            assert hasattr(module, name), (info.name, name)


def test_every_binding_is_reachable():
    for meta in r4pm.list_bindings():
        path = "r4pm.bindings." + meta["module"].removeprefix("process_mining::").replace("::", ".")
        module = importlib.import_module(path)
        if hasattr(module, "__path__"):
            module = importlib.import_module(path + "._impl")
        func = getattr(module, meta["name"].replace("-", "_"))
        assert meta["id"] in inspect.getsource(func)


def test_generated_typed_dicts_are_not_total_false():
    for path in [*(PACKAGE / "bindings").rglob("*.py"), *(PACKAGE / "bindings").rglob("*.pyi")]:
        assert "total=False" not in path.read_text(), path


def test_types_are_defined_once():
    names = []
    for path in (PACKAGE / "bindings").rglob("*.pyi"):
        tree = ast.parse(path.read_text())
        names += [(n.name, path.name) for n in tree.body if isinstance(n, ast.ClassDef)]
    assert all(file == "_types.pyi" for _, file in names)
    assert len({n for n, _ in names}) == len(names)


def test_native_stub_matches_module():
    tree = ast.parse((PACKAGE / "r4pm.pyi").read_text())
    stubbed = {n.name for n in tree.body if isinstance(n, (ast.FunctionDef, ast.ClassDef))}
    exported = {n for n in dir(native) if not n.startswith("_")}
    assert stubbed == exported
