; Orphyn installer / uninstaller: every page is custom and drawn in the app's style.
;
; The stock MUI chrome (header, grey button row) is hidden and the outer window is resized, painted
; navy and faded in. Each page is a full-window bitmap rendered by
; build/scripts/generate-installer-art.mjs (Segoe UI Variable text, glass logo, rounded pills), with
; small overlays on top: hover pills, unchecked checkbox rows and a breathing logo. Clicks land on
; the background bitmap and are dispatched on the zones generated next to the artwork.
; electron-updater runs the installer with /S: NSIS never creates a page in silent mode, so
; updates stay invisible.

!include "nsDialogs.nsh"
!include "LogicLib.nsh"
!include "WinMessages.nsh"
!ifndef BUILD_UNINSTALLER
  !include "StrContains.nsh"
!endif
!include "${BUILD_RESOURCES_DIR}\installer-art\installer-assets.nsh"

!define ORPHYN_BG "0B0F1C"
!define ORPHYN_TEXT "F5F5F7"
!define ORPHYN_WELL "161B2F"

!ifdef BUILD_UNINSTALLER
  !define LP_WELCOME unwelcome
  !define LP_PROGRESS unprogress
  !define LP_FINISH unfinish
!else
  !define LP_WELCOME welcome
  !define LP_PROGRESS progress
  !define LP_FINISH finish
!endif

; Static bitmap overlay at a zone of the artwork (px at scale 100, scaled to the bucket).
; style: 0x5000000E visible, 0x4000000E hidden. Clobbers $0-$5.
!macro ORPHYN_OVERLAY parent file zone style outvar
  IntOp $0 ${LZ_${zone}_X} * $OrphynBucket
  IntOp $0 $0 / 100
  IntOp $1 ${LZ_${zone}_Y} * $OrphynBucket
  IntOp $1 $1 / 100
  IntOp $2 ${LZ_${zone}_W} * $OrphynBucket
  IntOp $2 $2 / 100
  IntOp $3 ${LZ_${zone}_H} * $OrphynBucket
  IntOp $3 $3 / 100
  System::Call 'user32::CreateWindowEx(i 0, t "STATIC", t "", i ${style}, i r0, i r1, i r2, i r3, p ${parent}, p 0, p 0, p 0)p .r4'
  System::Call 'user32::LoadImage(p 0, t "$PLUGINSDIR\${file}.bmp", i 0, i 0, i 0, i 0x10)p .r5'
  SendMessage $4 0x172 0 $5
  StrCpy ${outvar} $4
!macroend

; $2 / $3 (cursor, px at scale 100) inside a zone?
!macro ORPHYN_IF_IN zone
  ${If} $2 >= ${LZ_${zone}_X1}
  ${AndIf} $2 <= ${LZ_${zone}_X2}
  ${AndIf} $3 >= ${LZ_${zone}_Y1}
  ${AndIf} $3 <= ${LZ_${zone}_Y2}
!macroend

; Show the hover overlay while the cursor is in the zone.
!macro ORPHYN_HOVER zone hwnd
  !insertmacro ORPHYN_IF_IN ${zone}
    ShowWindow ${hwnd} ${SW_SHOW}
  ${Else}
    ShowWindow ${hwnd} ${SW_HIDE}
  ${EndIf}
!macroend

!macro ORPHYN_HIDE id
  GetDlgItem $9 $HWNDPARENT ${id}
  ShowWindow $9 ${SW_HIDE}
!macroend

; Window, dialog and background of a page.
!macro ORPHYN_BEGIN un page
  ${If} ${Silent}
    Abort
  ${EndIf}
  Call ${un}OrphynFrame
  nsDialogs::Create 1018
  Pop $OrphynDialog
  ${If} $OrphynDialog == error
    Abort
  ${EndIf}
  SetCtlColors $OrphynDialog "" ${ORPHYN_BG}
  InitPluginsDir
  !insertmacro ORPHYN_FILES_${page}
  System::Call 'user32::LoadImage(p 0, t "$PLUGINSDIR\bg.bmp", i 0, i 0, i 0, i 0x10)p .r5'
  ${NSD_CreateBitmap} 0 0 100% 100% ""
  Pop $OrphynBg
  SendMessage $OrphynBg 0x172 0 $5
  ${NSD_AddStyle} $OrphynBg 0x100
  ${NSD_OnClick} $OrphynBg ${un}OrphynBgClick
  StrCpy $OrphynLogo 0
  StrCpy $OrphynFrameNo 0
  StrCpy $OrphynTicks 0
!macroend

; The breathing logo overlay (12 frames swapped by the timer).
!macro ORPHYN_LOGO
  !insertmacro ORPHYN_FILES_logo
  !insertmacro ORPHYN_OVERLAY $OrphynDialog logo0 GFRAME 0x5000000E $OrphynLogo
!macroend

; Background below the overlays (so it never repaints over them), timer, show.
!macro ORPHYN_END un
  System::Call 'user32::SetWindowPos(p $OrphynBg, p 1, i 0, i 0, i 0, i 0, i 0x13)'
  GetFunctionAddress $0 ${un}OrphynTick
  nsDialogs::CreateTimer $0 40
  nsDialogs::Show
!macroend

!macro ORPHYN_CODE un
  Var /GLOBAL OrphynFr
  Var /GLOBAL OrphynScale
  Var /GLOBAL OrphynBucket
  Var /GLOBAL OrphynKey
  Var /GLOBAL OrphynDialog
  Var /GLOBAL OrphynBg
  Var /GLOBAL OrphynPage
  Var /GLOBAL OrphynH1
  Var /GLOBAL OrphynLogo
  Var /GLOBAL OrphynFrameNo
  Var /GLOBAL OrphynTicks
  Var /GLOBAL OrphynFaded
  Var /GLOBAL OrphynAlpha
  !ifndef BUILD_UNINSTALLER
    Var /GLOBAL OrphynH2
    Var /GLOBAL OrphynR0
    Var /GLOBAL OrphynR1
    Var /GLOBAL OrphynR2
    Var /GLOBAL OrphynDirEdit
    Var /GLOBAL OrphynWantOptions
    Var /GLOBAL OrphynDesktop
    Var /GLOBAL OrphynMenu
    Var /GLOBAL OrphynLaunch
  !endif

  ; Display scale -> artwork bucket (100 / 125 / 150 / 200), language.
  Function ${un}OrphynInit
    System::Call 'user32::GetDpiForSystem()i .r1'
    ${If} $1 < 96
      StrCpy $1 96
    ${EndIf}
    IntOp $OrphynScale $1 * 100
    IntOp $OrphynScale $OrphynScale / 96
    ${If} $OrphynScale < 113
      StrCpy $OrphynBucket 100
    ${ElseIf} $OrphynScale < 138
      StrCpy $OrphynBucket 125
    ${ElseIf} $OrphynScale < 175
      StrCpy $OrphynBucket 150
    ${Else}
      StrCpy $OrphynBucket 200
    ${EndIf}
    System::Call 'kernel32::GetUserDefaultUILanguage()i .r1'
    IntOp $1 $1 & 0xFF
    ${If} $1 == 12
      StrCpy $OrphynFr 1
      StrCpy $OrphynKey "fr$OrphynBucket"
    ${Else}
      StrCpy $OrphynFr 0
      StrCpy $OrphynKey "en$OrphynBucket"
    ${EndIf}
    StrCpy $OrphynFaded 0
    StrCpy $OrphynAlpha 255
  !ifndef BUILD_UNINSTALLER
    StrCpy $OrphynWantOptions 0
    StrCpy $OrphynDesktop 1
    StrCpy $OrphynMenu 1
    StrCpy $OrphynLaunch 1
  !endif
  FunctionEnd

  ; Sizes, centres and paints the outer window, hides the stock chrome, sizes the inner dialog.
  Function ${un}OrphynFrame
    IntOp $1 ${ORPHYN_W} * $OrphynBucket
    IntOp $1 $1 / 100
    IntOp $2 ${ORPHYN_H} * $OrphynBucket
    IntOp $2 $2 / 100
    ; Two passes: the first one reveals the frame size, the second gives the exact client size.
    System::Call 'user32::SetWindowPos(p $HWNDPARENT, p 0, i 0, i 0, i $1, i $2, i 0x16)'
    System::Alloc 16
    Pop $3
    System::Call 'user32::GetClientRect(p $HWNDPARENT, p r3)'
    System::Call '*$3(i, i, i .r4, i .r5)'
    IntOp $4 $1 - $4
    IntOp $4 $1 + $4
    IntOp $5 $2 - $5
    IntOp $5 $2 + $5
    System::Call 'user32::GetSystemMetrics(i 0)i .r6'
    System::Call 'user32::GetSystemMetrics(i 1)i .r7'
    IntOp $6 $6 - $4
    IntOp $6 $6 / 2
    IntOp $7 $7 - $5
    IntOp $7 $7 / 2
    System::Call 'user32::SetWindowPos(p $HWNDPARENT, p 0, i $6, i $7, i $4, i $5, i 0x14)'
    SetCtlColors $HWNDPARENT "" ${ORPHYN_BG}
    !insertmacro ORPHYN_HIDE 1034
    !insertmacro ORPHYN_HIDE 1035
    !insertmacro ORPHYN_HIDE 1036
    !insertmacro ORPHYN_HIDE 1037
    !insertmacro ORPHYN_HIDE 1038
    !insertmacro ORPHYN_HIDE 1039
    !insertmacro ORPHYN_HIDE 1045
    !insertmacro ORPHYN_HIDE 1046
    !insertmacro ORPHYN_HIDE 1028
    !insertmacro ORPHYN_HIDE 1256
    !insertmacro ORPHYN_HIDE 1
    !insertmacro ORPHYN_HIDE 2
    !insertmacro ORPHYN_HIDE 3
    ; The inner dialog covers the whole client area.
    System::Call 'user32::GetClientRect(p $HWNDPARENT, p r3)'
    System::Call '*$3(i, i, i .r4, i .r5)'
    System::Free $3
    GetDlgItem $9 $HWNDPARENT 1018
    System::Call 'user32::SetWindowPos(p $9, p 0, i 0, i 0, i $4, i $5, i 0x14)'
    ; First page: start transparent, the timer fades the window in.
    ${If} $OrphynFaded == 0
      StrCpy $OrphynFaded 1
      StrCpy $OrphynAlpha 0
      System::Call 'user32::GetWindowLong(p $HWNDPARENT, i -20)i .r1'
      IntOp $1 $1 | 0x80000
      System::Call 'user32::SetWindowLong(p $HWNDPARENT, i -20, i r1)'
      System::Call 'user32::SetLayeredWindowAttributes(p $HWNDPARENT, i 0, i 0, i 2)'
    ${EndIf}
  FunctionEnd

  ; Cursor position in px at scale 100 relative to the window: $2 = x, $3 = y.
  Function ${un}OrphynCursor
    System::Alloc 16
    Pop $1
    System::Call 'user32::GetCursorPos(p r1)'
    System::Call 'user32::ScreenToClient(p $OrphynDialog, p r1)'
    System::Call '*$1(i .r2, i .r3)'
    System::Free $1
    IntOp $2 $2 * 100
    IntOp $2 $2 / $OrphynBucket
    IntOp $3 $3 * 100
    IntOp $3 $3 / $OrphynBucket
  FunctionEnd

  ; 40 ms timer: fade-in, hover overlays, logo animation.
  Function ${un}OrphynTick
    ${If} $OrphynAlpha < 255
      IntOp $OrphynAlpha $OrphynAlpha + 45
      ${If} $OrphynAlpha > 255
        StrCpy $OrphynAlpha 255
      ${EndIf}
      System::Call 'user32::SetLayeredWindowAttributes(p $HWNDPARENT, i 0, i $OrphynAlpha, i 2)'
    ${EndIf}
    Call ${un}OrphynCursor
    ${If} $OrphynPage == welcome
    ${OrIf} $OrphynPage == finish
      !insertmacro ORPHYN_HOVER PILL $OrphynH1
  !ifndef BUILD_UNINSTALLER
    ${ElseIf} $OrphynPage == options
      !insertmacro ORPHYN_HOVER BROWSE $OrphynH1
      !insertmacro ORPHYN_HOVER INSTALL $OrphynH2
  !endif
    ${EndIf}
    ${If} $OrphynLogo != 0
      IntOp $OrphynTicks $OrphynTicks + 1
      ${If} $OrphynTicks >= 4
        StrCpy $OrphynTicks 0
        IntOp $OrphynFrameNo $OrphynFrameNo + 1
        ${If} $OrphynFrameNo >= ${ORPHYN_FRAMES}
          StrCpy $OrphynFrameNo 0
        ${EndIf}
        System::Call 'user32::LoadImage(p 0, t "$PLUGINSDIR\logo$OrphynFrameNo.bmp", i 0, i 0, i 0, i 0x10)p .r5'
        SendMessage $OrphynLogo 0x172 0 $5 $6
        System::Call 'gdi32::DeleteObject(p r6)'
      ${EndIf}
    ${EndIf}
  FunctionEnd

  !ifdef BUILD_UNINSTALLER
    Function un.OrphynSkipPage
      Abort
    FunctionEnd
    Function un.OrphynOnCancel
      SendMessage $HWNDPARENT ${WM_CLOSE} 0 0
    FunctionEnd
  !else
    Function OrphynOnBack
      SendMessage $HWNDPARENT 0x408 -1 0
    FunctionEnd
    Function OrphynOnOptions
      StrCpy $OrphynWantOptions 1
      SendMessage $HWNDPARENT 0x408 1 0
    FunctionEnd
    Function OrphynOnLaunch
      ${StdUtils.ExecShellAsUser} $0 "$launchLink" "open" ""
      SendMessage $HWNDPARENT 0x408 1 0
    FunctionEnd
    Function OrphynOnBrowse
      nsDialogs::SelectFolderDialog "Orphyn" "$INSTDIR"
      Pop $0
      ${If} $0 != error
        System::Call 'user32::SetWindowText(p $OrphynDirEdit, t "$0")'
      ${EndIf}
    FunctionEnd
  !endif
  Function ${un}OrphynOnNext
    SendMessage $HWNDPARENT 0x408 1 0
  FunctionEnd

  ; Click on the background: the pill / link / row under the cursor.
  Function ${un}OrphynBgClick
    Call ${un}OrphynCursor
    ${If} $OrphynPage == welcome
    ${OrIf} $OrphynPage == finish
      !insertmacro ORPHYN_IF_IN PILL
  !ifndef BUILD_UNINSTALLER
        ${If} $OrphynPage == finish
        ${AndIf} $OrphynLaunch == 1
          Call OrphynOnLaunch
        ${Else}
          Call OrphynOnNext
        ${EndIf}
  !else
        Call un.OrphynOnNext
  !endif
        Return
      ${EndIf}
      ${If} $OrphynPage == welcome
        !insertmacro ORPHYN_IF_IN LINK
  !ifndef BUILD_UNINSTALLER
          Call OrphynOnOptions
  !else
          Call un.OrphynOnCancel
  !endif
          Return
        ${EndIf}
      ${EndIf}
  !ifndef BUILD_UNINSTALLER
    ${ElseIf} $OrphynPage == options
      !insertmacro ORPHYN_IF_IN BROWSE
        Call OrphynOnBrowse
        Return
      ${EndIf}
      !insertmacro ORPHYN_IF_IN INSTALL
        Call OrphynOnNext
        Return
      ${EndIf}
      !insertmacro ORPHYN_IF_IN BACK
        Call OrphynOnBack
        Return
      ${EndIf}
      !insertmacro ORPHYN_IF_IN ROW0
        ${If} $OrphynDesktop == 1
          StrCpy $OrphynDesktop 0
          ShowWindow $OrphynR0 ${SW_SHOW}
        ${Else}
          StrCpy $OrphynDesktop 1
          ShowWindow $OrphynR0 ${SW_HIDE}
        ${EndIf}
        Return
      ${EndIf}
      !insertmacro ORPHYN_IF_IN ROW1
        ${If} $OrphynMenu == 1
          StrCpy $OrphynMenu 0
          ShowWindow $OrphynR1 ${SW_SHOW}
        ${Else}
          StrCpy $OrphynMenu 1
          ShowWindow $OrphynR1 ${SW_HIDE}
        ${EndIf}
        Return
      ${EndIf}
      !insertmacro ORPHYN_IF_IN ROW2
        ${If} $OrphynLaunch == 1
          StrCpy $OrphynLaunch 0
          ShowWindow $OrphynR2 ${SW_SHOW}
        ${Else}
          StrCpy $OrphynLaunch 1
          ShowWindow $OrphynR2 ${SW_HIDE}
        ${EndIf}
        Return
      ${EndIf}
  !endif
    ${EndIf}
  FunctionEnd

  ; ---- welcome ----
  Function ${un}OrphynWelcomeShow
    StrCpy $OrphynPage welcome
  !ifndef BUILD_UNINSTALLER
    StrCpy $OrphynWantOptions 0
  !endif
    !insertmacro ORPHYN_BEGIN "${un}" ${LP_WELCOME}
    !insertmacro ORPHYN_OVERLAY $OrphynDialog pillh GPILL 0x4000000E $OrphynH1
    !insertmacro ORPHYN_LOGO
    !insertmacro ORPHYN_END "${un}"
  FunctionEnd
  Function ${un}OrphynWelcomeLeave
    GetFunctionAddress $0 ${un}OrphynTick
    nsDialogs::KillTimer $0
  FunctionEnd

  ; ---- finish ----
  Function ${un}OrphynFinishShow
    StrCpy $OrphynPage finish
    !insertmacro ORPHYN_BEGIN "${un}" ${LP_FINISH}
  !ifndef BUILD_UNINSTALLER
    ; Launch pill when "launch at the end" is on, otherwise a plain Done pill.
    ${If} $OrphynLaunch == 1
      !insertmacro ORPHYN_OVERLAY $OrphynDialog launch GPILL 0x5000000E $0
      !insertmacro ORPHYN_OVERLAY $OrphynDialog launchh GPILL 0x4000000E $OrphynH1
    ${Else}
      !insertmacro ORPHYN_OVERLAY $OrphynDialog done GPILL 0x5000000E $0
      !insertmacro ORPHYN_OVERLAY $OrphynDialog doneh GPILL 0x4000000E $OrphynH1
    ${EndIf}
  !else
    !insertmacro ORPHYN_OVERLAY $OrphynDialog pillh GPILL 0x4000000E $OrphynH1
  !endif
    !insertmacro ORPHYN_LOGO
    !insertmacro ORPHYN_END "${un}"
  FunctionEnd
  Function ${un}OrphynFinishLeave
    GetFunctionAddress $0 ${un}OrphynTick
    nsDialogs::KillTimer $0
  FunctionEnd

  ; ---- progress (the stock instfiles page, repainted: no log, thin bar, same background) ----
  Function ${un}OrphynProgressShow
    SetAutoClose true
    StrCpy $OrphynPage progress
    Call ${un}OrphynFrame
    FindWindow $R0 "#32770" "" $HWNDPARENT
    SetCtlColors $R0 "" ${ORPHYN_BG}
    System::Alloc 16
    Pop $3
    System::Call 'user32::GetClientRect(p $HWNDPARENT, p r3)'
    System::Call '*$3(i, i, i .r4, i .r5)'
    System::Free $3
    System::Call 'user32::SetWindowPos(p $R0, p 0, i 0, i 0, i $4, i $5, i 0x14)'
    GetDlgItem $R1 $R0 1006
    ShowWindow $R1 ${SW_HIDE}
    GetDlgItem $R1 $R0 1016
    ShowWindow $R1 ${SW_HIDE}
    GetDlgItem $R1 $R0 1027
    ShowWindow $R1 ${SW_HIDE}
    InitPluginsDir
    !insertmacro ORPHYN_FILES_${LP_PROGRESS}
    System::Call 'user32::CreateWindowEx(i 0, t "STATIC", t "", i 0x5000000E, i 0, i 0, i $4, i $5, p $R0, p 0, p 0, p 0)p .r6'
    System::Call 'user32::LoadImage(p 0, t "$PLUGINSDIR\bg.bmp", i 0, i 0, i 0, i 0x10)p .r7'
    SendMessage $6 0x172 0 $7
    ; Thin bar: accent on a dark track, centred under the title.
    GetDlgItem $R1 $R0 1004
    IntOp $1 ${LZ_BAR_X1} * $OrphynBucket
    IntOp $1 $1 / 100
    IntOp $2 ${LZ_BAR_X2} - ${LZ_BAR_X1}
    IntOp $2 $2 * $OrphynBucket
    IntOp $2 $2 / 100
    IntOp $3 ${LZ_BAR_Y1} * $OrphynBucket
    IntOp $3 $3 / 100
    IntOp $R2 ${LZ_BAR_Y2} - ${LZ_BAR_Y1}
    IntOp $R2 $R2 * $OrphynBucket
    IntOp $R2 $R2 / 100
    System::Call 'uxtheme::SetWindowTheme(p $R1, w "", w "")'
    System::Call 'user32::SetWindowLong(p $R1, i -16, i 0x50000001)'
    System::Call 'user32::SetWindowLong(p $R1, i -20, i 0)'
    SendMessage $R1 0x409 0 0xF06C7C
    SendMessage $R1 0x2001 0 0x36201A
    System::Call 'user32::SetWindowPos(p $R1, p 0, i $1, i $3, i $2, i $R2, i 0x40)'
  FunctionEnd
!macroend

!ifndef BUILD_UNINSTALLER
  ; ---- options page: folder, shortcuts, launch at the end ----
  !macro ORPHYN_OPTIONS
    Function OrphynOptionsShow
      ${If} $OrphynWantOptions != 1
        Abort
      ${EndIf}
      StrCpy $OrphynPage options
      !insertmacro ORPHYN_BEGIN "" options
      !insertmacro ORPHYN_OVERLAY $OrphynDialog browseh GBROWSE 0x4000000E $OrphynH1
      !insertmacro ORPHYN_OVERLAY $OrphynDialog installh GINSTALL 0x4000000E $OrphynH2
      !insertmacro ORPHYN_OVERLAY $OrphynDialog off0 GROW0 0x4000000E $OrphynR0
      !insertmacro ORPHYN_OVERLAY $OrphynDialog off1 GROW1 0x4000000E $OrphynR1
      !insertmacro ORPHYN_OVERLAY $OrphynDialog off2 GROW2 0x4000000E $OrphynR2
      ; Rows switched off earlier (coming back from the welcome page).
      ${If} $OrphynDesktop == 0
        ShowWindow $OrphynR0 ${SW_SHOW}
      ${EndIf}
      ${If} $OrphynMenu == 0
        ShowWindow $OrphynR1 ${SW_SHOW}
      ${EndIf}
      ${If} $OrphynLaunch == 0
        ShowWindow $OrphynR2 ${SW_SHOW}
      ${EndIf}
      ; Folder field: a borderless edit inside the well drawn in the artwork.
      IntOp $0 ${LZ_WELL_X1} + 12
      IntOp $0 $0 * $OrphynBucket
      IntOp $0 $0 / 100
      IntOp $1 ${LZ_WELL_Y1} + 8
      IntOp $1 $1 * $OrphynBucket
      IntOp $1 $1 / 100
      IntOp $2 ${LZ_WELL_X2} - ${LZ_WELL_X1}
      IntOp $2 $2 - 24
      IntOp $2 $2 * $OrphynBucket
      IntOp $2 $2 / 100
      IntOp $3 ${LZ_WELL_Y2} - ${LZ_WELL_Y1}
      IntOp $3 $3 - 16
      IntOp $3 $3 * $OrphynBucket
      IntOp $3 $3 / 100
      System::Call 'user32::CreateWindowEx(i 0, t "EDIT", t "$INSTDIR", i 0x50010080, i r0, i r1, i r2, i r3, p $OrphynDialog, p 0, p 0, p 0)p .r4'
      StrCpy $OrphynDirEdit $4
      IntOp $5 17 * $OrphynBucket
      IntOp $5 $5 / 100
      StrCpy $6 "Segoe UI"
      ${If} ${FileExists} "$WINDIR\Fonts\SegUIVar.ttf"
        StrCpy $6 "Segoe UI Variable Text"
      ${EndIf}
      System::Call 'gdi32::CreateFont(i -r5, i 0, i 0, i 0, i 400, i 0, i 0, i 0, i 1, i 0, i 0, i 5, i 0, t "$6")p .r7'
      SendMessage $OrphynDirEdit ${WM_SETFONT} $7 1
      SetCtlColors $OrphynDirEdit "${ORPHYN_TEXT}" "${ORPHYN_WELL}"
      !insertmacro ORPHYN_END ""
    FunctionEnd
    Function OrphynOptionsLeave
      GetFunctionAddress $0 OrphynTick
      nsDialogs::KillTimer $0
      System::Call 'user32::GetWindowText(p $OrphynDirEdit, t .r0, i 1024)'
      ; Like the stock directory page: the folder always ends in a "Orphyn" sub-folder.
      ${StrContains} $1 "${APP_FILENAME}" "$0"
      ${If} $1 == ""
        StrCpy $0 "$0\${APP_FILENAME}"
      ${EndIf}
      StrCpy $INSTDIR $0
    FunctionEnd
  !macroend

  !macro customHeader
    ManifestDPIAware true
    !insertmacro ORPHYN_CODE ""
    !insertmacro ORPHYN_OPTIONS
  !macroend
  ; Orphyn is per-user only: never show the stock install-mode (all users / current user) page.
  !macro customInstallMode
    StrCpy $isForceCurrentInstall "1"
  !macroend
  !macro customInit
    Call OrphynInit
  !macroend
  !macro customWelcomePage
    Page custom OrphynWelcomeShow OrphynWelcomeLeave
    Page custom OrphynOptionsShow OrphynOptionsLeave
  !macroend
  !macro customPageAfterChangeDir
    !define MUI_PAGE_CUSTOMFUNCTION_SHOW OrphynProgressShow
  !macroend
  !macro customFinishPage
    Page custom OrphynFinishShow OrphynFinishLeave
  !macroend
  ; The installer always creates both shortcuts: drop the ones the options page switched off.
  !macro customInstall
    ; Upgrading from Lumen (same appId): drop its old shortcuts, the Orphyn ones replace them.
    Delete "$DESKTOP\Lumen.lnk"
    Delete "$SMPROGRAMS\Lumen.lnk"
    ${IfNot} ${Silent}
      ${If} $OrphynDesktop == 0
        Delete "$newDesktopLink"
      ${EndIf}
      ${If} $OrphynMenu == 0
        Delete "$newStartMenuLink"
        StrCpy $launchLink "$INSTDIR\${APP_EXECUTABLE_FILENAME}"
      ${EndIf}
    ${EndIf}
  !macroend
!else
  !macro customHeader
    ManifestDPIAware true
    !insertmacro ORPHYN_CODE "un."
  !macroend
  !macro customInstallMode
    StrCpy $isForceCurrentInstall "1"
  !macroend
  !macro customUnInstall
    Delete "$DESKTOP\Lumen.lnk"
    Delete "$SMPROGRAMS\Lumen.lnk"
  !macroend
  !macro customuninit
    Call un.OrphynInit
  !macroend
  !macro customUnWelcomePage
    UninstPage custom un.OrphynWelcomeShow un.OrphynWelcomeLeave
    !define MUI_PAGE_CUSTOMFUNCTION_SHOW un.OrphynProgressShow
  !macroend
  !macro customUninstallPage
    !define MUI_PAGE_CUSTOMFUNCTION_PRE un.OrphynSkipPage
    UninstPage custom un.OrphynFinishShow un.OrphynFinishLeave
  !macroend
!endif
