#!/usr/bin/env python3
"""Generate korangar/archive/System/itemprices.turoran.v1.json.

Reads the Hercules item databases the TEST map server loads (renewal:
db/re/item_db.conf, then db/item_db2.conf overrides) and writes the NPC sell
price per item id, using Hercules' own defaults (itemdb.c): a missing Sell is
Buy / 2, a missing Buy is Sell * 2, neither is 0.

Usage: generate-item-prices.py <hercules checkout> <output json>
"""

import json
import re
import sys
from pathlib import Path

SCRIPT = re.compile(r'<".*?">', re.S)
BLOCK_COMMENT = re.compile(r"/\*.*?\*/", re.S)
LINE_COMMENT = re.compile(r"//[^\n]*")
FIELD = re.compile(r"^\s*(Id|Buy|Sell)\s*:\s*(-?\d+)", re.M)


def entries(text):
    """Top-level `{ ... }` blocks inside `item_db: ( ... )`."""
    text = SCRIPT.sub('""', text)
    text = BLOCK_COMMENT.sub("", text)
    text = LINE_COMMENT.sub("", text)
    depth = 0
    start = None
    for position, character in enumerate(text):
        if character == "{":
            if depth == 0:
                start = position
            depth += 1
        elif character == "}":
            depth -= 1
            if depth == 0 and start is not None:
                yield text[start + 1 : position]
                start = None


def top_level(block):
    """Fields of the entry itself, not of nested groups such as Job."""
    flat = []
    depth = 0
    for character in block:
        if character == "{":
            depth += 1
        elif character == "}":
            depth -= 1
        elif depth == 0:
            flat.append(character)
    return "".join(flat)


def read(path, items):
    for block in entries(path.read_text(encoding="utf-8", errors="replace")):
        fields = {name: int(value) for name, value in FIELD.findall(top_level(block))}
        if "Id" not in fields:
            continue
        entry = items.setdefault(fields["Id"], {})
        for name in ("Buy", "Sell"):
            if name in fields:
                entry[name] = fields[name]


def main():
    root = Path(sys.argv[1])
    output = Path(sys.argv[2])
    items = {}
    for relative in ("db/re/item_db.conf", "db/item_db2.conf"):
        read(root / relative, items)
    sell = {}
    for item_id, entry in sorted(items.items()):
        buy = entry.get("Buy", -1)
        price = entry.get("Sell", -1)
        if price == -1:
            price = 0 if buy == -1 else buy // 2
        if price > 0:
            sell[str(item_id)] = price
    data = {"version": 1, "source": "Hercules renewal item_db.conf + item_db2.conf", "sell": sell}
    output.write_text(json.dumps(data, separators=(",", ":")) + "\n", encoding="utf-8")
    print(f"{len(sell)} sell prices from {len(items)} items -> {output}")


if __name__ == "__main__":
    main()
