#!/usr/bin/env python3
import argparse
import os
import tarfile
import urllib.request
import zipfile
from pathlib import Path


parser = argparse.ArgumentParser()
parser.add_argument("--duckdb-version", default=os.environ.get("DUCKDB_VERSION"))
parser.add_argument("--duckdb-asset", default=os.environ.get("DUCKDB_ASSET"))
parser.add_argument("--kuzu-version", default=os.environ.get("KUZU_VERSION"))
parser.add_argument("--kuzu-asset", default=os.environ.get("KUZU_ASSET"))
parser.add_argument("--root", default="native")
args = parser.parse_args()

root = Path(args.root).resolve()
for name, repo, tag, asset in [
    ("duckdb", "duckdb/duckdb", args.duckdb_version, args.duckdb_asset),
    ("kuzu", "kuzudb/kuzu", args.kuzu_version, args.kuzu_asset),
]:
    if not tag or not asset:
        raise SystemExit(f"Missing version or asset for {name}")
    out = root / name
    out.mkdir(parents=True, exist_ok=True)
    archive = out / asset
    url = f"https://github.com/{repo}/releases/download/{tag}/{asset}"
    urllib.request.urlretrieve(url, archive)
    if archive.suffix == ".zip":
        zipfile.ZipFile(archive).extractall(out)
    else:
        tarfile.open(archive, "r:gz").extractall(out)

github_env = os.environ.get("GITHUB_ENV")
if github_env and Path(github_env).exists():
    with open(github_env, "a", encoding="utf-8") as env:
        env.write(f"DUCKDB_LIB_DIR={root / 'duckdb'}\n")
        env.write(f"DUCKDB_INCLUDE_DIR={root / 'duckdb'}\n")
        env.write("KUZU_SHARED=1\n")
        env.write(f"KUZU_LIBRARY_DIR={root / 'kuzu'}\n")
        env.write(f"KUZU_INCLUDE_DIR={root / 'kuzu'}\n")
        env.write(f"LD_LIBRARY_PATH={root / 'kuzu'}:{root / 'duckdb'}:{os.environ.get('LD_LIBRARY_PATH', '')}\n")
        env.write(f"DYLD_LIBRARY_PATH={root / 'kuzu'}:{root / 'duckdb'}:{os.environ.get('DYLD_LIBRARY_PATH', '')}\n")

github_path = os.environ.get("GITHUB_PATH")
if github_path and Path(github_path).exists():
    with open(github_path, "a", encoding="utf-8") as path:
        path.write(f"{root / 'duckdb'}\n{root / 'kuzu'}\n")
