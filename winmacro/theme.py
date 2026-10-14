DARK_QSS = """
QWidget {
    background: #0f1419;
    color: #e7edf4;
    font-family: "Segoe UI", "Microsoft YaHei UI", "PingFang SC", sans-serif;
    font-size: 13px;
}
QMainWindow, QDialog {
    background: #0f1419;
}
QFrame#Sidebar {
    background: #121820;
    border-right: 1px solid #1f2a36;
}
QFrame#Card, QFrame#Hero, QFrame#StatCard {
    background: #161d26;
    border: 1px solid #243140;
    border-radius: 14px;
}
QLabel#Brand {
    color: #f4f7fb;
    font-size: 20px;
    font-weight: 700;
    letter-spacing: 0.4px;
}
QLabel#Muted {
    color: #8ea0b5;
}
QLabel#Caption {
    color: #6f8298;
    font-size: 11px;
    letter-spacing: 1.2px;
    font-weight: 600;
}
QLabel#StatValue {
    color: #f4f7fb;
    font-size: 22px;
    font-weight: 700;
}
QLabel#Title {
    color: #f4f7fb;
    font-size: 18px;
    font-weight: 650;
}
QPushButton {
    background: #1c2733;
    color: #e7edf4;
    border: 1px solid #2b3b4d;
    border-radius: 10px;
    padding: 8px 14px;
    font-weight: 600;
}
QPushButton:hover {
    background: #243242;
    border-color: #3d536b;
}
QPushButton:pressed {
    background: #18222c;
}
QPushButton:disabled {
    color: #5d6f83;
    background: #151c24;
}
QPushButton#Primary {
    background: #2f6fed;
    border: 1px solid #3d7dff;
    color: #ffffff;
}
QPushButton#Primary:hover {
    background: #3b7cff;
}
QPushButton#Danger {
    background: #c23b4a;
    border: 1px solid #e14b5c;
    color: #ffffff;
}
QPushButton#Danger:hover {
    background: #d44757;
}
QPushButton#Ghost {
    background: transparent;
    border: 1px solid #2b3b4d;
}
QRadioButton {
    spacing: 8px;
    color: #e7edf4;
}
QRadioButton::indicator {
    width: 16px;
    height: 16px;
    border-radius: 8px;
    border: 1px solid #3d536b;
    background: #10161d;
}
QRadioButton::indicator:checked {
    background: #2f6fed;
    border: 1px solid #3d7dff;
}
QComboBox QAbstractItemView {
    background: #121820;
    color: #e7edf4;
    selection-background-color: #1d2e48;
    border: 1px solid #2b3b4d;
}
QLineEdit, QComboBox, QSpinBox {
    background: #10161d;
    border: 1px solid #2b3b4d;
    border-radius: 8px;
    padding: 7px 10px;
    color: #e7edf4;
    selection-background-color: #2f6fed;
}
QLineEdit:focus, QComboBox:focus {
    border: 1px solid #3d7dff;
}
QListWidget, QTableWidget, QTreeWidget {
    background: #121820;
    border: 1px solid #243140;
    border-radius: 12px;
    outline: none;
    alternate-background-color: #151d26;
}
QListWidget::item {
    padding: 10px 12px;
    border-bottom: 1px solid #1d2834;
}
QListWidget::item:selected {
    background: #1d2e48;
    color: #ffffff;
}
QListWidget::item:hover {
    background: #1a2430;
}
QHeaderView::section {
    background: #151d26;
    color: #8ea0b5;
    border: none;
    border-bottom: 1px solid #243140;
    padding: 8px 10px;
    font-weight: 600;
}
QTableWidget::item {
    padding: 6px 8px;
}
QTableWidget::item:selected {
    background: #1d2e48;
}
QScrollBar:vertical {
    background: transparent;
    width: 10px;
    margin: 4px;
}
QScrollBar::handle:vertical {
    background: #2b3b4d;
    border-radius: 5px;
    min-height: 28px;
}
QScrollBar::add-line:vertical, QScrollBar::sub-line:vertical {
    height: 0;
}
QStatusBar {
    background: #0f1419;
    color: #8ea0b5;
    border-top: 1px solid #1f2a36;
}
QSplitter::handle {
    background: #1f2a36;
}
QToolTip {
    background: #1c2733;
    color: #e7edf4;
    border: 1px solid #2b3b4d;
    padding: 6px 8px;
}
"""

OVERLAY_QSS = """
QWidget#OverlayRoot {
    background: transparent;
}
QFrame#OverlayCard {
    background: rgba(16, 22, 30, 230);
    border: 1px solid rgba(61, 125, 255, 180);
    border-radius: 16px;
}
QLabel#OverlayTitle {
    color: #f4f7fb;
    font-size: 13px;
    font-weight: 700;
    letter-spacing: 0.6px;
}
QLabel#OverlayHint {
    color: #8ea0b5;
    font-size: 11px;
}
QLabel#RecDot {
    color: #e14b5c;
    font-size: 14px;
    font-weight: 800;
}
QPushButton#RecButton {
    background: #2f6fed;
    color: white;
    border: none;
    border-radius: 10px;
    padding: 8px 16px;
    font-weight: 700;
    min-width: 108px;
}
QPushButton#RecButton:hover {
    background: #3b7cff;
}
QPushButton#RecButton[recording="true"] {
    background: #c23b4a;
}
QPushButton#RecButton[recording="true"]:hover {
    background: #d44757;
}
QPushButton#IconBtn {
    background: transparent;
    color: #8ea0b5;
    border: none;
    border-radius: 8px;
    min-width: 28px;
    max-width: 28px;
    min-height: 28px;
}
QPushButton#IconBtn:hover {
    background: rgba(255,255,255,0.08);
    color: #e7edf4;
}
"""
