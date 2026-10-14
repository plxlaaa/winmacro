from __future__ import annotations

import sys

from PySide6.QtCore import Qt
from PySide6.QtGui import QFont
from PySide6.QtWidgets import QApplication

from winmacro.main_window import MainWindow
from winmacro.screen import enable_dpi_awareness


def main(argv: list[str] | None = None) -> int:
    args = argv if argv is not None else sys.argv
    enable_dpi_awareness()
    QApplication.setHighDpiScaleFactorRoundingPolicy(
        Qt.HighDpiScaleFactorRoundingPolicy.PassThrough
    )
    app = QApplication(args)
    app.setApplicationName("WinMacro")
    app.setOrganizationName("WinMacro")
    font = QFont("Segoe UI")
    font.setPixelSize(13)
    app.setFont(font)
    window = MainWindow()
    window.show()
    return app.exec()
