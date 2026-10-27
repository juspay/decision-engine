#!/usr/bin/env python3
"""Structural checks that keep duplication from creeping back in.

Each check guards a pattern a refactor removed:

- orphan-modules: every `.rs` file under `src/` and `crates/*/src/` is reachable from a crate root
  through `mod` declarations. An unreachable file is never compiled, so it rots silently.
- db-cfg: `cfg(feature = "mysql" | "postgres")` stays where the backend is chosen. New code reaches
  the database through `storage::backend`; the per-file counts in `scripts/guardrails/db-cfg.txt`
  may only go down.
- config-keys: merchant configuration names are built by `MerchantConfig::name`, not by a
  `format!` that repeats one of its prefixes.

Run all checks with `python3 scripts/guardrails.py`, or name some: `... orphan-modules db-cfg`.
`--update-baseline` rewrites the db-cfg counts after a change that lowers them.
"""

import re
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DB_CFG_BASELINE = ROOT / "scripts" / "guardrails" / "db-cfg.txt"
CONFIG_KEYS_HOME = Path("src/types/service_configuration.rs")

MOD_DECL = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+)\s*;", re.M)
PATH_ATTR = re.compile(r'#\[path\s*=\s*"([^"]+)"\]\s*(?:#\[[^\]]*\]\s*)*(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+)\s*;')
DB_FEATURE = re.compile(r'feature\s*=\s*"(?:mysql|postgres)"')
CONFIG_PREFIX = re.compile(r'const PREFIX: &\'static str = "(\w+)";')


def rust_sources():
    dirs = [ROOT / "src", *sorted((ROOT / "crates").glob("*/src"))]
    return sorted(p for d in dirs for p in d.rglob("*.rs"))


def rel(path: Path) -> str:
    return path.relative_to(ROOT).as_posix()


def crate_roots():
    roots = [ROOT / "src" / "lib.rs", *sorted((ROOT / "src" / "bin").glob("*.rs"))]
    for src in sorted((ROOT / "crates").glob("*/src")):
        roots += [p for p in (src / "lib.rs", src / "main.rs") if p.exists()]
        roots += sorted((src / "bin").glob("*.rs"))
    return roots


def check_orphan_modules() -> list[str]:
    reached: set[Path] = set()
    pending = [p for p in crate_roots() if p.exists()]
    while pending:
        file = pending.pop()
        if file in reached:
            continue
        reached.add(file)
        text = file.read_text()
        # A crate root or mod.rs declares children beside itself; foo.rs declares them in foo/.
        owns_dir = file.name in ("lib.rs", "main.rs", "mod.rs") or file.parent.name == "bin"
        module_dir = file.parent if owns_dir else file.with_suffix("")
        explicit = {name: file.parent / path for path, name in PATH_ATTR.findall(text)}
        for name in MOD_DECL.findall(text):
            if name in explicit:
                pending.append(explicit[name].resolve())
                continue
            for candidate in (module_dir / f"{name}.rs", module_dir / name / "mod.rs"):
                if candidate.exists():
                    pending.append(candidate)
                    break
    return [
        f"{rel(p)}: not reachable from any crate root through `mod` declarations — "
        "declare it or delete it"
        for p in rust_sources()
        if p not in reached
    ]


def db_cfg_counts() -> Counter:
    counts = Counter()
    for path in rust_sources():
        n = len(DB_FEATURE.findall(path.read_text()))
        if n:
            counts[rel(path)] = n
    return counts


def read_baseline() -> Counter:
    baseline = Counter()
    if DB_CFG_BASELINE.exists():
        for line in DB_CFG_BASELINE.read_text().splitlines():
            line = line.strip()
            if line and not line.startswith("#"):
                count, path = line.split(maxsplit=1)
                baseline[path] = int(count)
    return baseline


def write_baseline(counts: Counter) -> None:
    DB_CFG_BASELINE.parent.mkdir(parents=True, exist_ok=True)
    lines = [
        "# Allowed `feature = \"mysql\"` / `feature = \"postgres\"` occurrences per file.",
        "# Counts may only go down; regenerate with `python3 scripts/guardrails.py --update-baseline`.",
        *(f"{n} {path}" for path, n in sorted(counts.items())),
    ]
    DB_CFG_BASELINE.write_text("\n".join(lines) + "\n")


def check_db_cfg() -> list[str]:
    baseline = read_baseline()
    return [
        f"{path}: {n} DB feature cfg(s), baseline allows {baseline[path]} — "
        "go through `storage::backend` (Db, DbConnection, schema) instead"
        for path, n in sorted(db_cfg_counts().items())
        if n > baseline[path]
    ]


def check_config_keys() -> list[str]:
    prefixes = CONFIG_PREFIX.findall((ROOT / CONFIG_KEYS_HOME).read_text())
    if not prefixes:
        return [f"{CONFIG_KEYS_HOME}: found no `MerchantConfig::PREFIX` constants to check against"]
    built = re.compile(r'format!\(\s*"(?:' + "|".join(map(re.escape, prefixes)) + r')_\{')
    errors = []
    for path in rust_sources():
        if path == ROOT / CONFIG_KEYS_HOME:
            continue
        for lineno, line in enumerate(path.read_text().splitlines(), 1):
            if built.search(line):
                errors.append(
                    f"{rel(path)}:{lineno}: builds a merchant config name by hand — "
                    "use the `MerchantConfig` key's `name()` / typed accessors"
                )
    return errors


CHECKS = {
    "orphan-modules": check_orphan_modules,
    "db-cfg": check_db_cfg,
    "config-keys": check_config_keys,
}


def main(argv: list[str]) -> int:
    if "--update-baseline" in argv:
        write_baseline(db_cfg_counts())
        print(f"wrote {rel(DB_CFG_BASELINE)}")
        return 0
    selected = argv or list(CHECKS)
    unknown = [name for name in selected if name not in CHECKS]
    if unknown:
        print(f"unknown check(s): {', '.join(unknown)}; choose from {', '.join(CHECKS)}")
        return 2
    failed = False
    for name in selected:
        errors = CHECKS[name]()
        print(f"{'FAIL' if errors else 'ok  '} {name}")
        for error in errors:
            print(f"     {error}")
        failed |= bool(errors)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
