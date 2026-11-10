#!/usr/bin/env python3
"""Generate korangar skill/effect tables from the roBrowser DB sources.

Reads EffectTable.js, SkillEffect.js, SkillUnit.js and their const tables.
Emits reviewable Rust static data for STR and sprite layers. Other effect
kinds are recorded so playback can skip them.
"""

from __future__ import annotations

import re
from pathlib import Path

RO = Path("/root/Work/refs/roBrowserLegacy/src/DB")
OUT = Path("/root/Work/korangar-gap-skill-effects/korangar/src/world/effects/generated.rs")


def strip_comments(text: str) -> str:
    out: list[str] = []
    i = 0
    n = len(text)
    quote = ""
    while i < n:
        c = text[i]
        if quote:
            out.append(c)
            if c == "\\":
                if i + 1 < n:
                    out.append(text[i + 1])
                    i += 2
                    continue
            elif c == quote:
                quote = ""
            i += 1
            continue
        if c in "'\"`":
            quote = c
            out.append(c)
            i += 1
            continue
        if c == "/" and i + 1 < n and text[i + 1] == "/":
            i = text.find("\n", i)
            if i < 0:
                break
            continue
        if c == "/" and i + 1 < n and text[i + 1] == "*":
            end = text.find("*/", i + 2)
            i = n if end < 0 else end + 2
            out.append(" ")
            continue
        out.append(c)
        i += 1
    return "".join(out)


def parse_const_object(text: str) -> dict[str, int]:
    body = strip_comments(text)
    start = body.find("export default")
    if start < 0:
        start = body.find("{")
    else:
        start = body.find("{", start)
    values: dict[str, int] = {}
    for match in re.finditer(
        r"([A-Za-z_][A-Za-z0-9_]*)\s*:\s*(-?0x[0-9a-fA-F]+|-?\d+)",
        body[start:],
    ):
        raw = match.group(2)
        values[match.group(1)] = int(raw, 16) if raw.lower().startswith(("0x", "-0x")) else int(raw)
    return values


def slice_value(text: str, index: int) -> tuple[str, int]:
    """Return the JS value starting at index and the index just after it."""
    n = len(text)
    while index < n and text[index].isspace():
        index += 1
    if index >= n:
        return "", index
    start = index
    opener = text[index]
    if opener in "{[(":
        closer = {"{": "}", "[": "]", "(": ")"}[opener]
        depth = 0
        quote = ""
        while index < n:
            c = text[index]
            if quote:
                if c == "\\":
                    index += 2
                    continue
                if c == quote:
                    quote = ""
                index += 1
                continue
            if c in "'\"`":
                quote = c
                index += 1
                continue
            if c == opener:
                depth += 1
            elif c == closer:
                depth -= 1
                if depth == 0:
                    return text[start : index + 1], index + 1
            index += 1
        return text[start:], n
    # Arrow or bare token until comma / semicolon at depth 0.
    quote = ""
    depth_brace = 0
    depth_bracket = 0
    depth_paren = 0
    while index < n:
        c = text[index]
        if quote:
            if c == "\\":
                index += 2
                continue
            if c == quote:
                quote = ""
            index += 1
            continue
        if c in "'\"`":
            quote = c
            index += 1
            continue
        if c == "{":
            depth_brace += 1
        elif c == "}":
            if depth_brace == 0 and depth_bracket == 0 and depth_paren == 0:
                break
            depth_brace -= 1
        elif c == "[":
            depth_bracket += 1
        elif c == "]":
            depth_bracket -= 1
        elif c == "(":
            depth_paren += 1
        elif c == ")":
            depth_paren -= 1
        elif c in ",;" and depth_brace == 0 and depth_bracket == 0 and depth_paren == 0:
            break
        index += 1
    return text[start:index].strip(), index


def iter_object_entries(body: str):
    index = 0
    n = len(body)
    while index < n and body[index].isspace():
        index += 1
    if index < n and body[index] == "{":
        index += 1
    while index < n:
        while index < n and body[index] in " \t\r\n,":
            index += 1
        if index >= n or body[index] == "}":
            break
        key, index = slice_value(body, index) if body[index] in "'\"`" else (None, index)
        if key is None:
            match = re.match(r"[A-Za-z_][A-Za-z0-9_]*|-?\d+", body[index:])
            if not match:
                index += 1
                continue
            key = match.group(0)
            index += len(key)
        else:
            key = key[1:-1]
        while index < n and body[index].isspace():
            index += 1
        if index >= n or body[index] != ":":
            continue
        index += 1
        value, index = slice_value(body, index)
        yield key, value


def parse_effect_layers(array_src: str) -> list[tuple[str, str]]:
    """Return (kind, file) for each object in an effect array."""
    if not array_src.startswith("["):
        return []
    inner = array_src[1:-1]
    layers: list[tuple[str, str]] = []
    index = 0
    n = len(inner)
    while index < n:
        while index < n and inner[index] in " \t\r\n,":
            index += 1
        if index >= n:
            break
        if inner[index] != "{":
            index += 1
            continue
        obj, index = slice_value(inner, index)
        kind = "Other"
        file_name = ""
        rand_lo = None
        for key, value in iter_object_entries(obj):
            if key == "type":
                kind = value.strip().strip("'\"")
            elif key == "file" and not file_name:
                file_name = value.strip().strip("'\"")
            elif key in ("absoluteSpriteName", "spriteName") and not file_name:
                file_name = value.strip().strip("'\"")
            elif key == "rand":
                numbers = re.findall(r"-?\d+", value)
                if numbers:
                    rand_lo = numbers[0]
        if "%d" in file_name:
            file_name = file_name.replace("%d", rand_lo or "1")
        normalized = kind.upper()
        if normalized == "STR":
            layers.append(("Str", str_path(file_name)))
        elif normalized == "SPR":
            layers.append(("Sprite", file_name.replace("/", "\\")))
        else:
            layers.append(("Other", ""))
    return layers


def str_path(file_name: str) -> str:
    path = file_name.replace("/", "\\")
    if not path.lower().endswith(".str"):
        path = f"{path}.str"
    return path


def parse_effect_table(text: str) -> tuple[dict[int, list[tuple[str, str]]], dict[str, list[tuple[str, str]]]]:
    body = strip_comments(text)
    start = body.find("export default")
    start = body.find("{", start)
    numeric: dict[int, list[tuple[str, str]]] = {}
    named: dict[str, list[tuple[str, str]]] = {}
    # The export default object is the whole table. Stop at the matching close
    # by slicing the object first.
    obj, _ = slice_value(body, start)
    for key, value in iter_object_entries(obj):
        layers = parse_effect_layers(value)
        if not layers:
            layers = [("Other", "")]
        if re.fullmatch(r"-?\d+", key):
            numeric[int(key)] = layers
        else:
            named[key] = layers
    return numeric, named


def parse_ref_list(value: str) -> list[tuple[str, object]]:
    value = value.strip()
    if "=>" in value:
        arrays = re.findall(r"\[[^\]]*\]", value)
        if not arrays:
            return []
        value = arrays[-1]
    if value.startswith("["):
        inner = value[1:-1]
        refs: list[tuple[str, object]] = []
        index = 0
        while index < len(inner):
            while index < len(inner) and inner[index] in " \t\r\n,":
                index += 1
            if index >= len(inner):
                break
            token, index = slice_value(inner, index)
            refs.extend(parse_ref_list(token))
        return refs
    if value.startswith(("'", '"')):
        return [("name", value[1:-1])]
    if re.fullmatch(r"-?\d+", value):
        number = int(value)
        if number < 0:
            return []
        return [("id", number)]
    return []


SKILL_FIELDS = {
    "effectId": "effect",
    "effectIdOnCaster": "effect_on_caster",
    "groundEffectId": "ground",
    "hitEffectId": "hit",
    "hitEffectIdOnCaster": "hit_on_caster",
    "beforeHitEffectId": "before_hit",
    "beforeHitEffectIdOnCaster": "before_hit_on_caster",
    "beginCastEffectId": "begin_cast",
    "successEffectId": "success",
    "successEffectIdOnCaster": "success_on_caster",
}


def empty_skill() -> dict:
    data = {name: [] for name in SKILL_FIELDS.values()}
    data["hide_cast_bar"] = False
    data["hide_cast_aura"] = False
    return data


def parse_skill_object(src: str) -> dict:
    data = empty_skill()
    for key, value in iter_object_entries(src):
        if key == "hideCastBar":
            data["hide_cast_bar"] = "true" in value
        elif key == "hideCastAura":
            data["hide_cast_aura"] = "true" in value
        elif key in SKILL_FIELDS:
            data[SKILL_FIELDS[key]] = parse_ref_list(value)
    return data


def parse_skill_effects(text: str, skill_ids: dict[str, int]) -> dict[int, dict]:
    body = strip_comments(text)
    skills: dict[int, dict] = {}
    aliases: list[tuple[int, str]] = []
    for match in re.finditer(r"SkillEffect\[SK\.([A-Za-z0-9_]+)\]\s*=\s*", body):
        name = match.group(1)
        if name not in skill_ids:
            continue
        skill_id = skill_ids[name]
        value, _ = slice_value(body, match.end())
        alias = re.fullmatch(r"SkillEffect\[SK\.([A-Za-z0-9_]+)\]", value.strip())
        if alias:
            aliases.append((skill_id, alias.group(1)))
            continue
        if value.startswith("{"):
            skills[skill_id] = parse_skill_object(value)
    pending = aliases
    for _ in range(8):
        next_pending = []
        for skill_id, source_name in pending:
            source_id = skill_ids.get(source_name)
            if source_id is None or source_id not in skills:
                next_pending.append((skill_id, source_name))
                continue
            skills[skill_id] = {
                key: (list(val) if isinstance(val, list) else val) for key, val in skills[source_id].items()
            }
        pending = next_pending
        if not pending:
            break
    return skills


def parse_skill_units(text: str, unit_ids: dict[str, int], effect_ids: dict[str, int]) -> dict[int, tuple[str, object]]:
    body = strip_comments(text)
    units: dict[int, tuple[str, object]] = {}
    for match in re.finditer(r"SkillUnit\[SU\.([A-Za-z0-9_]+)\]\s*=\s*([^;]+);", body):
        name = match.group(1)
        if name not in unit_ids:
            continue
        raw = match.group(2).strip()
        const = re.fullmatch(r"EC\.([A-Za-z0-9_]+)", raw)
        if const:
            effect = effect_ids.get(const.group(1))
            if effect is None or effect < 0:
                continue
            units[unit_ids[name]] = ("id", effect)
            continue
        if raw.startswith(("'", '"')):
            units[unit_ids[name]] = ("name", raw[1:-1])
            continue
        if re.fullmatch(r"-?\d+", raw):
            number = int(raw)
            if number >= 0:
                units[unit_ids[name]] = ("id", number)
    return units


def rust_string(value: str) -> str:
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'


def compact_layers(layers: list[tuple[str, str]]) -> list[tuple[str, str]]:
    playable = [layer for layer in layers if layer[0] in ("Str", "Sprite")]
    if playable:
        return playable
    return [("Other", "")]


def emit(numeric, named, skills, units) -> str:
    layers: list[tuple[str, str]] = []
    numeric_rows = []
    for effect_id in sorted(numeric):
        span = compact_layers(numeric[effect_id])
        start = len(layers)
        layers.extend(span)
        if start > 65535 or len(span) > 65535:
            raise SystemExit(f"layer span does not fit in u16 for effect {effect_id}")
        numeric_rows.append((effect_id, start, len(span)))

    named_rows = []
    for name in sorted(named):
        span = compact_layers(named[name])
        start = len(layers)
        layers.extend(span)
        if start > 65535 or len(span) > 65535:
            raise SystemExit(f"layer span does not fit in u16 for effect {name}")
        named_rows.append((name, start, len(span)))

    refs: list[tuple[str, object]] = []

    def add_refs(items: list[tuple[str, object]]) -> tuple[int, int]:
        if not items:
            return (0, 0)
        start = len(refs)
        refs.extend(items)
        return (start, len(items))

    skill_rows = []
    for skill_id in sorted(skills):
        data = skills[skill_id]
        skill_rows.append(
            (
                skill_id,
                data["hide_cast_bar"],
                data["hide_cast_aura"],
                add_refs(data["effect"]),
                add_refs(data["effect_on_caster"]),
                add_refs(data["ground"]),
                add_refs(data["hit"]),
                add_refs(data["hit_on_caster"]),
                add_refs(data["before_hit"]),
                add_refs(data["before_hit_on_caster"]),
                add_refs(data["begin_cast"]),
                add_refs(data["success"]),
                add_refs(data["success_on_caster"]),
            )
        )

    lines: list[str] = []
    lines.append("// Generated by scripts/gen_skill_effects.py from roBrowser")
    lines.append("// DB/Effects/EffectTable.js, DB/Skills/SkillEffect.js and DB/Skills/SkillUnit.js.")
    lines.append("// Includes the Turoran rows in those files. Do not edit by hand.")
    lines.append("")
    lines.append("#[derive(Clone, Copy, Debug, PartialEq, Eq)]")
    lines.append("pub enum LayerKind {")
    lines.append("    Str,")
    lines.append("    Sprite,")
    lines.append("    Other,")
    lines.append("}")
    lines.append("")
    lines.append("#[derive(Clone, Copy, Debug, PartialEq, Eq)]")
    lines.append("pub struct Layer {")
    lines.append("    pub kind: LayerKind,")
    lines.append("    pub file: &'static str,")
    lines.append("}")
    lines.append("")
    lines.append("#[derive(Clone, Copy, Debug, PartialEq, Eq)]")
    lines.append("pub enum EffectRef {")
    lines.append("    Id(u32),")
    lines.append("    Name(&'static str),")
    lines.append("}")
    lines.append("")
    lines.append("#[derive(Clone, Copy, Debug, PartialEq, Eq)]")
    lines.append("pub struct SkillVisual {")
    lines.append("    pub skill_id: u16,")
    lines.append("    pub hide_cast_bar: bool,")
    lines.append("    /// Stored from the roBrowser table. No cast-aura renderer reads it yet.")
    lines.append("    #[allow(dead_code)]")
    lines.append("    pub hide_cast_aura: bool,")
    lines.append("    pub effect: (u16, u16),")
    lines.append("    pub effect_on_caster: (u16, u16),")
    lines.append("    pub ground: (u16, u16),")
    lines.append("    pub hit: (u16, u16),")
    lines.append("    pub hit_on_caster: (u16, u16),")
    lines.append("    pub before_hit: (u16, u16),")
    lines.append("    pub before_hit_on_caster: (u16, u16),")
    lines.append("    pub begin_cast: (u16, u16),")
    lines.append("    pub success: (u16, u16),")
    lines.append("    pub success_on_caster: (u16, u16),")
    lines.append("}")
    lines.append("")
    lines.append("pub static LAYERS: &[Layer] = &[")
    for kind, file_name in layers:
        lines.append(f"    Layer {{ kind: LayerKind::{kind}, file: {rust_string(file_name)} }},")
    lines.append("];")
    lines.append("")
    lines.append("/// `(effect id, layer start, layer count)` into [`LAYERS`].")
    lines.append("pub static NUMERIC_EFFECTS: &[(u32, u16, u16)] = &[")
    for effect_id, start, count in numeric_rows:
        lines.append(f"    ({effect_id}, {start}, {count}),")
    lines.append("];")
    lines.append("")
    lines.append("/// `(effect name, layer start, layer count)` into [`LAYERS`].")
    lines.append("pub static NAMED_EFFECTS: &[(&str, u16, u16)] = &[")
    for name, start, count in named_rows:
        lines.append(f"    ({rust_string(name)}, {start}, {count}),")
    lines.append("];")
    lines.append("")
    lines.append("pub static EFFECT_REFS: &[EffectRef] = &[")
    for kind, value in refs:
        if kind == "id":
            lines.append(f"    EffectRef::Id({value}),")
        else:
            lines.append(f"    EffectRef::Name({rust_string(str(value))}),")
    lines.append("];")
    lines.append("")
    lines.append("pub static SKILL_VISUALS: &[SkillVisual] = &[")
    for row in skill_rows:
        (
            skill_id,
            hide_bar,
            hide_aura,
            effect,
            effect_on_caster,
            ground,
            hit,
            hit_on_caster,
            before_hit,
            before_hit_on_caster,
            begin_cast,
            success,
            success_on_caster,
        ) = row
        lines.append("    SkillVisual {")
        lines.append(f"        skill_id: {skill_id},")
        lines.append(f"        hide_cast_bar: {'true' if hide_bar else 'false'},")
        lines.append(f"        hide_cast_aura: {'true' if hide_aura else 'false'},")
        lines.append(f"        effect: {effect},")
        lines.append(f"        effect_on_caster: {effect_on_caster},")
        lines.append(f"        ground: {ground},")
        lines.append(f"        hit: {hit},")
        lines.append(f"        hit_on_caster: {hit_on_caster},")
        lines.append(f"        before_hit: {before_hit},")
        lines.append(f"        before_hit_on_caster: {before_hit_on_caster},")
        lines.append(f"        begin_cast: {begin_cast},")
        lines.append(f"        success: {success},")
        lines.append(f"        success_on_caster: {success_on_caster},")
        lines.append("    },")
    lines.append("];")
    lines.append("")
    lines.append("/// `(skill unit id, effect ref)`.")
    lines.append("pub static SKILL_UNITS: &[(u32, EffectRef)] = &[")
    for unit_id in sorted(units):
        kind, value = units[unit_id]
        if kind == "id":
            rendered = f"EffectRef::Id({value})"
        else:
            rendered = f"EffectRef::Name({rust_string(str(value))})"
        lines.append(f"    ({unit_id}, {rendered}),")
    lines.append("];")
    lines.append("")
    return "\n".join(lines)


def main() -> None:
    effect_ids = parse_const_object((RO / "Effects/EffectConst.js").read_text(encoding="utf-8"))
    skill_ids = parse_const_object((RO / "Skills/SkillConst.js").read_text(encoding="utf-8"))
    unit_ids = parse_const_object((RO / "Skills/SkillUnitConst.js").read_text(encoding="utf-8"))
    numeric, named = parse_effect_table((RO / "Effects/EffectTable.js").read_text(encoding="utf-8"))
    skills = parse_skill_effects((RO / "Skills/SkillEffect.js").read_text(encoding="utf-8"), skill_ids)
    units = parse_skill_units((RO / "Skills/SkillUnit.js").read_text(encoding="utf-8"), unit_ids, effect_ids)
    text = emit(numeric, named, skills, units)
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(text, encoding="utf-8")
    print(f"effects numeric={len(numeric)} named={len(named)} skills={len(skills)} units={len(units)} bytes={len(text)}")
    # Spot-check the same rows the Rust tests lock in.
    bash = skills[skill_ids["SM_BASH"]]
    assert bash["begin_cast"] == [("id", 16)] and bash["hit"] == [("id", 1)]
    heal = skills[skill_ids["AL_HEAL"]]
    assert heal["effect"] == [("id", 312)] and heal["hit"] == [("id", 320)]
    cold = skills[skill_ids["MG_COLDBOLT"]]
    assert cold["before_hit"] == [("name", "ef_coldbolt")] and cold["hit"] == [("id", 51)]
    brandish = skills[skill_ids["KN_BRANDISHSPEAR"]]
    assert brandish["hide_cast_bar"] and brandish["hide_cast_aura"]
    assert ("id", 25) == units[unit_ids["UNT_FIREWALL"]] or units[unit_ids["UNT_FIREWALL"]] == ("id", effect_ids["EF_FIREWALL"])
    assert units[unit_ids["UNT_CLOUD_KILL"]] == ("name", "ef_cloud_kill")
    assert numeric[10][0][0] == "Str" and "maemor" in numeric[10][0][1]
    assert 1884 in numeric, "Turoran heal effect 1884 missing"
    print("spot checks ok", "firewall", units[unit_ids["UNT_FIREWALL"]], "pneuma", units[unit_ids["UNT_PNEUMA"]])


if __name__ == "__main__":
    main()
