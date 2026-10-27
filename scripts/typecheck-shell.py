#!/usr/bin/env python3
"""Type-check the shared-closure web shell.

The menu parts are not modules: `main_menu.core.ts` opens one closure that the
following parts keep filling and `main_menu.shell.ts` closes. TypeScript cannot
parse those fragments on their own, so they are concatenated in bundle order
and checked as the single program the browser actually runs. Module parts
(campaign engine, tutorial, editor guide) are checked separately by
`sow-web/tsconfig.json`.

Usage: python3 scripts/typecheck-shell.py [--variant default|poki|jest]
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SHELL = ROOT / "sow-web" / "shell"

# Bundle order, kept in sync with `read_shell_bundle` in sow-dist/src/main.rs.
PARTS = [
    "sow-i18n.ts",
    "sow-dropdown.ts",
    "main_menu.flow.ts",
    "main_menu.core.ts",
    "main_menu.motion.ts",
    "main_menu.lobbies.ts",
    "main_menu.store.ts",
    "main_menu.heroes.ts",
    "main_menu.profile.ts",
    "main_menu.shell.ts",
    "main_menu.hud.ts",
]
VARIANT_PARTS = {
    "default": [],
    "poki": ["main_menu.poki.ts"],
    "jest": ["main_menu.jest.ts"],
}
# Portal builds strip these blocks from the shell before inserting the portal
# surface, so the variants never define the same renderer twice.
PORTAL_OVERRIDE_PARTS = ("main_menu.poki.ts", "main_menu.jest.ts")
STRIPPED_BLOCKS = [
    ("/* POKI_RENDER_REPLACEMENT_BEGIN */", "/* POKI_RENDER_REPLACEMENT_END */"),
    ("/* POKI_SHARED_STORE_ACTIONS_BEGIN */", "/* POKI_SHARED_STORE_ACTIONS_END */"),
    ("/* POKI_SHARED_STORE_UNLOCK_BEGIN */", "/* POKI_SHARED_STORE_UNLOCK_END */"),
    ("/* POKI_SHARED_AUTH_ACTIONS_BEGIN */", "/* POKI_SHARED_AUTH_ACTIONS_END */"),
    ("/* POKI_SHARED_AUTH_EVENTS_BEGIN */", "/* POKI_SHARED_AUTH_EVENTS_END */"),
    ("/* POKI_SHARED_STORE_EVENTS_BEGIN */", "/* POKI_SHARED_STORE_EVENTS_END */"),
    ("/* POKI_SHARED_AUTH_SUBMIT_BEGIN */", "/* POKI_SHARED_AUTH_SUBMIT_END */"),
    ("/* POKI_SHARED_AUTH_LOOKUP_BEGIN */", "/* POKI_SHARED_AUTH_LOOKUP_END */"),
    ("/* POKI_SHARED_SIGNIN_MARKUP_BEGIN */", "/* POKI_SHARED_SIGNIN_MARKUP_END */"),
]

TSC_ARGS = [
    "bunx",
    "tsc",
    "--ignoreConfig",
    "--noEmit",
    "--target",
    "ES2022",
    "--module",
    "preserve",
    "--moduleResolution",
    "bundler",
    "--lib",
    "ES2022,DOM,DOM.Iterable",
    "--strict",
    "false",
    "--skipLibCheck",
]
FORMAT_PARTS = [
    "main_menu.core.ts",
    "main_menu.lobbies.ts",
    "main_menu.motion.ts",
    "main_menu.store.ts",
    "main_menu.heroes.ts",
    "main_menu.profile.ts",
    "main_menu.shell.ts",
    "main_menu.poki.ts",
    "main_menu.jest.ts",
]
PORTAL_FORMAT_PARTS = ["main_menu.poki.ts", "main_menu.jest.ts"]


def strip_blocks(source: str) -> str:
    for begin, end in STRIPPED_BLOCKS:
        while begin in source:
            start = source.index(begin)
            stop = source.index(end, start) + len(end)
            source = source[:start] + source[stop:]
    return source


def build(variant: str) -> tuple[pathlib.Path, list[tuple[int, str, int]]]:
    order = list(PARTS)
    if variant == "poki":
        order.insert(order.index("main_menu.shell.ts"), "main_menu.poki.ts")
    elif variant == "jest":
        order.insert(order.index("main_menu.shell.ts"), "main_menu.jest.ts")
    chunks = []
    portal_names = sorted({
        name
        for portal in VARIANT_PARTS[variant]
        for name in re.findall(r"(?m)^function ([A-Za-z_$][\w$]*)\(", (SHELL / portal).read_text())
    })
    if portal_names:
        chunks.append("let " + ", ".join(portal_names) + ";")
    # (first bundle line, part name, line count) for error remapping.
    origins: list[tuple[int, str, int]] = []
    # Variant bundles start with the synthetic override declarations above.
    line = 2 if portal_names else 1
    for part in order:
        source = (SHELL / part).read_text()
        if variant != "default":
            source = strip_blocks(source)
        if part in PORTAL_OVERRIDE_PARTS:
            # Portal functions replace shared declarations. Model those overrides
            # as assignments so TypeScript sees one binding across the bundle.
            source = re.sub(
                r"(?m)^function ([A-Za-z_$][\w$]*)\(([^)]*)\)",
                lambda match: f"{match.group(1)} = function {match.group(1)}({match.group(2)})",
                source,
            )
        elif portal_names:
            for name in portal_names:
                source = re.sub(
                    rf"(?m)^(\s*)function {re.escape(name)}\(",
                    rf"\1function __shared_{name}(",
                    source,
                )
        count = source.count("\n") + 1
        origins.append((line, part, count))
        chunks.append(source)
        line += count
    bundle = SHELL / ".typecheck.ts"
    bundle.write_text("\n".join(chunks))
    return bundle, origins


def remap(origins: list[tuple[int, str, int]], line: int) -> str:
    for start, part, count in origins:
        if start <= line < start + count:
            return f"{part}:{line - start + 1}"
    return f".typecheck.ts:{line}"


def lint_shared_variant(bundle: pathlib.Path, origins: list[tuple[int, str, int]], variant: str) -> int:
    result = subprocess.run(
        ["bunx", "eslint", "--format", "json", str(bundle)],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if result.returncode not in (0, 1):
        print(result.stderr or result.stdout, file=sys.stderr)
        return result.returncode
    try:
        reports = json.loads(result.stdout)
    except json.JSONDecodeError:
        print(result.stderr or result.stdout, file=sys.stderr)
        return 1

    errors = 0
    warnings = 0
    filtered = 0
    for report in reports:
        for message in report["messages"]:
            source_line = message.get("line", 1)
            source_part = next(
                (part for start, part, count in origins if start <= source_line < start + count),
                None,
            )
            # The default assembly lints every shared fragment with its normal
            # declarations intact. Portal assembly removes platform-specific
            # call sites and renames overridden functions, which can create
            # unused/const-only diagnostics in those same shared fragments.
            # Keep all other variant diagnostics, including every portal part.
            if (
                variant != "default"
                and source_part in PARTS
                and message.get("ruleId") in {"@typescript-eslint/no-unused-vars", "prefer-const"}
            ):
                filtered += 1
                continue
            severity = message["severity"]
            errors += severity == 2
            warnings += severity == 1
            part, part_line = source_part or ".typecheck.ts", source_line
            if source_part:
                part_line = source_line - next(start for start, name, _ in origins if name == source_part)
            print(
                f"sow-web/shell/{part}:{part_line}:{message.get('column', 1)}: "
                f"{message.get('ruleId') or 'eslint'}: {message['message']}"
            )
    print(
        f"Linted {variant} shell variant: {errors} errors, {warnings} warnings"
        + (f" ({filtered} shared-fragment diagnostics filtered after default assembly lint)." if filtered else ".")
    )
    return 1 if errors or warnings else 0


def format_shared_parts(check_only: bool) -> int:
    temporary = SHELL / ".typecheck.ts"
    failures = []
    for part in FORMAT_PARTS:
        source_path = SHELL / part
        source = source_path.read_text()
        if part == "main_menu.core.ts":
            wrapped = source.rstrip() + "\n})();\n"
            wrapper = "core"
        elif part == "main_menu.shell.ts":
            wrapped = "(function () {\n" + source
            wrapper = "shell"
        else:
            wrapped = "(function () {\n" + source.rstrip() + "\n})();\n"
            wrapper = "body"
        temporary.write_text(wrapped)
        try:
            command = ["bunx", "prettier", "--parser", "typescript"]
            command.append(str(temporary))
            result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
            if result.returncode != 0:
                print(result.stderr or result.stdout, file=sys.stderr)
                return result.returncode
            formatted = result.stdout
            lines = formatted.splitlines()
            if wrapper == "core":
                if not lines or lines[-1].strip() != "})();":
                    raise RuntimeError(f"could not isolate formatter wrapper for {part}")
                lines.pop()
            elif wrapper == "shell":
                if not lines or lines[0].strip() != "(function () {":
                    raise RuntimeError(f"could not isolate formatter wrapper for {part}")
                lines.pop(0)
            else:
                if len(lines) < 2 or lines[0].strip() != "(function () {" or lines[-1].strip() != "})();":
                    raise RuntimeError(f"could not isolate formatter wrapper for {part}")
                lines = lines[1:-1]
            output = "\n".join(lines).rstrip() + "\n"
            if check_only:
                if output != source:
                    failures.append(part)
            else:
                source_path.write_text(output)
        finally:
            temporary.unlink(missing_ok=True)
    if failures:
        print("Shared shell fragments need formatting: " + ", ".join(failures))
        return 1
    print("All shared shell fragments use Prettier code style!")
    return 0


def fix_shared_shell() -> int:
    temporary = SHELL / ".typecheck.ts"
    chunks = []
    for part in PARTS:
        chunks.append(f"// __SOW_PART__:{part}\n")
        chunks.append((SHELL / part).read_text())
        if not chunks[-1].endswith("\n"):
            chunks.append("\n")
    temporary.write_text("".join(chunks))
    try:
        subprocess.run(["bunx", "eslint", "--fix", str(temporary)], cwd=ROOT, stdout=subprocess.DEVNULL)
        output = temporary.read_text()
    finally:
        temporary.unlink(missing_ok=True)
    matches = list(re.finditer(r"(?m)^// __SOW_PART__:(.+)$", output))
    if len(matches) != len(PARTS):
        print("ESLint fixes changed shell source markers; refusing to map edits back.", file=sys.stderr)
        return 1
    for index, match in enumerate(matches):
        end = matches[index + 1].start() if index + 1 < len(matches) else len(output)
        body = output[match.end():end]
        if body.startswith("\n"):
            body = body[1:]
        (SHELL / match.group(1)).write_text(body)
    for part in PORTAL_FORMAT_PARTS:
        source_path = SHELL / part
        source = source_path.read_text()
        temporary.write_text("(function () {\n" + source.rstrip() + "\n})();\n")
        try:
            subprocess.run(
                ["bunx", "eslint", "--fix", str(temporary)],
                cwd=ROOT,
                stdout=subprocess.DEVNULL,
            )
            lines = temporary.read_text().splitlines()
        finally:
            temporary.unlink(missing_ok=True)
        if len(lines) < 2 or lines[0].strip() != "(function () {" or lines[-1].strip() != "})();":
            print(f"ESLint fixes changed shell wrapper for {part}; refusing to map edits back.", file=sys.stderr)
            return 1
        source_path.write_text("\n".join(lines[1:-1]).rstrip() + "\n")
    print("Applied ESLint automatic fixes to shared shell parts.")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--variant", choices=sorted(VARIANT_PARTS), default="default")
    parser.add_argument("--lint", action="store_true")
    parser.add_argument("--format", action="store_true")
    parser.add_argument("--format-check", action="store_true")
    parser.add_argument("--fix-shared", action="store_true")
    args = parser.parse_args()
    if args.fix_shared:
        return fix_shared_shell()
    if args.format or args.format_check:
        return format_shared_parts(check_only=args.format_check)
    bundle, origins = build(args.variant)
    if args.lint:
        try:
            return lint_shared_variant(bundle, origins, args.variant)
        finally:
            bundle.unlink(missing_ok=True)
    try:
        result = subprocess.run(
            [*TSC_ARGS, str(bundle), str(SHELL / "globals.d.ts")],
            cwd=ROOT / "sow-web",
            capture_output=True,
            text=True,
        )
    finally:
        bundle.unlink(missing_ok=True)
    output = (result.stdout + result.stderr).strip()
    if not output:
        return 0
    pattern = re.compile(r"\.typecheck\.ts\((\d+),(\d+)\): (error TS\d+: .*)")
    lines = []
    for entry in output.splitlines():
        match = pattern.search(entry)
        if match:
            lines.append(f"{remap(origins, int(match.group(1)))}:{match.group(2)}: {match.group(3)}")
        elif "error TS" in entry:
            lines.append(entry)
    print("\n".join(lines) if lines else output)
    return 1 if lines else result.returncode


if __name__ == "__main__":
    sys.exit(main())
