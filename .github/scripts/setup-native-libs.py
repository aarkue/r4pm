#!/usr/bin/env python3
import os
import tarfile
import urllib.request
import zipfile
from pathlib import Path


root = Path("native").resolve()
for name, repo, tag, asset in [
    ("duckdb", "duckdb/duckdb", os.environ["DUCKDB_VERSION"], os.environ["DUCKDB_ASSET"]),
    ("kuzu", "kuzudb/kuzu", os.environ["KUZU_VERSION"], os.environ["KUZU_ASSET"]),
]:
    out = root / name
    out.mkdir(parents=True, exist_ok=True)
    archive = out / asset
    url = f"https://github.com/{repo}/releases/download/{tag}/{asset}"
    urllib.request.urlretrieve(url, archive)
    if archive.suffix == ".zip":
        zipfile.ZipFile(archive).extractall(out)
    else:
        tarfile.open(archive, "r:gz").extractall(out)

if "GITHUB_ENV" in os.environ:
    with open(os.environ["GITHUB_ENV"], "a", encoding="utf-8") as env:
        env.write(f"DUCKDB_LIB_DIR={root / 'duckdb'}\n")
        env.write(f"DUCKDB_INCLUDE_DIR={root / 'duckdb'}\n")
        env.write("KUZU_SHARED=1\n")
        env.write(f"KUZU_LIBRARY_DIR={root / 'kuzu'}\n")
        env.write(f"KUZU_INCLUDE_DIR={root / 'kuzu'}\n")
        env.write(f"LD_LIBRARY_PATH={root / 'kuzu'}:{root / 'duckdb'}:{os.environ.get('LD_LIBRARY_PATH', '')}\n")
        env.write(f"DYLD_LIBRARY_PATH={root / 'kuzu'}:{root / 'duckdb'}:{os.environ.get('DYLD_LIBRARY_PATH', '')}\n")

if "GITHUB_PATH" in os.environ:
    with open(os.environ["GITHUB_PATH"], "a", encoding="utf-8") as path:
        path.write(f"{root / 'duckdb'}\n{root / 'kuzu'}\n")
