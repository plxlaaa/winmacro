# -*- mode: python ; coding: utf-8 -*-
# PyInstaller spec — onedir 构建（所有文件集中在 dist/WinMacro/ 下）。
# 用法（仓库根目录执行）：
#   pyinstaller installer/winmacro.spec --noconfirm --distpath build/app
# 之后把 native DLL 放进 build/app/WinMacro/bin/，再由 NSIS 打成安装器。

import sys
import tomllib
from pathlib import Path

from PyInstaller.utils.versioninfo import (
    FixedFileInfo,
    StringFileInfo,
    StringStruct,
    StringTable,
    VarFileInfo,
    VarStruct,
    VSVersionInfo,
)

REPO = Path(SPECPATH).resolve().parent
sys.path.insert(0, str(REPO))

_version = tomllib.loads((REPO / "pyproject.toml").read_text(encoding="utf-8"))[
    "project"
]["version"]
_vtuple = tuple(int(p) for p in _version.split(".")[:3]) + (0,)

version_info = VSVersionInfo(
    ffi=FixedFileInfo(
        filevers=_vtuple,
        prodvers=_vtuple,
        mask=0x3F,
        flags=0x0,
        OS=0x40004,
        fileType=0x1,
        subtype=0x0,
        date=(0, 0),
    ),
    kids=[
        StringFileInfo(
            [
                StringTable(
                    "080404b0",
                    [
                        StringStruct("CompanyName", "plxlaaa"),
                        StringStruct("FileDescription", "WinMacro 宏录制与回放工具"),
                        StringStruct("FileVersion", _version),
                        StringStruct("InternalName", "WinMacro"),
                        StringStruct("LegalCopyright", "© plxlaaa"),
                        StringStruct("OriginalFilename", "WinMacro.exe"),
                        StringStruct("ProductName", "WinMacro"),
                        StringStruct("ProductVersion", _version),
                    ],
                )
            ]
        ),
        VarFileInfo([VarStruct("Translation", [2052, 1200])]),
    ],
)

a = Analysis(
    [str(REPO / "winmacro" / "__main__.py")],
    pathex=[str(REPO)],
    binaries=[],
    datas=[],
    hiddenimports=[],
    hookspath=[],
    hooksconfig={},
    excludes=["tkinter", "unittest", "pydoc_data", "test"],
    noarchive=False,
    optimize=0,
)
pyz = PYZ(a.pure)

exe = EXE(
    pyz,
    a.scripts,
    exclude_binaries=True,
    name="WinMacro",
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=False,
    console=False,
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
    icon=str(REPO / "installer" / "winmacro.ico"),
    version=version_info,
)

coll = COLLECT(
    exe,
    a.binaries,
    a.datas,
    strip=False,
    upx=False,
    upx_exclude=[],
    name="WinMacro",
)
