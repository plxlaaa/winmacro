from __future__ import annotations

from PySide6.QtWidgets import (
    QButtonGroup,
    QComboBox,
    QDialog,
    QFormLayout,
    QFrame,
    QHBoxLayout,
    QLabel,
    QPushButton,
    QRadioButton,
    QSpinBox,
    QVBoxLayout,
    QWidget,
)

from winmacro.screen import COMMON_RESOLUTIONS, detect_primary_resolution, resolution_label
from winmacro.settings import AppSettings


def _wrap(layout) -> QWidget:
    widget = QWidget()
    widget.setLayout(layout)
    return widget


class SettingsDialog(QDialog):
    def __init__(self, settings: AppSettings, parent=None) -> None:
        super().__init__(parent)
        self.setWindowTitle("设置")
        self.setModal(True)
        self.resize(460, 360)
        self._settings = settings
        self._detected = detect_primary_resolution()
        self._build()
        self._load(settings)

    def _build(self) -> None:
        layout = QVBoxLayout(self)
        layout.setContentsMargins(22, 20, 22, 18)
        layout.setSpacing(14)

        title = QLabel("录制分辨率")
        title.setObjectName("Title")
        hint = QLabel("自动识别当前主屏物理分辨率，或手动指定脚本使用的坐标空间。回放时会按比例映射到当前屏幕。")
        hint.setObjectName("Muted")
        hint.setWordWrap(True)
        layout.addWidget(title)
        layout.addWidget(hint)

        card = QFrame()
        card.setObjectName("Card")
        form = QFormLayout(card)
        form.setContentsMargins(16, 14, 16, 14)
        form.setSpacing(10)

        self.auto_radio = QRadioButton("自动识别")
        self.manual_radio = QRadioButton("手动设置")
        self.mode_group = QButtonGroup(self)
        self.mode_group.addButton(self.auto_radio)
        self.mode_group.addButton(self.manual_radio)
        self.auto_radio.toggled.connect(self._on_mode_changed)

        self.detected_label = QLabel()
        self.detected_label.setObjectName("Muted")

        self.preset = QComboBox()
        self.preset.addItem("自定义", None)
        for width, height, name in COMMON_RESOLUTIONS:
            self.preset.addItem(f"{width} × {height}  {name}", (width, height))
        self.preset.currentIndexChanged.connect(self._on_preset_changed)

        self.width_spin = QSpinBox()
        self.width_spin.setRange(320, 16384)
        self.width_spin.setSingleStep(10)
        self.height_spin = QSpinBox()
        self.height_spin.setRange(240, 16384)
        self.height_spin.setSingleStep(10)
        self.width_spin.valueChanged.connect(self._sync_preset_from_spins)
        self.height_spin.valueChanged.connect(self._sync_preset_from_spins)

        detect_btn = QPushButton("重新识别")
        detect_btn.setObjectName("Ghost")
        detect_btn.clicked.connect(self._refresh_detected)

        mode_row = QHBoxLayout()
        mode_row.addWidget(self.auto_radio)
        mode_row.addWidget(self.manual_radio)
        mode_row.addStretch()

        detect_row = QHBoxLayout()
        detect_row.addWidget(self.detected_label, 1)
        detect_row.addWidget(detect_btn)

        size_row = QHBoxLayout()
        size_row.addWidget(self.width_spin)
        size_row.addWidget(QLabel("×"))
        size_row.addWidget(self.height_spin)
        size_row.addStretch()

        form.addRow("模式", _wrap(mode_row))
        form.addRow("当前识别", _wrap(detect_row))
        form.addRow("常用分辨率", self.preset)
        form.addRow("宽度 × 高度", _wrap(size_row))
        layout.addWidget(card)
        layout.addStretch()

        footer = QHBoxLayout()
        footer.addStretch()
        cancel = QPushButton("取消")
        cancel.clicked.connect(self.reject)
        save = QPushButton("保存")
        save.setObjectName("Primary")
        save.clicked.connect(self.accept)
        footer.addWidget(cancel)
        footer.addWidget(save)
        layout.addLayout(footer)

        self._refresh_detected()
        self._on_mode_changed()

    def _refresh_detected(self) -> None:
        width, height = detect_primary_resolution()
        self._detected = (width, height)
        self.detected_label.setText(resolution_label(width, height))
        if self.auto_radio.isChecked():
            self._set_size(width, height)

    def _load(self, settings: AppSettings) -> None:
        if settings.is_auto():
            self.auto_radio.setChecked(True)
            width, height = detect_primary_resolution()
            self._set_size(width, height)
        else:
            self.manual_radio.setChecked(True)
            self._set_size(settings.screen_width, settings.screen_height)
        self._on_mode_changed()

    def _on_mode_changed(self) -> None:
        manual = self.manual_radio.isChecked()
        self.preset.setEnabled(manual)
        self.width_spin.setEnabled(manual)
        self.height_spin.setEnabled(manual)
        if not manual and hasattr(self, "_detected"):
            self._set_size(*self._detected)

    def _set_size(self, width: int, height: int) -> None:
        self.width_spin.blockSignals(True)
        self.height_spin.blockSignals(True)
        self.preset.blockSignals(True)
        self.width_spin.setValue(width)
        self.height_spin.setValue(height)
        index = 0
        for i in range(self.preset.count()):
            data = self.preset.itemData(i)
            if data == (width, height):
                index = i
                break
        self.preset.setCurrentIndex(index)
        self.width_spin.blockSignals(False)
        self.height_spin.blockSignals(False)
        self.preset.blockSignals(False)

    def _on_preset_changed(self, index: int) -> None:
        data = self.preset.itemData(index)
        if not data:
            return
        width, height = data
        self.width_spin.blockSignals(True)
        self.height_spin.blockSignals(True)
        self.width_spin.setValue(width)
        self.height_spin.setValue(height)
        self.width_spin.blockSignals(False)
        self.height_spin.blockSignals(False)

    def _sync_preset_from_spins(self) -> None:
        current = (self.width_spin.value(), self.height_spin.value())
        self.preset.blockSignals(True)
        index = 0
        for i in range(self.preset.count()):
            if self.preset.itemData(i) == current:
                index = i
                break
        self.preset.setCurrentIndex(index)
        self.preset.blockSignals(False)

    def result_settings(self) -> AppSettings:
        if self.auto_radio.isChecked():
            width, height = detect_primary_resolution()
            return AppSettings("auto", width, height)
        return AppSettings("manual", self.width_spin.value(), self.height_spin.value())
