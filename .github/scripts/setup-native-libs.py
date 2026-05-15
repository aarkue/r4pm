#!/usr/bin/env python3
import os
import subprocess
import tarfile
import zipfile
from pathlib import Path


def run(*args):
    subprocess.run(args, check=True)


root = Path("native").resolve()
for name, repo, tag, asset in [
    ("duckdb", "duckdb/duckdb", os.environ["DUCKDB_VERSION"], os.environ["DUCKDB_ASSET"]),
    ("kuzu", "kuzudb/kuzu", os.environ["KUZU_VERSION"], os.environ["KUZU_ASSET"]),
]:
    out = root / name
    out.mkdir(parents=True, exist_ok=True)
    run("gh", "release", "download", tag, "--repo", repo, "--pattern", asset, "--dir", str(out))
    archive = next(p for p in out.iterdir() if p.suffix == ".zip" or p.name.endswith(".tar.gz"))
    if archive.suffix == ".zip":
        zipfile.ZipFile(archive).extractall(out)
    else:
        tarfile.open(archive, "r:gz").extractall(out)

with open(os.environ["GITHUB_ENV"], "a", encoding="utf-8") as env:
    env.write(f"DUCKDB_LIB_DIR={root / 'duckdb'}\n")
    env.write(f"DUCKDB_INCLUDE_DIR={root / 'duckdb'}\n")
    env.write("KUZU_SHARED=1\n")
    env.write(f"KUZU_LIBRARY_DIR={root / 'kuzu'}\n")
    env.write(f"KUZU_INCLUDE_DIR={root / 'kuzu'}\n")
    env.write(f"LD_LIBRARY_PATH={root / 'kuzu'}:{root / 'duckdb'}:{os.environ.get('LD_LIBRARY_PATH', '')}\n")
    env.write(f"DYLD_LIBRARY_PATH={root / 'kuzu'}:{root / 'duckdb'}:{os.environ.get('DYLD_LIBRARY_PATH', '')}\n")

with open(os.environ["GITHUB_PATH"], "a", encoding="utf-8") as path:
    path.write(f"{root / 'duckdb'}\n{root / 'kuzu'}\n")
