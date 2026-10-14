from __future__ import annotations

from PySide6.QtCore import QEvent, QPoint, Qt, Signal
from PySide6.QtGui import QCursor, QMouseEvent
from PySide6.QtWidgets import (
    QFrame,
    QHBoxLayout,
    QLabel,
    QPushButton,
    QVBoxLayout,
    QWidget,
)

from winmacro.geometry import child_screen_rect, inflate_rect, widget_screen_rect
from winmacro.theme import OVERLAY_QSS


class ScreenOverlay(QWidget):
    record_toggled = Signal(bool)
    closed = Signal()
    moved_or_resized = Signal()

    def __init__(self) -> None:
        flags = (
            Qt.WindowType.FramelessWindowHint
            | Qt.WindowType.WindowStaysOnTopHint
            | Qt.WindowType.Tool
        )
        super().__init__(None, flags)
        self.setObjectName("OverlayRoot")
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground, True)
        self.setWindowTitle("WinMacro 屏幕组件")
        self.setStyleSheet(OVERLAY_QSS)
        self.resize(300, 108)
        self._drag_origin: QPoint | None = None
        self._recording = False
        self._build()

    def _build(self) -> None:
        root = QVBoxLayout(self)
        root.setContentsMargins(0, 0, 0, 0)

        self.card = QFrame()
        self.card.setObjectName("OverlayCard")
        layout = QVBoxLayout(self.card)
        layout.setContentsMargins(14, 12, 12, 14)
        layout.setSpacing(8)

        header = QHBoxLayout()
        self.grip = QLabel("⋮⋮  屏幕组件")
        self.grip.setObjectName("OverlayTitle")
        self.grip.setCursor(Qt.CursorShape.SizeAllCursor)
        close_btn = QPushButton("×")
        close_btn.setObjectName("IconBtn")
        close_btn.setCursor(QCursor(Qt.CursorShape.PointingHandCursor))
        close_btn.setFocusPolicy(Qt.FocusPolicy.NoFocus)
        close_btn.clicked.connect(self._close)
        header.addWidget(self.grip)
        header.addStretch()
        header.addWidget(close_btn)

        self.hint = QLabel("拖动此组件调整位置")
        self.hint.setObjectName("OverlayHint")

        body = QHBoxLayout()
        self.dot = QLabel("●  待命")
        self.dot.setObjectName("OverlayHint")
        self.rec_btn = QPushButton("开始录制")
        self.rec_btn.setObjectName("RecButton")
        self.rec_btn.setCursor(QCursor(Qt.CursorShape.PointingHandCursor))
        self.rec_btn.setFocusPolicy(Qt.FocusPolicy.NoFocus)
        self.rec_btn.clicked.connect(self._toggle)
        body.addWidget(self.dot)
        body.addStretch()
        body.addWidget(self.rec_btn)

        layout.addLayout(header)
        layout.addWidget(self.hint)
        layout.addLayout(body)
        root.addWidget(self.card)

        self.grip.installEventFilter(self)
        self.hint.installEventFilter(self)
        self.dot.installEventFilter(self)

    def eventFilter(self, watched, event) -> bool:
        if watched in (self.grip, self.hint, self.dot):
            if event.type() == QEvent.Type.MouseButtonPress:
                self._begin_drag(event)
                return True
            if event.type() == QEvent.Type.MouseMove:
                self._drag_to(event)
                return True
            if event.type() == QEvent.Type.MouseButtonRelease:
                self._end_drag()
                return True
        return super().eventFilter(watched, event)

    def _is_control(self, widget) -> bool:
        while widget is not None and widget is not self:
            if widget is self.rec_btn or widget.objectName() == "IconBtn":
                return True
            widget = widget.parentWidget()
        return False

    def mousePressEvent(self, event: QMouseEvent) -> None:
        child = self.childAt(event.position().toPoint())
        if self._is_control(child):
            super().mousePressEvent(event)
            return
        self._begin_drag(event)
        super().mousePressEvent(event)

    def mouseMoveEvent(self, event: QMouseEvent) -> None:
        self._drag_to(event)
        super().mouseMoveEvent(event)

    def mouseReleaseEvent(self, event: QMouseEvent) -> None:
        self._end_drag()
        super().mouseReleaseEvent(event)

    def _begin_drag(self, event) -> None:
        if hasattr(event, "button") and event.button() == Qt.MouseButton.LeftButton:
            self._drag_origin = event.globalPosition().toPoint() - self.frameGeometry().topLeft()

    def _drag_to(self, event) -> None:
        if self._drag_origin is None:
            return
        if not (event.buttons() & Qt.MouseButton.LeftButton):
            return
        self.move(event.globalPosition().toPoint() - self._drag_origin)
        self.moved_or_resized.emit()

    def _end_drag(self) -> None:
        if self._drag_origin is not None:
            self._drag_origin = None
            self.moved_or_resized.emit()

    def _close(self) -> None:
        if self._recording:
            self.set_recording(False)
            self.record_toggled.emit(False)
        self.hide()
        self.closed.emit()

    def _toggle(self) -> None:
        self.set_recording(not self._recording)
        self.record_toggled.emit(self._recording)

    def set_recording(self, active: bool) -> None:
        self._recording = active
        self.rec_btn.setProperty("recording", "true" if active else "false")
        self.rec_btn.style().unpolish(self.rec_btn)
        self.rec_btn.style().polish(self.rec_btn)
        if active:
            self.rec_btn.setText("结束录制")
            self.dot.setText("●  录制中")
            self.dot.setStyleSheet("color:#e14b5c; font-weight:700;")
            self.hint.setText("正在记录整个屏幕的键盘 / 鼠标")
        else:
            self.rec_btn.setText("开始录制")
            self.dot.setText("●  待命")
            self.dot.setStyleSheet("")
            self.hint.setText("拖动此组件调整位置")

    def is_recording(self) -> bool:
        return self._recording

    def overlay_rect(self) -> tuple[int, int, int, int]:
        return widget_screen_rect(self)

    def record_button_rect(self) -> tuple[int, int, int, int]:
        return inflate_rect(child_screen_rect(self, self.rec_btn), 8)
