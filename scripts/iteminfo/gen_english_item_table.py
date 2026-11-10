#!/usr/bin/env python3
"""Build the compact English item table Korangar ships in its archive overlay.

The Turoran renewal overlay (``itemInfo.turoran.renewal11.lua``) is the same
file roBrowser loads last from ``translation.grf`` / remoteclient2. It is not
valid UTF-8: display text is ASCII or CP949, and resource names are CP949
sprite keys. This script keeps only what the client shows and stores:

    id, slot count, identified name, unidentified name,
    identified description, unidentified description

Sprite resource names stay on the stock ``iteminfo.lub`` row. ``{ "" }`` and
``{ "..." }`` description stubs are written as an empty field so the loader
does not keep a Korean blurb under the English name.

Output format ``KEN1`` (UTF-8, one item per line, six tab-separated fields):

    KEN1
    <id>\\t<slots>\\t<identified name>\\t<unidentified name>\\t<identified description>\\t<unidentified description>

Newlines and tabs inside a field are ``\\n`` and ``\\t``. A backslash is ``\\\\``.
Description lines are joined with ``\\n``. ``<NAVI>`` / ``<ITEM>`` markup is
reduced to its label. ``^RRGGBB`` color codes are kept.

Usage:
    python3 scripts/iteminfo/gen_english_item_table.py \\
        /path/to/itemInfo.turoran.renewal11.lua \\
        korangar/archive/System/iteminfo.english.v1.txt.gz

The default input on this host is
``/root/Work/korangar-itemfinder-inputs/itemInfo.turoran.renewal11.lua``.
"""

from __future__ import annotations

import argparse
import gzip
import re
import sys
from pathlib import Path

HEADER = "KEN1"
ITEM_HEADER = re.compile(br"^\t\[(\d+)\] = \{", re.M)
NAVI = re.compile(
    r"<(?:itemlink|iteml|item|navi)>(.*?)<info>.*?</info></(?:itemlink|iteml|item|navi)>",
    re.IGNORECASE | re.DOTALL,
)


def parse_lua_string(data: bytes, index: int) -> tuple[bytes, int]:
    if data[index : index + 1] != b'"':
        raise ValueError(f"expected string at {index}")
    index += 1
    out = bytearray()
    while index < len(data):
        byte = data[index]
        if byte == 0x5C:  # backslash
            nxt = data[index + 1]
            if nxt in (0x22, 0x27, 0x5C):
                out.append(nxt)
            else:
                out.extend((byte, nxt))
            index += 2
            continue
        if byte == 0x22:
            return bytes(out), index + 1
        out.append(byte)
        index += 1
    raise ValueError("unterminated Lua string")


def decode_game(raw: bytes) -> str:
    """UTF-8, then CP949. A few official names are Windows-1252 (``Günter``)."""
    try:
        return raw.decode("utf-8")
    except UnicodeDecodeError:
        pass
    try:
        return raw.decode("cp949")
    except UnicodeDecodeError:
        return raw.decode("latin-1")


def skip_ws(data: bytes, index: int) -> int:
    while index < len(data) and data[index] in b" \t\r\n":
        index += 1
    return index


def field_at(chunk: bytes, key: bytes) -> int | None:
    """Index of `key` at the start of a field line.

    `identifiedDisplayName` is a suffix of `unidentifiedDisplayName`, so a bare
    substring search would read the unidentified field.
    """
    marker = b"\n\t\t" + key + b" = "
    index = chunk.find(marker)
    if index < 0:
        return None
    return index + len(marker)


def field_strings(chunk: bytes, key: bytes) -> list[bytes] | None:
    found = field_at(chunk, key)
    if found is None:
        return None
    index = skip_ws(chunk, found)
    if chunk[index : index + 1] == b'"':
        text, _ = parse_lua_string(chunk, index)
        return [text]
    if chunk[index : index + 1] != b"{":
        return None
    index += 1
    strings: list[bytes] = []
    while index < len(chunk):
        index = skip_ws(chunk, index)
        if index >= len(chunk) or chunk[index : index + 1] == b"}":
            break
        if chunk[index : index + 1] == b",":
            index += 1
            continue
        if chunk[index : index + 1] != b'"':
            break
        text, index = parse_lua_string(chunk, index)
        strings.append(text)
    return strings


def field_int(chunk: bytes, key: bytes) -> int | None:
    found = field_at(chunk, key)
    if found is None:
        return None
    index = found
    end = index
    while end < len(chunk) and chr(chunk[end]).isdigit():
        end += 1
    if end == index:
        return None
    return int(chunk[index:end])


def strip_markup(text: str) -> str:
    previous = None
    while previous != text:
        previous = text
        text = NAVI.sub(r"\1", text)
    return text


def is_stub(lines: list[str]) -> bool:
    return not lines or (len(lines) == 1 and lines[0] in ("", "..."))


def escape_field(text: str) -> str:
    text = text.replace("\u0085", "\n").replace("\u2028", "\n").replace("\u2029", "\n")
    return text.replace("\\", "\\\\").replace("\t", "\\t").replace("\r", "").replace("\n", "\\n")


def description_field(lines: list[str] | None) -> str:
    if lines is None or is_stub(lines):
        return ""
    cleaned = [strip_markup(line).replace("\r", "") for line in lines]
    if is_stub(cleaned):
        return ""
    return escape_field("\n".join(cleaned))


def name_field(raw: bytes | None) -> str:
    if raw is None:
        return ""
    return escape_field(strip_markup(decode_game(raw)).strip())


def iter_items(data: bytes):
    matches = list(ITEM_HEADER.finditer(data))
    for index, match in enumerate(matches):
        start = match.end()
        end = matches[index + 1].start() if index + 1 < len(matches) else len(data)
        yield int(match.group(1)), data[start:end]


def build_table(data: bytes) -> str:
    lines = [HEADER]
    for item_id, chunk in iter_items(data):
        identified = field_strings(chunk, b"identifiedDisplayName")
        unidentified = field_strings(chunk, b"unidentifiedDisplayName")
        identified_desc = field_strings(chunk, b"identifiedDescriptionName")
        unidentified_desc = field_strings(chunk, b"unidentifiedDescriptionName")
        slots = field_int(chunk, b"slotCount") or 0
        if slots > 255:
            slots = 255
        identified_name = name_field(identified[0] if identified else None)
        unidentified_name = name_field(unidentified[0] if unidentified else None)
        if not identified_name and not unidentified_name:
            continue
        lines.append(
            "\t".join(
                [
                    str(item_id),
                    str(slots),
                    identified_name,
                    unidentified_name,
                    description_field([decode_game(part) for part in identified_desc] if identified_desc else None),
                    description_field([decode_game(part) for part in unidentified_desc] if unidentified_desc else None),
                ]
            )
        )
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path, help="itemInfo.turoran.renewal11.lua")
    parser.add_argument("output", type=Path, help="iteminfo.english.v1.txt.gz")
    args = parser.parse_args()
    data = args.source.read_bytes()
    text = build_table(data)
    payload = text.encode("utf-8")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with gzip.open(args.output, "wb", compresslevel=9) as handle:
        handle.write(payload)
    print(
        f"wrote {args.output} ({args.output.stat().st_size} bytes gzip, {len(payload)} bytes text, {text.count(chr(10)) - 1} items)",
        file=sys.stderr,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
