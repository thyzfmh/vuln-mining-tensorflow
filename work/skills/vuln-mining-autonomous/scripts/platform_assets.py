#!/usr/bin/env python3
"""Platform asset discovery and work-output layout resolution.

This module centralizes two concerns that issue #1's scoring failure exposed:

1. Discovering the judge-provided target source tree instead of assuming
   ``code/`` always contains it.
2. Routing every runtime artifact beneath ``work/`` so the final gate and
   automated scoring see a single, self-contained output root.

Both concerns are resolved non-interactively through environment variables,
explicit arguments, and deterministic fallback heuristics. The package never
asks the user a question.
"""
from __future__ import annotations

import os
import pathlib

# ---------------------------------------------------------------------------
# Path anchors — derived from this file so the module works regardless of cwd.
# ---------------------------------------------------------------------------

# platform_assets.py lives at:
#   <repo>/work/skills/vuln-mining-autonomous/scripts/platform_assets.py
SCRIPT_DIR = pathlib.Path(__file__).resolve().parent
SKILL_DIR = SCRIPT_DIR.parent
WORK_DIR = SKILL_DIR.parents[1]          # <repo>/work
REPO_ROOT = SKILL_DIR.parents[2]         # <repo>

# ---------------------------------------------------------------------------
# Configuration constants
# ---------------------------------------------------------------------------

TARGET_ROOT_ENV_VARS = ("VULN_TARGET_ROOT", "TARGET_ROOT")
WORK_ROOT_ENV_VARS = ("VULN_WORK_ROOT", "WORK_ROOT")
DEFAULT_WORK_DIR_NAME = "work"
CODE_DIR_NAME = "code"
TARGET_CONTEXT_FILE = ".vuln-mining-target-root"

SOURCE_EXTENSIONS = {
    ".c", ".cc", ".cpp", ".cxx", ".h", ".hh", ".hpp", ".hxx", ".cu", ".py",
}

# Names that must never appear in LLM chat logs (black-box contract).
# These are always forbidden regardless of the discovered target.
ALWAYS_FORBIDDEN_NAMES: frozenset[str] = frozenset()


# ---------------------------------------------------------------------------
# Target-root discovery
# ---------------------------------------------------------------------------

def has_source_files(root: pathlib.Path, max_files: int = 2000) -> bool:
    """Return True if *root* contains at least one file with a source extension."""
    try:
        count = 0
        for path in root.rglob("*"):
            if path.is_file() and path.suffix in SOURCE_EXTENSIONS:
                return True
            count += 1
            if count > max_files:
                # If we scanned 2000 entries and found no source, something is
                # wrong, but we err on the side of "no source" so the caller
                # can try the next candidate.
                return False
    except OSError:
        return False
    return False


def _candidate_target_roots(
    explicit: str | os.PathLike[str] | None,
) -> list[pathlib.Path]:
    """Build the ordered list of target-root candidates."""
    candidates: list[pathlib.Path] = []

    if explicit:
        candidates.append(pathlib.Path(explicit).resolve())

    for env_name in TARGET_ROOT_ENV_VARS:
        env_val = os.environ.get(env_name)
        if env_val:
            p = pathlib.Path(env_val).resolve()
            if p not in candidates:
                candidates.append(p)

    work_roots = [WORK_DIR]
    for env_name in WORK_ROOT_ENV_VARS:
        env_val = os.environ.get(env_name)
        if env_val:
            work_roots.insert(0, pathlib.Path(env_val).resolve())
    cwd = pathlib.Path.cwd().resolve()
    if (cwd / TARGET_CONTEXT_FILE).is_file() and cwd not in work_roots:
        work_roots.insert(0, cwd)
    for work_root in work_roots:
        context = work_root / TARGET_CONTEXT_FILE
        if not context.is_file():
            continue
        try:
            target = pathlib.Path(context.read_text().strip()).resolve()
        except OSError:
            continue
        if target not in candidates:
            candidates.append(target)

    code_dir = REPO_ROOT / CODE_DIR_NAME
    if code_dir.is_dir():
        resolved_code = code_dir.resolve()
        children = [
            p for p in code_dir.iterdir()
            if p.is_dir() and not p.name.startswith(".")
        ]
        # A single non-hidden subdirectory takes priority over code/ itself,
        # matching the original detect_target() behavior (code/<pkg> layout).
        if len(children) == 1:
            child = children[0].resolve()
            if child not in candidates:
                candidates.append(child)
        if resolved_code not in candidates:
            candidates.append(resolved_code)

    return candidates


def discover_target_root(
    explicit: str | os.PathLike[str] | None = None,
) -> pathlib.Path:
    """Discover the judge-provided target source tree.

    Resolution order (first match with real source files wins):

    1. *explicit* argument (highest priority — typically from ``--target-root``).
    2. ``VULN_TARGET_ROOT`` environment variable.
    3. ``TARGET_ROOT`` environment variable.
    4. The target context persisted under the resolved work output root.
    5. ``<repo>/code`` if it directly contains source files.
    6. A single non-hidden subdirectory under ``<repo>/code`` that contains
       source files.

    Returns an absolute, resolved ``pathlib.Path``.
    Raises ``SystemExit`` if no target root can be found.
    """
    candidates = _candidate_target_roots(explicit)
    for candidate in candidates:
        if candidate.is_dir() and has_source_files(candidate):
            return candidate

    searched = ", ".join(str(c) for c in candidates) or "(none)"
    raise SystemExit(
        "could not discover a target source tree with real source files. "
        f"Searched: {searched}. "
        f"Set one of {', '.join(TARGET_ROOT_ENV_VARS)} or pass --target-root."
    )


# ---------------------------------------------------------------------------
# Work-root resolution
# ---------------------------------------------------------------------------

def resolve_work_root(
    explicit: str | os.PathLike[str] | None = None,
) -> pathlib.Path:
    """Resolve the work output root where every runtime artifact is written.

    Resolution order:

    1. *explicit* argument (typically from ``--work-root``).
    2. ``VULN_WORK_ROOT`` environment variable.
    3. ``WORK_ROOT`` environment variable.
    4. A target-context marker in the current directory.
    5. ``<repo>/work`` (default).

    Returns an absolute, resolved ``pathlib.Path``.
    """
    if explicit:
        return pathlib.Path(explicit).resolve()
    for env_name in WORK_ROOT_ENV_VARS:
        env_val = os.environ.get(env_name)
        if env_val:
            return pathlib.Path(env_val).resolve()
    cwd = pathlib.Path.cwd().resolve()
    if (cwd / TARGET_CONTEXT_FILE).is_file():
        return cwd
    return (REPO_ROOT / DEFAULT_WORK_DIR_NAME).resolve()


# ---------------------------------------------------------------------------
# Output-path helpers
# ---------------------------------------------------------------------------

def output_path(work_root: pathlib.Path, *parts: str) -> pathlib.Path:
    """Return a file path under *work_root*, creating parent dirs if needed."""
    path = work_root.joinpath(*parts)
    path.parent.mkdir(parents=True, exist_ok=True)
    return path


def output_dir(work_root: pathlib.Path, *parts: str) -> pathlib.Path:
    """Return a directory path under *work_root*, creating it if needed."""
    path = work_root.joinpath(*parts)
    path.mkdir(parents=True, exist_ok=True)
    return path


# ---------------------------------------------------------------------------
# Black-box helpers
# ---------------------------------------------------------------------------

def target_blacklist_names(target_root: pathlib.Path) -> list[str]:
    """Return directory basenames under *target_root* that must not appear in
    LLM chat logs.

    Only the top-level directory name of the target itself is returned, plus
    any single top-level package directory (the common "code/<pkg>" layout).
    Deeper names are not blacklisted because they are legitimate relative
    paths in source-snippet prompts.
    """
    names: list[str] = []
    target_name = target_root.name
    if target_name and target_name not in {CODE_DIR_NAME, DEFAULT_WORK_DIR_NAME}:
        names.append(target_name)
    children = [
        p for p in target_root.iterdir()
        if p.is_dir() and not p.name.startswith(".")
    ]
    if len(children) == 1:
        pkg = children[0].name
        if pkg not in {CODE_DIR_NAME, DEFAULT_WORK_DIR_NAME}:
            names.append(pkg)
    return names


def env_for_subprocess(
    target_root: pathlib.Path,
    work_root: pathlib.Path,
) -> dict[str, str]:
    """Return an environment dict suitable for subprocess calls to the
    Phase 1 scripts so they pick up the same target and work roots."""
    env = os.environ.copy()
    env[TARGET_ROOT_ENV_VARS[0]] = str(target_root)
    env[WORK_ROOT_ENV_VARS[0]] = str(work_root)
    return env


# ---------------------------------------------------------------------------
# CLI self-test
# ---------------------------------------------------------------------------

if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Platform asset discovery")
    parser.add_argument("--target-root", default=None)
    parser.add_argument("--work-root", default=None)
    args = parser.parse_args()

    target = discover_target_root(args.target_root)
    work = resolve_work_root(args.work_root)
    print(f"TARGET_ROOT={target}")
    print(f"WORK_ROOT={work}")
    print(f"BLACKLIST_NAMES={target_blacklist_names(target)}")
