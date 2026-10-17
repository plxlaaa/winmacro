; ============================================================
; WinMacro 安装器（NSIS 3.x / Unicode）
; 构建方式（仓库根目录）：
;   makensis -DVERSION=1.1.0 -DPAYLOAD=build/app/WinMacro installer/winmacro.nsi
; 产物：dist/WinMacro-Setup-<VERSION>.exe
;
; 特性：
;   - 双击运行，选择安装路径（默认 Program Files\WinMacro，升级时记住旧路径）
;   - 所有文件都安装在所选目录内（不散落其他位置）
;   - 同目录生成 Uninstall.exe（卸载器），双击即进入卸载流程
;   - 可选桌面 / 开始菜单快捷方式，注册到「设置-应用」列表
;   - 卸载时询问是否连用户录制数据（%USERPROFILE%\.winmacro）一并删除
; ============================================================
Unicode true

!define APPNAME "WinMacro"
!define APPExeName "WinMacro.exe"
!define APPUninstName "Uninstall.exe"
!define APPPublisher "plxlaaa"
!define APPWebSite "https://github.com/plxlaaa/winmacro"

!ifndef VERSION
  !define VERSION "1.1.0"
!endif
!ifndef PAYLOAD
  !define PAYLOAD "build\app\WinMacro"
!endif
!ifndef OUTDIR
  !define OUTDIR "dist"
!endif

Name "${APPNAME} ${VERSION}"
BrandingText "${APPNAME} ${VERSION} 安装程序"
OutFile "${OUTDIR}\${APPNAME}-Setup-${VERSION}.exe"
InstallDir "$PROGRAMFILES64\${APPNAME}"
InstallDirRegKey HKLM "Software\${APPNAME}" "InstallDir"
RequestExecutionLevel admin
SetCompressor /SOLID lzma
ShowUninstDetails show

!include "MUI2.nsh"
!include "FileFunc.nsh"
!include "LogicLib.nsh"

; ---------- 界面 ----------
!define MUI_ABORTWARNING
!define MUI_ICON "winmacro.ico"
!define MUI_UNICON "winmacro.ico"
!define MUI_FINISHPAGE_RUN "$INSTDIR\${APPExeName}"
!define MUI_FINISHPAGE_RUN_TEXT "立即运行 ${APPNAME}"
!define MUI_FINISHPAGE_TITLE_3LINES

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "SimpChinese"

; ---------- 安装 ----------
Section "${APPNAME} 主程序（必需）" SecCore
  SectionIn RO
  SetOutPath "$INSTDIR"
  File /r "${PAYLOAD}\*"
  WriteUninstaller "$INSTDIR\${APPUninstName}"

  WriteRegStr HKLM "Software\${APPNAME}" "InstallDir" "$INSTDIR"
  WriteRegStr HKLM "Software\${APPNAME}" "Version" "${VERSION}"

  ; 注册到「设置 - 应用 - 已安装的应用」
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}" "DisplayName" "${APPNAME}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}" "DisplayIcon" "$INSTDIR\${APPExeName}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}" "Publisher" "${APPPublisher}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}" "URLInfoAbout" "${APPWebSite}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}" "UninstallString" "$\"$INSTDIR\${APPUninstName}$\""
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}" "NoModify" 1
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}" "NoRepair" 1

  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "%d" $0
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}" "EstimatedSize" "$0"
SectionEnd

Section "桌面快捷方式" SecDesktop
  SetOutPath "$INSTDIR"
  CreateShortcut "$DESKTOP\${APPNAME}.lnk" "$INSTDIR\${APPExeName}" "" "$INSTDIR\${APPExeName}" 0
SectionEnd

Section "开始菜单快捷方式" SecStartMenu
  SetOutPath "$INSTDIR"
  CreateDirectory "$SMPROGRAMS\${APPNAME}"
  CreateShortcut "$SMPROGRAMS\${APPNAME}\${APPNAME}.lnk" "$INSTDIR\${APPExeName}" "" "$INSTDIR\${APPExeName}" 0
  CreateShortcut "$SMPROGRAMS\${APPNAME}\卸载 ${APPNAME}.lnk" "$INSTDIR\${APPUninstName}" "" "$INSTDIR\${APPUninstName}" 0
SectionEnd

; ---------- 卸载 ----------
Section "Uninstall"
  ; 先问是否连录制数据一起删（默认保留）
  IfFileExists "$PROFILE\.winmacro\*.*" 0 skip_data
    MessageBox MB_YESNO|MB_DEFBUTTON2|MB_ICONQUESTION "是否连录制数据一起删除？$\n$\n数据目录：$PROFILE\.winmacro$\n保留它，将来重装后录制仍可用。" /SD IDNO IDYES del_data
    Goto skip_data
  del_data:
    RMDir /r "$PROFILE\.winmacro"
  skip_data:

  Delete "$DESKTOP\${APPNAME}.lnk"
  RMDir /r "$SMPROGRAMS\${APPNAME}"

  RMDir /r "$INSTDIR"
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}"
  DeleteRegKey HKLM "Software\${APPNAME}"
SectionEnd
