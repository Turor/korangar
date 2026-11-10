#!/usr/bin/env python3
"""Regenerate Item Finder JSON from a Hercules tree and a Turoran itemInfo.lua.

See README.md in this directory.
"""
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))


def main(argv):
    if len(argv) < 5:
        print(
            "usage: export.py <hercules-root> <itemInfo.lua> <catalog-out.json> <sources-out.json> [sources-fallback.json]",
            file=sys.stderr,
        )
        return 2
    root, info, catalog_out, sources_out = argv[1:5]
    fallback = argv[5] if len(argv) > 5 else ""
    db = os.path.join(root, "db/re/item_db.conf")
    db2 = os.path.join(root, "db/item_db2.conf")
    if not os.path.exists(db2):
        db2 = "-"
    maptable = os.path.join(root, "db/map_index.txt")
    # Official map names live in the client mapnametable. map_index is not that
    # file; pass it only when it looks like a name table, otherwise "-".
    if not _looks_like_mapnametable(maptable):
        maptable = "-"
    os.makedirs(os.path.dirname(os.path.abspath(catalog_out)) or ".", exist_ok=True)
    os.makedirs(os.path.dirname(os.path.abspath(sources_out)) or ".", exist_ok=True)
    subprocess.check_call([sys.executable, os.path.join(HERE, "genitemfinder.py"), db, db2, info, catalog_out])
    cmd = [sys.executable, os.path.join(HERE, "genitemsources.py"), root, maptable, sources_out]
    if fallback:
        cmd.append(fallback)
    subprocess.check_call(cmd)
    return 0


def _looks_like_mapnametable(path):
    if not os.path.exists(path):
        return False
    with open(path, encoding="latin-1") as handle:
        for _ in range(30):
            line = handle.readline()
            if not line:
                break
            if ".rsw#" in line.lower():
                return True
    return False


if __name__ == "__main__":
    sys.exit(main(sys.argv))
