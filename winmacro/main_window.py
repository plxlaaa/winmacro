from __future__ import annotations

from pathlib import Path

from PySide6.QtCore import QObject, QSize, QThread, Qt, QTimer, Signal, Slot
from PySide6.QtGui import QAction, QCloseEvent, QKeySequence
from PySide6.QtWidgets import (
    QComboBox,
    QFileDialog,
    QFormLayout,
    QFrame,
    QHBoxLayout,
    QLabel,
    QListWidget,
    QListWidgetItem,
    QMainWindow,
    QMessageBox,
    QPushButton,
    QSpinBox,
    QStatusBar,
    QVBoxLayout,
    QWidget,
)

from winmacro.backends.factory import create_backend
from winmacro.geometry import inflate_rect
from winmacro.models import Recording, strip_stop_click
from winmacro.screen import (
    canvas_to_screen,
    capture_to_canvas,
    detect_screen_info,
    resolution_label,
)
from winmacro.storage import (
    ensure_unique_id,
    export_recording,
    import_recording,
    load_recordings,
    load_settings,
    save_recordings,
    save_settings,
)
from winmacro.theme import DARK_QSS
from winmacro.timing import interruptible_sleep
from winmacro.widgets.details import RecordingDetailsDialog
from winmacro.widgets.overlay import ScreenOverlay
from winmacro.widgets.settings_dialog import SettingsDialog


class EventBridge(QObject):
    captured = Signal()


class PlaybackThread(QThread):
    failed = Signal(str)
    finished_ok = Signal()
    progress = Signal(int, int, float)
    countdown = Signal(int)

    def __init__(
        self,
        backend,
        events,
        speed: float = 1.0,
        repeat: int = 1,
        countdown_s: int = 0,
    ) -> None:
        super().__init__()
        self._backend = backend
        self._events = events
        self._speed = float(speed)
        self._repeat = max(1, int(repeat))
        self._countdown = max(0, int(countdown_s))
        self._abort = False

    def abort(self) -> None:
        self._abort = True
        self._backend.abort_playback()

    def run(self) -> None:
        try:
            for remaining in range(self._countdown, 0, -1):
                self.countdown.emit(remaining)
                if interruptible_sleep(1.0, lambda: self._abort) or self._abort:
                    return
            for index in range(self._repeat):
                if self._abort:
                    return
                if index and interruptible_sleep(1.0, lambda: self._abort):
                    return
                self.progress.emit(index + 1, self._repeat, self._speed)
                self._backend.play(
                    self._events,
                    abort_check=lambda: self._abort,
                    speed=self._speed,
                )
                if self._abort:
                    return
            self.finished_ok.emit()
        except Exception as exc:
            self.failed.emit(str(exc))


class MainWindow(QMainWindow):
    def __init__(self) -> None:
        super().__init__()
        self.setWindowTitle("WinMacro · 脚本制作与执行")
        self.resize(1080, 680)
        self.setMinimumSize(920, 560)
        self.setStyleSheet(DARK_QSS)

        self.backend, self.engine_name = create_backend()
        self.recordings: list[Recording] = load_recordings()
        self.settings = load_settings()
        self.overlay: ScreenOverlay | None = None
        self.play_thread: PlaybackThread | None = None
        self.details_dialog: RecordingDetailsDialog | None = None
        self._live_steps = 0
        self._capture_info = detect_screen_info()
        self._canvas_size = self.settings.canvas_size()
        self._bridge = EventBridge()
        self._bridge.captured.connect(self._on_live_event)
        self._stopping = False
        self._overlay_restore = False
        self._play_label = ""

        self._build()
        self._refresh_list()
        self._refresh_stats()
        self.statusBar().showMessage(f"引擎：{self.engine_name}")

    def _build(self) -> None:
        root = QWidget()
        self.setCentralWidget(root)
        layout = QHBoxLayout(root)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(0)

        sidebar = QFrame()
        sidebar.setObjectName("Sidebar")
        sidebar.setFixedWidth(250)
        side = QVBoxLayout(sidebar)
        side.setContentsMargins(22, 24, 22, 22)
        side.setSpacing(10)

        brand = QLabel("WinMacro")
        brand.setObjectName("Brand")
        tag = QLabel("脚本制作 / 全屏录制 / 回放")
        tag.setObjectName("Muted")
        engine = QLabel(self.engine_name)
        engine.setObjectName("Caption")
        engine.setWordWrap(True)

        add_btn = QPushButton("添加屏幕组件")
        add_btn.setObjectName("Primary")
        add_btn.clicked.connect(self.show_overlay)

        self.stop_overlay_btn = QPushButton("隐藏屏幕组件")
        self.stop_overlay_btn.setObjectName("Ghost")
        self.stop_overlay_btn.clicked.connect(self.hide_overlay)

        import_btn = QPushButton("导入脚本")
        import_btn.setObjectName("Ghost")
        import_btn.clicked.connect(self.import_script)

        settings_btn = QPushButton("设置")
        settings_btn.setObjectName("Ghost")
        settings_btn.clicked.connect(self.open_settings)

        self.resolution_label = QLabel()
        self.resolution_label.setObjectName("Muted")
        self.resolution_label.setWordWrap(True)
        self._refresh_resolution_status()

        playback_title = QLabel("回放选项")
        playback_title.setObjectName("Caption")

        self.speed_combo = QComboBox()
        for label, value in (
            ("0.5 倍速", 0.5),
            ("原速", 1.0),
            ("2 倍速", 2.0),
            ("5 倍速", 5.0),
            ("立即执行", 0.0),
        ):
            self.speed_combo.addItem(label, value)
        self.speed_combo.setCurrentIndex(1)

        self.repeat_spin = QSpinBox()
        self.repeat_spin.setRange(1, 999)
        self.repeat_spin.setValue(1)
        self.repeat_spin.setSuffix(" 次")

        self.countdown_spin = QSpinBox()
        self.countdown_spin.setRange(0, 60)
        self.countdown_spin.setValue(3)
        self.countdown_spin.setSuffix(" 秒")

        playback_form = QFormLayout()
        playback_form.setContentsMargins(0, 0, 0, 0)
        playback_form.setSpacing(8)
        playback_form.addRow("速度", self.speed_combo)
        playback_form.addRow("循环", self.repeat_spin)
        playback_form.addRow("倒计时", self.countdown_spin)

        side.addWidget(brand)
        side.addWidget(tag)
        side.addSpacing(8)
        side.addWidget(engine)
        side.addSpacing(18)
        side.addWidget(add_btn)
        side.addWidget(self.stop_overlay_btn)
        side.addWidget(import_btn)
        side.addWidget(settings_btn)
        side.addSpacing(8)
        side.addWidget(self.resolution_label)
        side.addSpacing(14)
        side.addWidget(playback_title)
        side.addLayout(playback_form)
        side.addStretch()
        hint = QLabel("点击「开始录制」后，以整个屏幕为范围记录键盘与鼠标。结束录制的点击不会写入脚本。")
        hint.setObjectName("Muted")
        hint.setWordWrap(True)
        side.addWidget(hint)

        main = QWidget()
        body = QVBoxLayout(main)
        body.setContentsMargins(24, 22, 24, 18)
        body.setSpacing(14)

        hero = QFrame()
        hero.setObjectName("Hero")
        hero_l = QVBoxLayout(hero)
        hero_l.setContentsMargins(18, 16, 18, 16)
        hero_title = QLabel("操作录制库")
        hero_title.setObjectName("Title")
        hero_sub = QLabel("在屏幕组件上开始录制。完成后会出现在下方列表，点击即可查看每一步的类型、时间和坐标。")
        hero_sub.setObjectName("Muted")
        hero_sub.setWordWrap(True)
        stats = QHBoxLayout()
        self.stat_count = self._stat_card("脚本数量", "0")
        self.stat_steps = self._stat_card("最近步数", "—")
        self.stat_dur = self._stat_card("最近时长", "—")
        stats.addWidget(self.stat_count)
        stats.addWidget(self.stat_steps)
        stats.addWidget(self.stat_dur)
        hero_l.addWidget(hero_title)
        hero_l.addWidget(hero_sub)
        hero_l.addLayout(stats)

        list_card = QFrame()
        list_card.setObjectName("Card")
        list_l = QVBoxLayout(list_card)
        list_l.setContentsMargins(14, 12, 14, 14)
        head = QHBoxLayout()
        head_title = QLabel("录制列表")
        head_title.setObjectName("Caption")
        self.play_btn = QPushButton("执行选中")
        self.play_btn.setObjectName("Primary")
        self.play_btn.clicked.connect(self.play_selected)
        self.detail_btn = QPushButton("查看详情")
        self.detail_btn.clicked.connect(self.open_details)
        self.export_btn = QPushButton("导出")
        self.export_btn.clicked.connect(self.export_selected)
        self.rename_btn = QPushButton("重命名")
        self.rename_btn.clicked.connect(self.rename_selected)
        self.delete_btn = QPushButton("删除")
        self.delete_btn.setObjectName("Danger")
        self.delete_btn.clicked.connect(self.delete_selected)
        head.addWidget(head_title)
        head.addStretch()
        head.addWidget(self.play_btn)
        head.addWidget(self.detail_btn)
        head.addWidget(self.export_btn)
        head.addWidget(self.rename_btn)
        head.addWidget(self.delete_btn)

        self.list_widget = QListWidget()
        self.list_widget.itemClicked.connect(self.open_details)
        self.list_widget.setAlternatingRowColors(True)

        empty = QLabel("还没有录制。添加屏幕组件并点击「开始录制」。")
        empty.setObjectName("Muted")
        empty.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.empty_label = empty

        list_l.addLayout(head)
        list_l.addWidget(self.list_widget, 1)
        list_l.addWidget(self.empty_label)

        body.addWidget(hero)
        body.addWidget(list_card, 1)

        layout.addWidget(sidebar)
        layout.addWidget(main, 1)

        status = QStatusBar()
        self.setStatusBar(status)

        stop_play = QAction("停止回放", self)
        stop_play.setShortcut(QKeySequence("Escape"))
        stop_play.setShortcutContext(Qt.ShortcutContext.ApplicationShortcut)
        stop_play.triggered.connect(self.stop_playback)
        self.addAction(stop_play)

        self._tick = QTimer(self)
        self._tick.setInterval(250)
        self._tick.timeout.connect(self._on_tick)

    def _stat_card(self, caption: str, value: str) -> QFrame:
        card = QFrame()
        card.setObjectName("StatCard")
        box = QVBoxLayout(card)
        box.setContentsMargins(14, 12, 14, 12)
        cap = QLabel(caption.upper())
        cap.setObjectName("Caption")
        val = QLabel(value)
        val.setObjectName("StatValue")
        box.addWidget(cap)
        box.addWidget(val)
        card.value_label = val  # type: ignore[attr-defined]
        return card

    def _refresh_list(self, select_id: str | None = None) -> None:
        current_id = select_id
        if current_id is None:
            item = self.list_widget.currentItem()
            if item is not None:
                current_id = item.data(Qt.ItemDataRole.UserRole)
        self.list_widget.blockSignals(True)
        self.list_widget.clear()
        target: QListWidgetItem | None = None
        for rec in reversed(self.recordings):
            item = QListWidgetItem(
                f"{rec.name}\n{rec.created_at}    {rec.step_count()} 步    {rec.duration_text()}    {rec.resolution_text()}"
            )
            item.setData(Qt.ItemDataRole.UserRole, rec.id)
            item.setSizeHint(QSize(0, 56))
            self.list_widget.addItem(item)
            if rec.id == current_id:
                target = item
        empty = len(self.recordings) == 0
        self.empty_label.setVisible(empty)
        self.list_widget.setVisible(not empty)
        if target is not None:
            self.list_widget.setCurrentItem(target)
        self.list_widget.blockSignals(False)

    def _refresh_stats(self) -> None:
        self.stat_count.value_label.setText(str(len(self.recordings)))  # type: ignore[attr-defined]
        if self.recordings:
            last = self.recordings[-1]
            self.stat_steps.value_label.setText(str(last.step_count()))  # type: ignore[attr-defined]
            self.stat_dur.value_label.setText(last.duration_text())  # type: ignore[attr-defined]
        else:
            self.stat_steps.value_label.setText("—")  # type: ignore[attr-defined]
            self.stat_dur.value_label.setText("—")  # type: ignore[attr-defined]

    def _selected_recording(self) -> Recording | None:
        item = self.list_widget.currentItem()
        if item is None:
            return None
        rec_id = item.data(Qt.ItemDataRole.UserRole)
        for rec in self.recordings:
            if rec.id == rec_id:
                return rec
        return None

    def show_overlay(self) -> None:
        if self.overlay is None:
            self.overlay = ScreenOverlay()
            self.overlay.record_toggled.connect(self._on_record_toggled)
            self.overlay.closed.connect(lambda: self.statusBar().showMessage("屏幕组件已隐藏"))
            self.overlay.moved_or_resized.connect(self._sync_ignore_rect)
            screen = self.screen().availableGeometry()
            self.overlay.move(screen.x() + screen.width() - 320, screen.y() + 48)
        self.overlay.show()
        self.overlay.raise_()
        self._sync_ignore_rect()
        self.statusBar().showMessage("屏幕组件已显示，可拖动调整位置")

    def hide_overlay(self) -> None:
        if self.overlay:
            if self.overlay.is_recording():
                self._stop_recording()
            self.overlay.hide()

    def _sync_ignore_rect(self) -> None:
        if not self.overlay:
            return
        x, y, w, h = inflate_rect(self.overlay.overlay_rect(), 8)
        self.backend.set_ignore_rect(x, y, w, h)

    def _on_record_toggled(self, active: bool) -> None:
        if active:
            self._start_recording()
        else:
            self._stop_recording()

    def _refresh_resolution_status(self) -> None:
        canvas_w, canvas_h = self.settings.canvas_size()
        if self.settings.is_auto():
            text = f"录制分辨率  自动识别\n{resolution_label(canvas_w, canvas_h)}"
        else:
            text = f"录制分辨率  手动设置\n{resolution_label(canvas_w, canvas_h)}"
        self.resolution_label.setText(text)

    def open_settings(self) -> None:
        dialog = SettingsDialog(self.settings, self)
        if dialog.exec() != SettingsDialog.DialogCode.Accepted:
            return
        self.settings = dialog.result_settings()
        try:
            save_settings(self.settings)
        except OSError as exc:
            QMessageBox.critical(self, "保存设置失败", str(exc))
            return
        self._refresh_resolution_status()
        self.statusBar().showMessage(
            f"已保存分辨率设置：{self.settings.canvas_size()[0]}×{self.settings.canvas_size()[1]}"
        )

    def _start_recording(self) -> None:
        if self.backend.is_recording():
            return
        if self.play_thread and self.play_thread.isRunning():
            if self.overlay:
                self.overlay.set_recording(False)
            QMessageBox.information(self, "正在执行", "请先停止回放（Esc），再开始录制。")
            return
        self._sync_ignore_rect()
        self._live_steps = 0
        self._capture_info = detect_screen_info()
        self._canvas_size = self.settings.canvas_size()
        try:
            self.backend.start(on_event=self._on_backend_event)
        except Exception as exc:
            QMessageBox.critical(self, "无法开始录制", str(exc))
            if self.overlay:
                self.overlay.set_recording(False)
            return
        self._tick.start()
        canvas_w, canvas_h = self._canvas_size
        self.statusBar().showMessage(f"正在全屏录制… 坐标空间 {canvas_w}×{canvas_h}")

    def _on_backend_event(self, _event) -> None:
        self._bridge.captured.emit()

    @Slot()
    def _on_live_event(self) -> None:
        count = self.backend.event_count()
        self._live_steps = count if count else self._live_steps + 1

    def _stop_recording(self) -> None:
        if self._stopping:
            return
        self._stopping = True
        try:
            self._tick.stop()
            overlay_rect = self.overlay.overlay_rect() if self.overlay else None
            button_rect = self.overlay.record_button_rect() if self.overlay else None
            try:
                raw = self.backend.stop()
            except Exception as exc:
                if self.overlay:
                    self.overlay.set_recording(False)
                QMessageBox.critical(self, "结束录制失败", str(exc))
                return
            events = strip_stop_click(raw, overlay_rect, button_rect)
            if self.overlay:
                self.overlay.set_recording(False)
            if not events:
                self.statusBar().showMessage("录制已结束，没有可保存的操作")
                return
            canvas_w, canvas_h = self._canvas_size
            events = capture_to_canvas(events, self._capture_info, canvas_w, canvas_h)
            recording = Recording.create(
                events,
                screen_width=canvas_w,
                screen_height=canvas_h,
                resolution_mode=self.settings.resolution_mode,
            )
            self.recordings.append(recording)
            if not self._persist():
                self.recordings.pop()
                self.statusBar().showMessage("保存失败，本次录制已丢弃")
                return
            self._refresh_list(select_id=recording.id)
            self._refresh_stats()
            self.statusBar().showMessage(
                f"已保存「{recording.name}」· {recording.step_count()} 步 · {recording.duration_text()}"
            )
        finally:
            self._stopping = False

    def _on_tick(self) -> None:
        if self.overlay and self.overlay.is_recording():
            self._sync_ignore_rect()
            count = self.backend.event_count()
            if count:
                self._live_steps = count
            self.statusBar().showMessage(f"录制中… 已捕获 {self._live_steps} 个事件")

    def _on_details_closed(self, _result: int = 0) -> None:
        self.details_dialog = None

    def open_details(self, _item: QListWidgetItem | None = None) -> None:
        rec = self._selected_recording()
        if rec is None:
            return
        if self.details_dialog is None:
            self.details_dialog = RecordingDetailsDialog(rec, self)
            self.details_dialog.finished.connect(self._on_details_closed)
        else:
            self.details_dialog.load(rec)
        self.details_dialog.show()
        self.details_dialog.raise_()
        self.details_dialog.activateWindow()

    def play_selected(self) -> None:
        rec = self._selected_recording()
        if rec is None:
            QMessageBox.information(self, "未选择", "请先在列表中选择一条录制。")
            return
        if self.play_thread and self.play_thread.isRunning():
            QMessageBox.information(self, "正在执行", "当前已有脚本在回放，按 Esc 停止。")
            return
        if self.overlay and self.overlay.is_recording():
            QMessageBox.information(self, "正在录制", "请先结束录制，再执行脚本。")
            return
        if not rec.events:
            QMessageBox.information(self, "空脚本", "这条录制没有可执行的操作。")
            return
        screen = detect_screen_info()
        src_w = rec.screen_width or screen.width
        src_h = rec.screen_height or screen.height
        events = canvas_to_screen(rec.events, src_w, src_h, screen)
        speed = float(self.speed_combo.currentData() or 1.0)
        repeat = int(self.repeat_spin.value())
        countdown = int(self.countdown_spin.value())
        self._play_label = rec.name
        self._hide_overlay_for_play()
        self.play_thread = PlaybackThread(self.backend, events, speed, repeat, countdown)
        self.play_thread.failed.connect(self._on_play_failed)
        self.play_thread.finished_ok.connect(self._on_play_finished_ok)
        self.play_thread.progress.connect(self._on_play_progress)
        self.play_thread.countdown.connect(self._on_play_countdown)
        self.play_thread.finished.connect(self._on_play_thread_done)
        self.play_thread.start()
        self.statusBar().showMessage(self._play_status(1, repeat, speed))

    def _play_status(self, current: int, total: int, speed: float) -> str:
        speed_text = "立即" if speed <= 0 else f"{speed:g}x"
        rounds = f" 第 {current}/{total} 轮" if total > 1 else ""
        return f"正在执行「{self._play_label}」{rounds} · {speed_text}… 按 Esc 停止"

    def _hide_overlay_for_play(self) -> None:
        self._overlay_restore = bool(self.overlay and self.overlay.isVisible())
        if self.overlay and self._overlay_restore:
            self.overlay.hide()

    def _restore_overlay(self) -> None:
        if self._overlay_restore and self.overlay is not None:
            self.overlay.show()
            self.overlay.raise_()
        self._overlay_restore = False

    def _on_play_countdown(self, remaining: int) -> None:
        self.statusBar().showMessage(f"回放将在 {remaining} 秒后开始，按 Esc 取消")

    def _on_play_progress(self, current: int, total: int, speed: float) -> None:
        self.statusBar().showMessage(self._play_status(current, total, speed))

    def _on_play_finished_ok(self) -> None:
        self.statusBar().showMessage(f"「{self._play_label}」回放完成")

    def _on_play_thread_done(self) -> None:
        self._restore_overlay()

    def _on_play_failed(self, msg: str) -> None:
        QMessageBox.critical(self, "回放失败", msg)

    def stop_playback(self) -> None:
        if self.play_thread and self.play_thread.isRunning():
            self.play_thread.abort()
            self.statusBar().showMessage("已请求停止回放")

    def _persist(self) -> bool:
        try:
            save_recordings(self.recordings)
            return True
        except OSError as exc:
            QMessageBox.critical(self, "保存失败", str(exc))
            return False

    def export_selected(self) -> None:
        rec = self._selected_recording()
        if rec is None:
            return
        path, _ = QFileDialog.getSaveFileName(
            self, "导出脚本", f"{rec.name}.json", "WinMacro 脚本 (*.json)"
        )
        if not path:
            return
        try:
            export_recording(rec, Path(path))
        except OSError as exc:
            QMessageBox.critical(self, "导出失败", str(exc))
            return
        self.statusBar().showMessage(f"已导出到 {path}")

    def import_script(self) -> None:
        path, _ = QFileDialog.getOpenFileName(self, "导入脚本", "", "WinMacro 脚本 (*.json)")
        if not path:
            return
        try:
            rec = import_recording(Path(path))
        except Exception as exc:
            QMessageBox.critical(self, "导入失败", str(exc))
            return
        rec = ensure_unique_id(rec, {item.id for item in self.recordings})
        self.recordings.append(rec)
        if not self._persist():
            self.recordings.pop()
            return
        self._refresh_list(select_id=rec.id)
        self._refresh_stats()

    def rename_selected(self) -> None:
        rec = self._selected_recording()
        if rec is None:
            return
        from PySide6.QtWidgets import QInputDialog

        name, ok = QInputDialog.getText(self, "重命名", "脚本名称", text=rec.name)
        if not ok or not name.strip():
            return
        rec.name = name.strip()
        if not self._persist():
            return
        self._refresh_list(select_id=rec.id)
        if self.details_dialog is not None:
            self.details_dialog.load(rec)

    def delete_selected(self) -> None:
        rec = self._selected_recording()
        if rec is None:
            return
        if QMessageBox.question(self, "删除脚本", f"删除「{rec.name}」？") != QMessageBox.StandardButton.Yes:
            return
        kept = self.recordings
        self.recordings = [item for item in self.recordings if item.id != rec.id]
        if not self._persist():
            self.recordings = kept
            return
        if self.details_dialog is not None:
            self.details_dialog.close()
        self._refresh_list()
        self._refresh_stats()

    def closeEvent(self, event: QCloseEvent) -> None:
        self.stop_playback()
        if self.play_thread and self.play_thread.isRunning():
            self.play_thread.wait(1500)
        if self.overlay and self.overlay.is_recording():
            self._stop_recording()
        if self.overlay:
            self.overlay.close()
        self.backend.shutdown()
        event.accept()
