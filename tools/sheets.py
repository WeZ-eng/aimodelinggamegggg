#!/usr/bin/env python3
"""Design sheets: preflight and code generation.

    python3 tools/sheets.py preflight            # blocks a build on unfilled cells or broken refs
    python3 tools/sheets.py preflight --release  # also blocks on cells not yet verified in game
    python3 tools/sheets.py codegen              # sheets -> mod/src/generated.rs (runs preflight first)

A sheet is sheets/<name>.json: {sheet, about, key, columns{name: {type, ...}}, rows[...]}.
Every row/column crossing is a checkbox:
  UNFILLED    the cell is missing or null
  BAD         wrong type or not one of the enum values
  UNRESOLVED  a ref to a row that does not exist in the target sheet
  UNVERIFIED  filled, but the row's _verify says what still has to be confirmed in the running game
"""
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SHEETS = ROOT / "sheets"
OUT = ROOT / "mod" / "src" / "generated.rs"

# Sheets compiled into the DLL. coop and release feed the Melty recipe instead.
CODE_SHEETS = ["effects", "controls", "camera", "rarity", "artifacts", "enchantments", "loot", "hooks", "ui"]


def load():
    sheets = {}
    for p in sorted(SHEETS.glob("*.json")):
        s = json.loads(p.read_text())
        if s.get("sheet") != p.stem:
            raise SystemExit(f"{p.name}: 'sheet' must be '{p.stem}'")
        sheets[p.stem] = s
    return sheets


def type_ok(col, v):
    t = col["type"]
    if t in ("str", "ref"):
        return isinstance(v, str)
    if t == "enum":
        return isinstance(v, str) and v in col["values"]
    if t == "int":
        return isinstance(v, int) and not isinstance(v, bool)
    if t == "float":
        return isinstance(v, (int, float)) and not isinstance(v, bool)
    if t == "bool":
        return isinstance(v, bool)
    raise SystemExit(f"unknown column type {t}")


def rule_checks(sheets):
    """Cross-sheet rules the column types alone cannot express."""
    out = []
    eff = {r["id"]: r for r in sheets["effects"]["rows"]}
    want_param = {
        "bullet_toward_cursor": "Bullet", "bullet_at_cursor": "Bullet", "bullet_at_self": "Bullet",
        "channel_toward_cursor": "Bullet", "speffect_self": "SpEffectParam",
    }
    for r in sheets["artifacts"]["rows"]:
        e = eff.get(r.get("effect"))
        if e and e["param"] != want_param.get(r.get("kind")):
            out.append(f"artifacts.{r['id']}: kind {r.get('kind')} needs a {want_param.get(r.get('kind'))} effect, got {e['param']}")
        g = eff.get(r.get("er_item"))
        if g and g["param"] != "EquipParamGoods":
            out.append(f"artifacts.{r['id']}: er_item must be an EquipParamGoods row")
        if r.get("kind") == "channel_toward_cursor" and not (r.get("channel_interval_s") or 0) > 0:
            out.append(f"artifacts.{r['id']}: channel needs channel_interval_s > 0")
    for r in sheets["enchantments"]["rows"]:
        e = eff.get(r.get("game_effect"))
        if e and e["id"] != "none" and e["param"] != "SpEffectParam":
            out.append(f"enchantments.{r['id']}: game_effect must be an SpEffectParam row or none")
        if r.get("trigger") == "every_nth_hit" and not (r.get("nth") or 0) > 1:
            out.append(f"enchantments.{r['id']}: every_nth_hit needs nth > 1")
    for r in sheets["loot"]["rows"]:
        if (r.get("w_artifact", 0) + r.get("w_weapon_enchant", 0) + r.get("w_armor_enchant", 0)) <= 0:
            out.append(f"loot.{r['id']}: reward weights sum to 0")
    ctrl = {r["id"] for r in sheets["controls"]["rows"]}
    for needed in ["toggle_camera", "zoom_in", "zoom_out", "artifact_1", "artifact_2", "artifact_3", "artifact_menu", "satchel_next", "aim_attack", "aim_guard"]:
        if needed not in ctrl:
            out.append(f"controls: the DLL reads '{needed}' but the sheet has no such row")
    for needed in ["dungeons", "vanilla"]:
        if needed not in {r["id"] for r in sheets["camera"]["rows"]}:
            out.append(f"camera: the DLL needs a '{needed}' row")
    return out


def preflight(sheets):
    unfilled, bad, unresolved, unverified, problems = [], [], [], [], []
    ids = {name: {r.get(s["key"]) for r in s["rows"]} for name, s in sheets.items()}
    for name, s in sheets.items():
        cols = s["columns"]
        seen = set()
        for i, r in enumerate(s["rows"]):
            rid = r.get(s["key"], f"#{i}")
            if rid in seen:
                problems.append(f"{name}.{rid}: duplicate id")
            seen.add(rid)
            for k in r:
                if k != "_verify" and k not in cols:
                    problems.append(f"{name}.{rid}.{k}: not a column of {name}")
            for k in r.get("_verify", {}):
                if k not in cols:
                    problems.append(f"{name}.{rid}._verify.{k}: not a column")
            for c, col in cols.items():
                v = r.get(c)
                where = f"{name}.{rid}.{c}"
                if v is None:
                    unfilled.append(f"{where}  ({col.get('doc', col['type'])})")
                    continue
                if not type_ok(col, v):
                    bad.append(f"{where} = {v!r}: expected {col['type']}" + (f" {col['values']}" if col['type'] == 'enum' else ""))
                    continue
                if col["type"] == "ref":
                    if col["ref"] not in sheets:
                        unresolved.append(f"{where}: sheet '{col['ref']}' does not exist")
                    elif v not in ids[col["ref"]]:
                        unresolved.append(f"{where} -> {col['ref']}.{v} does not exist")
                if c in r.get("_verify", {}):
                    unverified.append(f"{where}: {r['_verify'][c]}")
    if all(n in sheets for n in ["effects", "artifacts", "enchantments", "loot", "controls", "camera"]):
        problems += rule_checks(sheets)
    total = sum(len(s["rows"]) * len(s["columns"]) for s in sheets.values())
    return dict(total=total, unfilled=unfilled, bad=bad, unresolved=unresolved, unverified=unverified, problems=problems)


def report(res, release):
    def section(title, items):
        print(f"\n{title}: {len(items)}")
        for x in items:
            print(f"  - {x}")
    print(f"sheets: {len(load())}  cells: {res['total']}")
    section("UNFILLED", res["unfilled"])
    section("BAD", res["bad"])
    section("UNRESOLVED", res["unresolved"])
    section("RULES", res["problems"])
    section("UNVERIFIED (must be confirmed in the running game before release)", res["unverified"])
    checked = res["total"] - len(res["unfilled"]) - len(res["bad"]) - len(res["unresolved"]) - len(res["unverified"])
    print(f"\nchecked: {checked}/{res['total']}")


def blocking(res, release, only=None):
    def mine(x):
        return only is None or x.split(".")[0] in only
    b = [x for x in res["unfilled"] + res["bad"] + res["unresolved"] + res["problems"] if mine(x)]
    if release:
        b += res["unverified"]
    return b


# ---------------------------------------------------------------- codegen

def camel(s):
    return "".join(p[:1].upper() + p[1:] for p in s.replace("-", "_").split("_") if p)


def rust_lit(col, v, sheets, sheet_name, c):
    t = col["type"]
    if t in ("str",):
        return json.dumps(v, ensure_ascii=False)
    if t == "int":
        return str(v)
    if t == "float":
        return repr(float(v)) if "." in repr(float(v)) or "e" in repr(float(v)) else f"{float(v)}.0"
    if t == "bool":
        return "true" if v else "false"
    if t == "enum":
        return f"{camel(sheet_name)}{camel(c)}::{camel(v)}"
    if t == "ref":
        return f"&{col['ref'].upper()}_{v.upper()}"
    raise SystemExit(t)


def rust_type(col, sheet_name, c):
    t = col["type"]
    return {
        "str": "&'static str", "int": "i32", "float": "f32", "bool": "bool",
        "enum": f"{camel(sheet_name)}{camel(c)}",
        "ref": f"&'static {camel(col.get('ref', ''))}Row",
    }[t]


def codegen(sheets):
    lines = [
        "// @generated by tools/sheets.py from sheets/*.json. Do not edit: change the sheet, then regenerate.",
        "#![allow(dead_code, clippy::all)]",
        "",
    ]
    for name in CODE_SHEETS:
        s = sheets[name]
        cols = s["columns"]
        lines.append(f"// ---- {name}: {s['about']}")
        for c, col in cols.items():
            if col["type"] == "enum":
                lines.append("#[derive(Clone, Copy, Debug, PartialEq, Eq)]")
                lines.append(f"pub enum {camel(name)}{camel(c)} {{ " + ", ".join(camel(v) for v in col["values"]) + " }")
        lines.append("#[derive(Debug)]")
        lines.append(f"pub struct {camel(name)}Row {{")
        for c, col in cols.items():
            doc = col.get("doc")
            if doc:
                lines.append(f"    /// {doc}")
            lines.append(f"    pub {c}: {rust_type(col, name, c)},")
        lines.append("}")
        consts = []
        for r in s["rows"]:
            cname = f"{name.upper()}_{r[s['key']].upper()}"
            consts.append(cname)
            fields = ", ".join(f"{c}: {rust_lit(col, r[c], sheets, name, c)}" for c, col in cols.items())
            lines.append(f"pub const {cname}: {camel(name)}Row = {camel(name)}Row {{ {fields} }};")
        lines.append(f"pub static {name.upper()}: &[&{camel(name)}Row] = &[" + ", ".join(f"&{c}" for c in consts) + "];")
        lines.append("")
    OUT.write_text("\n".join(lines) + "\n")
    print(f"wrote {OUT.relative_to(ROOT)} ({sum(len(sheets[n]['rows']) for n in CODE_SHEETS)} rows)")


def main():
    args = sys.argv[1:]
    if not args or args[0] not in ("preflight", "codegen"):
        print(__doc__)
        return 2
    sheets = load()
    res = preflight(sheets)
    release = "--release" in args
    if args[0] == "preflight":
        report(res, release)
        b = blocking(res, release)
        print("\nPREFLIGHT " + ("BLOCKED" if b else "CLEAN") + (" (release)" if release else " (build)"))
        return 1 if b else 0
    # codegen only needs the sheets that are compiled in to be clean
    b = blocking(res, False, only=set(CODE_SHEETS))
    if b:
        report(res, False)
        print("\ncodegen refused: fix the blocking cells above first")
        return 1
    codegen(sheets)
    return 0


if __name__ == "__main__":
    sys.exit(main())
