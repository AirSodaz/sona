; Sona NSIS Installer & Uninstaller Hooks
; Integrates sona-shim with User PATH and manages Channel Registry lifecycle

!macro NSIS_HOOK_POSTINSTALL
  ; Register current installation and install universal shims to %USERPROFILE%\.sona\bin
  DetailPrint "Configuring Sona universal command-line shims..."
  IfFileExists "$INSTDIR\sona-shim.exe" 0 +3
    nsExec::ExecToLog '"$INSTDIR\sona-shim.exe" install --path "$INSTDIR"'
    Goto +2
  DetailPrint "Notice: sona-shim.exe not found in $INSTDIR"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; Safe uninstallation: decrements reference count in channel registry
  ; If another channel (Stable or Nightly) remains, shims and PATH are preserved
  DetailPrint "Unregistering Sona channel and updating shims..."
  IfFileExists "$INSTDIR\sona-shim.exe" 0 +3
    nsExec::ExecToLog '"$INSTDIR\sona-shim.exe" uninstall'
    Goto +2
  DetailPrint "Notice: sona-shim.exe not found during uninstall"
!macroend
