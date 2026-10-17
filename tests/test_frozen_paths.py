import sys
from pathlib import Path

from winmacro.backends.native import _candidate_paths


def test_source_tree_candidates(monkeypatch):
    monkeypatch.delattr(sys, "frozen", raising=False)
    paths = _candidate_paths()
    assert paths, "source-tree probing must yield candidates"
    names = {p.name for p in paths}
    if sys.platform == "win32":
        assert names == {"recorder_core.dll"}
    else:
        assert names <= {"librecorder_core.so", "librecorder_core.dylib"}
    # source tree layout must still be probed
    joined = [str(p) for p in paths]
    assert any("native" in p and "recorder_core" in p for p in joined)


def test_frozen_build_probes_next_to_executable(monkeypatch, tmp_path):
    app_dir = tmp_path / "WinMacro"
    (app_dir / "bin").mkdir(parents=True)
    exe = app_dir / "WinMacro.exe"
    exe.write_bytes(b"fake exe")

    monkeypatch.setattr(sys, "frozen", True, raising=False)
    monkeypatch.setattr(sys, "executable", str(exe))

    paths = _candidate_paths()
    # frozen probing must come first so the bundled DLL wins over
    # any stale source-tree artifacts on the build machine
    first = paths[0]
    assert first.parent == app_dir / "bin"
    # within the first two probed folders, the exe's own directory follows
    head = paths[: 2 * len({p.name for p in paths})]
    assert any(p.parent == app_dir for p in head)


def test_frozen_candidates_resolve_real_dll(monkeypatch, tmp_path):
    app_dir = tmp_path / "WinMacro"
    (app_dir / "bin").mkdir(parents=True)
    exe = app_dir / "WinMacro.exe"
    exe.write_bytes(b"fake exe")

    if sys.platform == "win32":
        dll = app_dir / "bin" / "recorder_core.dll"
    else:
        dll = app_dir / "bin" / "librecorder_core.so"
    dll.write_bytes(b"fake dll")

    monkeypatch.setattr(sys, "frozen", True, raising=False)
    monkeypatch.setattr(sys, "executable", str(exe))

    from winmacro.backends.native import find_native_library

    assert find_native_library() == dll
