from __future__ import annotations

from PySide6.QtCore import Qt, QTimer
from PySide6.QtWidgets import (
    QDialog,
    QHBoxLayout,
    QHeaderView,
    QLabel,
    QPushButton,
    QTableWidget,
    QTableWidgetItem,
    QVBoxLayout,
)

from winmacro.models import Recording


MAX_ROWS = 20000
PAGE_ROWS = 500


class RecordingDetailsDialog(QDialog):
    def __init__(self, recording: Recording, parent=None) -> None:
        super().__init__(parent)
        self.recording = recording
        self.setWindowTitle(recording.name)
        self.resize(820, 560)
        self.setModal(False)
        self._events: list = []
        self._index = 0
        self._timer = QTimer(self)
        self._timer.setInterval(0)
        self._timer.timeout.connect(self._fill_next)
        self._build()

    def closeEvent(self, event) -> None:
        self._timer.stop()
        super().closeEvent(event)

    def load(self, recording: Recording) -> None:
        self.recording = recording
        self.setWindowTitle(recording.name)
        self._title.setText(recording.name)
        total = recording.step_count()
        shown = min(total, MAX_ROWS)
        suffix = "" if shown == total else f"（仅显示前 {shown} 步）"
        self._meta.setText(
            f"创建 {recording.created_at}    ·    "
            f"{total} 步{suffix}    ·    "
            f"时长 {recording.duration_text()}    ·    "
            f"分辨率 {recording.resolution_text()}"
        )
        self._fill_table()

    def _fill_table(self) -> None:
        self._timer.stop()
        self.table.setRowCount(0)
        self._events = self.recording.events[:MAX_ROWS]
        self._index = 0
        self._fill_next()

    def _fill_next(self) -> None:
        start = self._index
        batch = self._events[start : start + PAGE_ROWS]
        if not batch:
            return
        self.table.setRowCount(start + len(batch))
        for offset, event in enumerate(batch):
            row = start + offset
            values = [
                f"{row + 1:04d}",
                event.label(),
                event.time_text(),
                event.position_text(),
                event.extra_text(),
            ]
            for col, value in enumerate(values):
                item = QTableWidgetItem(value)
                item.setTextAlignment(Qt.AlignmentFlag.AlignVCenter | Qt.AlignmentFlag.AlignLeft)
                self.table.setItem(row, col, item)
        self._index = start + len(batch)
        if self._index < len(self._events):
            self._timer.start()
        else:
            self._timer.stop()
