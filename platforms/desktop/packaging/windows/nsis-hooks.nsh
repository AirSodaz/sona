; Sona NSIS Installer & Uninstaller Hooks
; Integrates sona-shim with User PATH and manages Channel Registry lifecycle

!macro NSIS_HOOK_POSTINSTALL
  ; Register current installation and install universal shims to %USERPROFILE%\.sona\bin
  DetailPrint "Configuring Sona universal command-line shims..."
  ${If} ${FileExists} "$INSTDIR\sona-shim.exe"
    nsExec::ExecToLog '"$INSTDIR\sona-shim.exe" install --path "$INSTDIR\\"'
    Pop $0
  ${Else}
    DetailPrint "Notice: sona-shim.exe not found in $INSTDIR"
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; Safe uninstallation: decrements reference count in channel registry
  ; If another channel (Stable or Nightly) remains, shims and PATH are preserved
  DetailPrint "Unregistering Sona channel and updating shims..."
  ${If} ${FileExists} "$INSTDIR\sona-shim.exe"
    nsExec::ExecToLog '"$INSTDIR\sona-shim.exe" uninstall --path "$INSTDIR\\"'
    Pop $0
  ${Else}
    DetailPrint "Notice: sona-shim.exe not found during uninstall"
  ${EndIf}
!macroend
