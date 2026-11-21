; NSIS hooks of the 2.x Windows installer (tauri.conf.json -> bundle.windows.nsis.installerHooks).
;
; A MangoClient 1.3.x (Electron) install updates itself to 2.x by running this installer as
;   MangoClient_<v>_x64-setup.exe --updated /S --force-run
; (electron-updater, see tools/make-electron-feeds.js). The old app lives in a different folder than 2.x, so:
;   - before installing, its own uninstaller removes it silently (program files and its shortcuts only; the player's
;     data in %APPDATA%\.mangoclient is never part of an app folder and stays untouched),
;   - after installing, --force-run starts 2.x, as the 1.3.x updater expects.
; Only plain NSIS jumps are used here (no LogicLib); FileFunc.nsh (GetOptions) is included by the template.

!macro NSIS_HOOK_PREINSTALL
  IfFileExists "$LOCALAPPDATA\Programs\mangoclient\Uninstall MangoClient.exe" 0 mango_no_legacy_install
    ; _?= runs the uninstaller in place and waits for it (otherwise it copies itself to %TEMP% and returns at once)
    ExecWait '"$LOCALAPPDATA\Programs\mangoclient\Uninstall MangoClient.exe" /S _?=$LOCALAPPDATA\Programs\mangoclient'
    RMDir /r "$LOCALAPPDATA\Programs\mangoclient"
  mango_no_legacy_install:
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ClearErrors
  ${GetOptions} $CMDLINE "--force-run" $R9
  IfErrors mango_no_force_run 0
    Exec '"$INSTDIR\${MAINBINARYNAME}.exe"'
  mango_no_force_run:
!macroend
