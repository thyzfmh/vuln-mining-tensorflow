#!/usr/bin/env python3
"""Tests for platform asset discovery (target root and work root resolution).

These tests prove that the package discovers the judge-provided target source
tree without assuming code/ contains it, and that the work output root is
resolved correctly — all without a real target checkout.
"""
from __future__ import annotations

import importlib
import os
import pathlib
import sys
import tempfile

SCRIPT_DIR = pathlib.Path(__file__).resolve().parent.parent / "scripts"
sys.path.insert(0, str(SCRIPT_DIR))


def reload_platform_assets():
    """Reload platform_assets so it picks up environment changes."""
    if "platform_assets" in sys.modules:
        del sys.modules["platform_assets"]
    return importlib.import_module("platform_assets")


def write_source(path: pathlib.Path, body: str = "int main() { return 0; }\n") -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(body)


def test_env_var_target_root():
    """VULN_TARGET_ROOT env var takes priority over code/."""
    with tempfile.TemporaryDirectory() as td:
        fake_target = pathlib.Path(td) / "judge_project"
        write_source(fake_target / "src" / "main.c")
        code_dir = pathlib.Path(td) / "code"
        code_dir.mkdir()
        (code_dir / ".gitkeep").write_text("")

        env = os.environ.copy()
        env["VULN_TARGET_ROOT"] = str(fake_target)
        old_env = os.environ.copy()
        os.environ.clear()
        os.environ.update(env)
        try:
            pa = reload_platform_assets()
            result = pa.discover_target_root()
            assert result == fake_target.resolve(), f"expected {fake_target.resolve()}, got {result}"
        finally:
            os.environ.clear()
            os.environ.update(old_env)
    print("test_env_var_target_root: PASS")


def test_target_root_env_var_alias():
    """TARGET_ROOT env var is also checked."""
    with tempfile.TemporaryDirectory() as td:
        fake_target = pathlib.Path(td) / "another_project"
        write_source(fake_target / "lib.cc")

        old_env = os.environ.copy()
        os.environ.pop("VULN_TARGET_ROOT", None)
        os.environ["TARGET_ROOT"] = str(fake_target)
        try:
            pa = reload_platform_assets()
            result = pa.discover_target_root()
            assert result == fake_target.resolve(), f"expected {fake_target.resolve()}, got {result}"
        finally:
            os.environ.clear()
            os.environ.update(old_env)
    print("test_target_root_env_var_alias: PASS")


def test_persisted_work_context():
    """A work context marker restores the target without an exported path."""
    with tempfile.TemporaryDirectory() as td:
        root = pathlib.Path(td)
        target = root / "judge_target"
        work = root / "work"
        write_source(target / "module.py")
        work.mkdir()
        (work / ".vuln-mining-target-root").write_text(f"{target}\n")

        old_env = os.environ.copy()
        os.environ.pop("VULN_TARGET_ROOT", None)
        os.environ.pop("TARGET_ROOT", None)
        os.environ["VULN_WORK_ROOT"] = str(work)
        try:
            pa = reload_platform_assets()
            assert pa.discover_target_root() == target.resolve()
        finally:
            os.environ.clear()
            os.environ.update(old_env)
    print("test_persisted_work_context: PASS")


def test_explicit_arg_highest_priority():
    """Explicit argument overrides env vars."""
    with tempfile.TemporaryDirectory() as td:
        explicit_target = pathlib.Path(td) / "explicit"
        write_source(explicit_target / "main.py", "print('hello')\n")
        env_target = pathlib.Path(td) / "envvar"
        write_source(env_target / "other.py", "x = 1\n")

        old_env = os.environ.copy()
        os.environ["VULN_TARGET_ROOT"] = str(env_target)
        try:
            pa = reload_platform_assets()
            result = pa.discover_target_root(str(explicit_target))
            assert result == explicit_target.resolve(), f"expected {explicit_target.resolve()}, got {result}"
        finally:
            os.environ.clear()
            os.environ.update(old_env)
    print("test_explicit_arg_highest_priority: PASS")


def test_code_dir_with_source_files():
    """code/ with direct source files is used as fallback."""
    with tempfile.TemporaryDirectory() as td:
        code_dir = pathlib.Path(td) / "code"
        write_source(code_dir / "tool.c")

        old_env = os.environ.copy()
        os.environ.pop("VULN_TARGET_ROOT", None)
        os.environ.pop("TARGET_ROOT", None)
        try:
            pa = reload_platform_assets()
            # We need to temporarily make REPO_ROOT point to td so code/ is found
            original_repo_root = pa.REPO_ROOT
            pa.REPO_ROOT = pathlib.Path(td)
            result = pa.discover_target_root()
            assert result == code_dir.resolve(), f"expected {code_dir.resolve()}, got {result}"
            pa.REPO_ROOT = original_repo_root
        finally:
            os.environ.clear()
            os.environ.update(old_env)
    print("test_code_dir_with_source_files: PASS")


def test_code_dir_single_subdir():
    """code/ with one subdir containing source uses that subdir."""
    with tempfile.TemporaryDirectory() as td:
        code_dir = pathlib.Path(td) / "code"
        pkg = code_dir / "somepkg"
        write_source(pkg / "main.cpp")
        (code_dir / ".gitkeep").write_text("")

        old_env = os.environ.copy()
        os.environ.pop("VULN_TARGET_ROOT", None)
        os.environ.pop("TARGET_ROOT", None)
        try:
            pa = reload_platform_assets()
            original_repo_root = pa.REPO_ROOT
            pa.REPO_ROOT = pathlib.Path(td)
            result = pa.discover_target_root()
            assert result == pkg.resolve(), f"expected {pkg.resolve()}, got {result}"
            pa.REPO_ROOT = original_repo_root
        finally:
            os.environ.clear()
            os.environ.update(old_env)
    print("test_code_dir_single_subdir: PASS")


def test_no_target_found_raises():
    """SystemExit when no target can be found."""
    with tempfile.TemporaryDirectory() as td:
        code_dir = pathlib.Path(td) / "code"
        code_dir.mkdir()
        (code_dir / ".gitkeep").write_text("")

        old_env = os.environ.copy()
        os.environ.pop("VULN_TARGET_ROOT", None)
        os.environ.pop("TARGET_ROOT", None)
        try:
            pa = reload_platform_assets()
            original_repo_root = pa.REPO_ROOT
            pa.REPO_ROOT = pathlib.Path(td)
            try:
                pa.discover_target_root()
                raise AssertionError("expected SystemExit")
            except SystemExit:
                pass
            pa.REPO_ROOT = original_repo_root
        finally:
            os.environ.clear()
            os.environ.update(old_env)
    print("test_no_target_found_raises: PASS")


def test_work_root_env_var():
    """VULN_WORK_ROOT env var is used for work root."""
    with tempfile.TemporaryDirectory() as td:
        custom_work = pathlib.Path(td) / "custom_work"
        old_env = os.environ.copy()
        os.environ["VULN_WORK_ROOT"] = str(custom_work)
        try:
            pa = reload_platform_assets()
            result = pa.resolve_work_root()
            assert result == custom_work.resolve(), f"expected {custom_work.resolve()}, got {result}"
        finally:
            os.environ.clear()
            os.environ.update(old_env)
    print("test_work_root_env_var: PASS")


def test_work_root_explicit_arg():
    """Explicit work-root arg takes priority over env vars."""
    with tempfile.TemporaryDirectory() as td:
        explicit_work = pathlib.Path(td) / "explicit_work"
        env_work = pathlib.Path(td) / "env_work"
        old_env = os.environ.copy()
        os.environ["VULN_WORK_ROOT"] = str(env_work)
        try:
            pa = reload_platform_assets()
            result = pa.resolve_work_root(str(explicit_work))
            assert result == explicit_work.resolve(), f"expected {explicit_work.resolve()}, got {result}"
        finally:
            os.environ.clear()
            os.environ.update(old_env)
    print("test_work_root_explicit_arg: PASS")


def test_has_source_files():
    pa = reload_platform_assets()
    with tempfile.TemporaryDirectory() as td:
        empty = pathlib.Path(td) / "empty"
        empty.mkdir()
        assert not pa.has_source_files(empty)
        with_source = pathlib.Path(td) / "with_source"
        write_source(with_source / "a.py")
        assert pa.has_source_files(with_source)
    print("test_has_source_files: PASS")


def test_output_path_creates_parents():
    pa = reload_platform_assets()
    with tempfile.TemporaryDirectory() as td:
        work = pathlib.Path(td) / "work"
        p = pa.output_path(work, "reports", "sub", "file.md")
        assert p == work / "reports" / "sub" / "file.md"
        assert p.parent.is_dir()
    print("test_output_path_creates_parents: PASS")


def test_target_blacklist_names():
    pa = reload_platform_assets()
    with tempfile.TemporaryDirectory() as td:
        target = pathlib.Path(td) / "secret_project"
        pkg = target / "secret_pkg"
        pkg.mkdir(parents=True)
        names = pa.target_blacklist_names(target)
        assert "secret_project" in names
        assert "secret_pkg" in names
    print("test_target_blacklist_names: PASS")


def test_target_blacklist_names_excludes_code_and_work():
    pa = reload_platform_assets()
    with tempfile.TemporaryDirectory() as td:
        target = pathlib.Path(td) / "code"
        target.mkdir()
        names = pa.target_blacklist_names(target)
        assert "code" not in names
    print("test_target_blacklist_names_excludes_code_and_work: PASS")


def test_env_for_subprocess():
    pa = reload_platform_assets()
    with tempfile.TemporaryDirectory() as td:
        target = pathlib.Path(td) / "target"
        work = pathlib.Path(td) / "work"
        env = pa.env_for_subprocess(target, work)
        assert env["VULN_TARGET_ROOT"] == str(target)
        assert env["VULN_WORK_ROOT"] == str(work)
    print("test_env_for_subprocess: PASS")


def main() -> None:
    test_env_var_target_root()
    test_target_root_env_var_alias()
    test_persisted_work_context()
    test_explicit_arg_highest_priority()
    test_code_dir_with_source_files()
    test_code_dir_single_subdir()
    test_no_target_found_raises()
    test_work_root_env_var()
    test_work_root_explicit_arg()
    test_has_source_files()
    test_output_path_creates_parents()
    test_target_blacklist_names()
    test_target_blacklist_names_excludes_code_and_work()
    test_env_for_subprocess()
    print("platform_asset_discovery_test.py: PASS")


if __name__ == "__main__":
    main()
