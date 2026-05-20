Unicode true
RequestExecutionLevel user

!ifndef APP_VERSION
  !define APP_VERSION "dev"
!endif

!ifndef APP_NUMERIC_VERSION
  !define APP_NUMERIC_VERSION "0.0.0.0"
!endif

!ifndef OUT_FILE
  !define OUT_FILE "MarkRush-setup.exe"
!endif

!ifndef PAYLOAD_DIR
  !error "PAYLOAD_DIR must be provided"
!endif

Name "MarkRush"
OutFile "${OUT_FILE}"
InstallDir "$LOCALAPPDATA\Programs\MarkRush"
InstallDirRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MarkRush" "InstallLocation"
Icon "..\assets\icons\MarkRush.ico"
UninstallIcon "..\assets\icons\MarkRush.ico"

VIProductVersion "${APP_NUMERIC_VERSION}"
VIAddVersionKey "ProductName" "MarkRush"
VIAddVersionKey "CompanyName" "flashosophy"
VIAddVersionKey "LegalCopyright" "MIT"
VIAddVersionKey "FileDescription" "MarkRush Installer"
VIAddVersionKey "FileVersion" "${APP_VERSION}"
VIAddVersionKey "ProductVersion" "${APP_VERSION}"

Page directory
Page instfiles
UninstPage uninstConfirm
UninstPage instfiles

Section "Install"
  SetShellVarContext current

  SetOutPath "$INSTDIR"
  File /oname=markrush.exe "${PAYLOAD_DIR}\markrush.exe"
  File /oname=README.md "${PAYLOAD_DIR}\README.md"
  File /oname=LICENSE "${PAYLOAD_DIR}\LICENSE"

  SetOutPath "$INSTDIR\font-licenses"
  File /oname=README.md "${PAYLOAD_DIR}\font-licenses-README.md"
  File "${PAYLOAD_DIR}\OFL-Lora.txt"
  File "${PAYLOAD_DIR}\OFL-NotoSans.txt"
  File "${PAYLOAD_DIR}\OFL-NotoSerif.txt"
  File "${PAYLOAD_DIR}\OFL-Poppins.txt"
  File "${PAYLOAD_DIR}\OFL-RobotoMono.txt"

  CreateShortcut "$SMPROGRAMS\MarkRush.lnk" "$INSTDIR\markrush.exe" "" "$INSTDIR\markrush.exe" 0

  WriteUninstaller "$INSTDIR\Uninstall-MarkRush.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MarkRush" "DisplayName" "MarkRush"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MarkRush" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MarkRush" "Publisher" "flashosophy"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MarkRush" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MarkRush" "DisplayIcon" "$INSTDIR\markrush.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MarkRush" "UninstallString" "$INSTDIR\Uninstall-MarkRush.exe"
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MarkRush" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MarkRush" "NoRepair" 1
SectionEnd

Section "Uninstall"
  SetShellVarContext current

  Delete "$SMPROGRAMS\MarkRush.lnk"
  Delete "$INSTDIR\markrush.exe"
  Delete "$INSTDIR\README.md"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\font-licenses\README.md"
  Delete "$INSTDIR\font-licenses\OFL-Lora.txt"
  Delete "$INSTDIR\font-licenses\OFL-NotoSans.txt"
  Delete "$INSTDIR\font-licenses\OFL-NotoSerif.txt"
  Delete "$INSTDIR\font-licenses\OFL-Poppins.txt"
  Delete "$INSTDIR\font-licenses\OFL-RobotoMono.txt"
  RMDir "$INSTDIR\font-licenses"
  Delete "$INSTDIR\Uninstall-MarkRush.exe"
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MarkRush"
SectionEnd
