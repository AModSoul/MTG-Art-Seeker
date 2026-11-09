; Hooks into the NSIS installer Tauri builds (`bundle.windows.nsis.installerHooks`).

; The app was MTG Art Downloader up to 0.2.0. Tauri keys an NSIS install by the product name
; (its folder, its Start menu shortcut, its entry in Installed apps), so this version, installed
; over that one by hand or by the updater, would leave it in place beside itself. Once this one
; is in, the old one's uninstaller runs, silently. That leaves the settings and card data alone:
; they're kept under the bundle identifier, which hasn't changed, and a silent uninstall never
; deletes them. It stops the app only if it's running from the old folder, so never this one.
!define OLD_UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\MTG Art Downloader"

!macro NSIS_HOOK_POSTINSTALL
  Push $0
  Push $1
  Push $2
  ReadRegStr $0 SHCTX "${OLD_UNINSTKEY}" "UninstallString"
  ${If} $0 != ""
    ; The string is the uninstaller's path in quotes: strip them to find its folder.
    StrCpy $1 $0 "" 1
    StrCpy $1 $1 -1
    ${GetParent} $1 $2
    ; Run in place (`_?=`), as Tauri's installer runs an old version's, so this waits for it.
    ; It can't delete itself or its folder then, so that's done here.
    ExecWait '$0 /S _?=$2'
    Delete "$1"
    RMDir "$2"
  ${EndIf}
  Pop $2
  Pop $1
  Pop $0
!macroend
