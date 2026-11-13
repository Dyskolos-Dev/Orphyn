; Lumen installer / uninstaller: every page is custom and drawn in the app's style.
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

!define LUMEN_BG "0B0F1C"
!define LUMEN_TEXT "F5F5F7"
!define LUMEN_WELL "161B2F"

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
!macro LUMEN_OVERLAY parent file zone style outvar
  IntOp $0 ${LZ_${zone}_X} * $LumenBucket
  IntOp $0 $0 / 100
  IntOp $1 ${LZ_${zone}_Y} * $LumenBucket
  IntOp $1 $1 / 100
  IntOp $2 ${LZ_${zone}_W} * $LumenBucket
  IntOp $2 $2 / 100
  IntOp $3 ${LZ_${zone}_H} * $LumenBucket
  IntOp $3 $3 / 100
  System::Call 'user32::CreateWindowEx(i 0, t "STATIC", t "", i ${style}, i r0, i r1, i r2, i r3, p ${parent}, p 0, p 0, p 0)p .r4'
  System::Call 'user32::LoadImage(p 0, t "$PLUGINSDIR\${file}.bmp", i 0, i 0, i 0, i 0x10)p .r5'
  SendMessage $4 0x172 0 $5
  StrCpy ${outvar} $4
!macroend

; $2 / $3 (cursor, px at scale 100) inside a zone?
!macro LUMEN_IF_IN zone
  ${If} $2 >= ${LZ_${zone}_X1}
  ${AndIf} $2 <= ${LZ_${zone}_X2}
  ${AndIf} $3 >= ${LZ_${zone}_Y1}
  ${AndIf} $3 <= ${LZ_${zone}_Y2}
!macroend

; Show the hover overlay while the cursor is in the zone.
!macro LUMEN_HOVER zone hwnd
  !insertmacro LUMEN_IF_IN ${zone}
    ShowWindow ${hwnd} ${SW_SHOW}
  ${Else}
    ShowWindow ${hwnd} ${SW_HIDE}
  ${EndIf}
!macroend

!macro LUMEN_HIDE id
  GetDlgItem $9 $HWNDPARENT ${id}
  ShowWindow $9 ${SW_HIDE}
!macroend

; Window, dialog and background of a page.
!macro LUMEN_BEGIN un page
  ${If} ${Silent}
    Abort
  ${EndIf}
  Call ${un}LumenFrame
  nsDialogs::Create 1018
  Pop $LumenDialog
  ${If} $LumenDialog == error
    Abort
  ${EndIf}
  SetCtlColors $LumenDialog "" ${LUMEN_BG}
  InitPluginsDir
  !insertmacro LUMEN_FILES_${page}
  System::Call 'user32::LoadImage(p 0, t "$PLUGINSDIR\bg.bmp", i 0, i 0, i 0, i 0x10)p .r5'
  ${NSD_CreateBitmap} 0 0 100% 100% ""
  Pop $LumenBg
  SendMessage $LumenBg 0x172 0 $5
  ${NSD_AddStyle} $LumenBg 0x100
  ${NSD_OnClick} $LumenBg ${un}LumenBgClick
  StrCpy $LumenLogo 0
  StrCpy $LumenFrameNo 0
  StrCpy $LumenTicks 0
!macroend

; The breathing logo overlay (12 frames swapped by the timer).
!macro LUMEN_LOGO
  !insertmacro LUMEN_FILES_logo
  !insertmacro LUMEN_OVERLAY $LumenDialog logo0 GFRAME 0x5000000E $LumenLogo
!macroend

; Background below the overlays (so it never repaints over them), timer, show.
!macro LUMEN_END un
  System::Call 'user32::SetWindowPos(p $LumenBg, p 1, i 0, i 0, i 0, i 0, i 0x13)'
  GetFunctionAddress $0 ${un}LumenTick
  nsDialogs::CreateTimer $0 40
  nsDialogs::Show
!macroend

!macro LUMEN_CODE un
  Var /GLOBAL LumenFr
  Var /GLOBAL LumenScale
  Var /GLOBAL LumenBucket
  Var /GLOBAL LumenKey
  Var /GLOBAL LumenDialog
  Var /GLOBAL LumenBg
  Var /GLOBAL LumenPage
  Var /GLOBAL LumenH1
  Var /GLOBAL LumenLogo
  Var /GLOBAL LumenFrameNo
  Var /GLOBAL LumenTicks
  Var /GLOBAL LumenFaded
  Var /GLOBAL LumenAlpha
  !ifndef BUILD_UNINSTALLER
    Var /GLOBAL LumenH2
    Var /GLOBAL LumenR0
    Var /GLOBAL LumenR1
    Var /GLOBAL LumenR2
    Var /GLOBAL LumenDirEdit
    Var /GLOBAL LumenWantOptions
    Var /GLOBAL LumenDesktop
    Var /GLOBAL LumenMenu
    Var /GLOBAL LumenLaunch
  !endif

  ; Display scale -> artwork bucket (100 / 125 / 150 / 200), language.
  Function ${un}LumenInit
    System::Call 'user32::GetDpiForSystem()i .r1'
    ${If} $1 < 96
      StrCpy $1 96
    ${EndIf}
    IntOp $LumenScale $1 * 100
    IntOp $LumenScale $LumenScale / 96
    ${If} $LumenScale < 113
      StrCpy $LumenBucket 100
    ${ElseIf} $LumenScale < 138
      StrCpy $LumenBucket 125
    ${ElseIf} $LumenScale < 175
      StrCpy $LumenBucket 150
    ${Else}
      StrCpy $LumenBucket 200
    ${EndIf}
    System::Call 'kernel32::GetUserDefaultUILanguage()i .r1'
    IntOp $1 $1 & 0xFF
    ${If} $1 == 12
      StrCpy $LumenFr 1
      StrCpy $LumenKey "fr$LumenBucket"
    ${Else}
      StrCpy $LumenFr 0
      StrCpy $LumenKey "en$LumenBucket"
    ${EndIf}
    StrCpy $LumenFaded 0
    StrCpy $LumenAlpha 255
  !ifndef BUILD_UNINSTALLER
    StrCpy $LumenWantOptions 0
    StrCpy $LumenDesktop 1
    StrCpy $LumenMenu 1
    StrCpy $LumenLaunch 1
  !endif
  FunctionEnd

  ; Sizes, centres and paints the outer window, hides the stock chrome, sizes the inner dialog.
  Function ${un}LumenFrame
    IntOp $1 ${LUMEN_W} * $LumenBucket
    IntOp $1 $1 / 100
    IntOp $2 ${LUMEN_H} * $LumenBucket
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
    SetCtlColors $HWNDPARENT "" ${LUMEN_BG}
    !insertmacro LUMEN_HIDE 1034
    !insertmacro LUMEN_HIDE 1035
    !insertmacro LUMEN_HIDE 1036
    !insertmacro LUMEN_HIDE 1037
    !insertmacro LUMEN_HIDE 1038
    !insertmacro LUMEN_HIDE 1039
    !insertmacro LUMEN_HIDE 1045
    !insertmacro LUMEN_HIDE 1046
    !insertmacro LUMEN_HIDE 1028
    !insertmacro LUMEN_HIDE 1256
    !insertmacro LUMEN_HIDE 1
    !insertmacro LUMEN_HIDE 2
    !insertmacro LUMEN_HIDE 3
    ; The inner dialog covers the whole client area.
    System::Call 'user32::GetClientRect(p $HWNDPARENT, p r3)'
    System::Call '*$3(i, i, i .r4, i .r5)'
    System::Free $3
    GetDlgItem $9 $HWNDPARENT 1018
    System::Call 'user32::SetWindowPos(p $9, p 0, i 0, i 0, i $4, i $5, i 0x14)'
    ; First page: start transparent, the timer fades the window in.
    ${If} $LumenFaded == 0
      StrCpy $LumenFaded 1
      StrCpy $LumenAlpha 0
      System::Call 'user32::GetWindowLong(p $HWNDPARENT, i -20)i .r1'
      IntOp $1 $1 | 0x80000
      System::Call 'user32::SetWindowLong(p $HWNDPARENT, i -20, i r1)'
      System::Call 'user32::SetLayeredWindowAttributes(p $HWNDPARENT, i 0, i 0, i 2)'
    ${EndIf}
  FunctionEnd

  ; Cursor position in px at scale 100 relative to the window: $2 = x, $3 = y.
  Function ${un}LumenCursor
    System::Alloc 16
    Pop $1
    System::Call 'user32::GetCursorPos(p r1)'
    System::Call 'user32::ScreenToClient(p $LumenDialog, p r1)'
    System::Call '*$1(i .r2, i .r3)'
    System::Free $1
    IntOp $2 $2 * 100
    IntOp $2 $2 / $LumenBucket
    IntOp $3 $3 * 100
    IntOp $3 $3 / $LumenBucket
  FunctionEnd

  ; 40 ms timer: fade-in, hover overlays, logo animation.
  Function ${un}LumenTick
    ${If} $LumenAlpha < 255
      IntOp $LumenAlpha $LumenAlpha + 45
      ${If} $LumenAlpha > 255
        StrCpy $LumenAlpha 255
      ${EndIf}
      System::Call 'user32::SetLayeredWindowAttributes(p $HWNDPARENT, i 0, i $LumenAlpha, i 2)'
    ${EndIf}
    Call ${un}LumenCursor
    ${If} $LumenPage == welcome
    ${OrIf} $LumenPage == finish
      !insertmacro LUMEN_HOVER PILL $LumenH1
  !ifndef BUILD_UNINSTALLER
    ${ElseIf} $LumenPage == options
      !insertmacro LUMEN_HOVER BROWSE $LumenH1
      !insertmacro LUMEN_HOVER INSTALL $LumenH2
  !endif
    ${EndIf}
    ${If} $LumenLogo != 0
      IntOp $LumenTicks $LumenTicks + 1
      ${If} $LumenTicks >= 4
        StrCpy $LumenTicks 0
        IntOp $LumenFrameNo $LumenFrameNo + 1
        ${If} $LumenFrameNo >= ${LUMEN_FRAMES}
          StrCpy $LumenFrameNo 0
        ${EndIf}
        System::Call 'user32::LoadImage(p 0, t "$PLUGINSDIR\logo$LumenFrameNo.bmp", i 0, i 0, i 0, i 0x10)p .r5'
        SendMessage $LumenLogo 0x172 0 $5 $6
        System::Call 'gdi32::DeleteObject(p r6)'
      ${EndIf}
    ${EndIf}
  FunctionEnd

  !ifdef BUILD_UNINSTALLER
    Function un.LumenSkipPage
      Abort
    FunctionEnd
    Function un.LumenOnCancel
      SendMessage $HWNDPARENT ${WM_CLOSE} 0 0
    FunctionEnd
  !else
    Function LumenOnBack
      SendMessage $HWNDPARENT 0x408 -1 0
    FunctionEnd
    Function LumenOnOptions
      StrCpy $LumenWantOptions 1
      SendMessage $HWNDPARENT 0x408 1 0
    FunctionEnd
    Function LumenOnLaunch
      ${StdUtils.ExecShellAsUser} $0 "$launchLink" "open" ""
      SendMessage $HWNDPARENT 0x408 1 0
    FunctionEnd
    Function LumenOnBrowse
      nsDialogs::SelectFolderDialog "Lumen" "$INSTDIR"
      Pop $0
      ${If} $0 != error
        System::Call 'user32::SetWindowText(p $LumenDirEdit, t "$0")'
      ${EndIf}
    FunctionEnd
  !endif
  Function ${un}LumenOnNext
    SendMessage $HWNDPARENT 0x408 1 0
  FunctionEnd

  ; Click on the background: the pill / link / row under the cursor.
  Function ${un}LumenBgClick
    Call ${un}LumenCursor
    ${If} $LumenPage == welcome
    ${OrIf} $LumenPage == finish
      !insertmacro LUMEN_IF_IN PILL
  !ifndef BUILD_UNINSTALLER
        ${If} $LumenPage == finish
        ${AndIf} $LumenLaunch == 1
          Call LumenOnLaunch
        ${Else}
          Call LumenOnNext
        ${EndIf}
  !else
        Call un.LumenOnNext
  !endif
        Return
      ${EndIf}
      ${If} $LumenPage == welcome
        !insertmacro LUMEN_IF_IN LINK
  !ifndef BUILD_UNINSTALLER
          Call LumenOnOptions
  !else
          Call un.LumenOnCancel
  !endif
          Return
        ${EndIf}
      ${EndIf}
  !ifndef BUILD_UNINSTALLER
    ${ElseIf} $LumenPage == options
      !insertmacro LUMEN_IF_IN BROWSE
        Call LumenOnBrowse
        Return
      ${EndIf}
      !insertmacro LUMEN_IF_IN INSTALL
        Call LumenOnNext
        Return
      ${EndIf}
      !insertmacro LUMEN_IF_IN BACK
        Call LumenOnBack
        Return
      ${EndIf}
      !insertmacro LUMEN_IF_IN ROW0
        ${If} $LumenDesktop == 1
          StrCpy $LumenDesktop 0
          ShowWindow $LumenR0 ${SW_SHOW}
        ${Else}
          StrCpy $LumenDesktop 1
          ShowWindow $LumenR0 ${SW_HIDE}
        ${EndIf}
        Return
      ${EndIf}
      !insertmacro LUMEN_IF_IN ROW1
        ${If} $LumenMenu == 1
          StrCpy $LumenMenu 0
          ShowWindow $LumenR1 ${SW_SHOW}
        ${Else}
          StrCpy $LumenMenu 1
          ShowWindow $LumenR1 ${SW_HIDE}
        ${EndIf}
        Return
      ${EndIf}
      !insertmacro LUMEN_IF_IN ROW2
        ${If} $LumenLaunch == 1
          StrCpy $LumenLaunch 0
          ShowWindow $LumenR2 ${SW_SHOW}
        ${Else}
          StrCpy $LumenLaunch 1
          ShowWindow $LumenR2 ${SW_HIDE}
        ${EndIf}
        Return
      ${EndIf}
  !endif
    ${EndIf}
  FunctionEnd

  ; ---- welcome ----
  Function ${un}LumenWelcomeShow
    StrCpy $LumenPage welcome
  !ifndef BUILD_UNINSTALLER
    StrCpy $LumenWantOptions 0
  !endif
    !insertmacro LUMEN_BEGIN "${un}" ${LP_WELCOME}
    !insertmacro LUMEN_OVERLAY $LumenDialog pillh GPILL 0x4000000E $LumenH1
    !insertmacro LUMEN_LOGO
    !insertmacro LUMEN_END "${un}"
  FunctionEnd
  Function ${un}LumenWelcomeLeave
    GetFunctionAddress $0 ${un}LumenTick
    nsDialogs::KillTimer $0
  FunctionEnd

  ; ---- finish ----
  Function ${un}LumenFinishShow
    StrCpy $LumenPage finish
    !insertmacro LUMEN_BEGIN "${un}" ${LP_FINISH}
  !ifndef BUILD_UNINSTALLER
    ; Launch pill when "launch at the end" is on, otherwise a plain Done pill.
    ${If} $LumenLaunch == 1
      !insertmacro LUMEN_OVERLAY $LumenDialog launch GPILL 0x5000000E $0
      !insertmacro LUMEN_OVERLAY $LumenDialog launchh GPILL 0x4000000E $LumenH1
    ${Else}
      !insertmacro LUMEN_OVERLAY $LumenDialog done GPILL 0x5000000E $0
      !insertmacro LUMEN_OVERLAY $LumenDialog doneh GPILL 0x4000000E $LumenH1
    ${EndIf}
  !else
    !insertmacro LUMEN_OVERLAY $LumenDialog pillh GPILL 0x4000000E $LumenH1
  !endif
    !insertmacro LUMEN_LOGO
    !insertmacro LUMEN_END "${un}"
  FunctionEnd
  Function ${un}LumenFinishLeave
    GetFunctionAddress $0 ${un}LumenTick
    nsDialogs::KillTimer $0
  FunctionEnd

  ; ---- progress (the stock instfiles page, repainted: no log, thin bar, same background) ----
  Function ${un}LumenProgressShow
    SetAutoClose true
    StrCpy $LumenPage progress
    Call ${un}LumenFrame
    FindWindow $R0 "#32770" "" $HWNDPARENT
    SetCtlColors $R0 "" ${LUMEN_BG}
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
    !insertmacro LUMEN_FILES_${LP_PROGRESS}
    System::Call 'user32::CreateWindowEx(i 0, t "STATIC", t "", i 0x5000000E, i 0, i 0, i $4, i $5, p $R0, p 0, p 0, p 0)p .r6'
    System::Call 'user32::LoadImage(p 0, t "$PLUGINSDIR\bg.bmp", i 0, i 0, i 0, i 0x10)p .r7'
    SendMessage $6 0x172 0 $7
    ; Thin bar: accent on a dark track, centred under the title.
    GetDlgItem $R1 $R0 1004
    IntOp $1 ${LZ_BAR_X1} * $LumenBucket
    IntOp $1 $1 / 100
    IntOp $2 ${LZ_BAR_X2} - ${LZ_BAR_X1}
    IntOp $2 $2 * $LumenBucket
    IntOp $2 $2 / 100
    IntOp $3 ${LZ_BAR_Y1} * $LumenBucket
    IntOp $3 $3 / 100
    IntOp $R2 ${LZ_BAR_Y2} - ${LZ_BAR_Y1}
    IntOp $R2 $R2 * $LumenBucket
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
  !macro LUMEN_OPTIONS
    Function LumenOptionsShow
      ${If} $LumenWantOptions != 1
        Abort
      ${EndIf}
      StrCpy $LumenPage options
      !insertmacro LUMEN_BEGIN "" options
      !insertmacro LUMEN_OVERLAY $LumenDialog browseh GBROWSE 0x4000000E $LumenH1
      !insertmacro LUMEN_OVERLAY $LumenDialog installh GINSTALL 0x4000000E $LumenH2
      !insertmacro LUMEN_OVERLAY $LumenDialog off0 GROW0 0x4000000E $LumenR0
      !insertmacro LUMEN_OVERLAY $LumenDialog off1 GROW1 0x4000000E $LumenR1
      !insertmacro LUMEN_OVERLAY $LumenDialog off2 GROW2 0x4000000E $LumenR2
      ; Rows switched off earlier (coming back from the welcome page).
      ${If} $LumenDesktop == 0
        ShowWindow $LumenR0 ${SW_SHOW}
      ${EndIf}
      ${If} $LumenMenu == 0
        ShowWindow $LumenR1 ${SW_SHOW}
      ${EndIf}
      ${If} $LumenLaunch == 0
        ShowWindow $LumenR2 ${SW_SHOW}
      ${EndIf}
      ; Folder field: a borderless edit inside the well drawn in the artwork.
      IntOp $0 ${LZ_WELL_X1} + 12
      IntOp $0 $0 * $LumenBucket
      IntOp $0 $0 / 100
      IntOp $1 ${LZ_WELL_Y1} + 8
      IntOp $1 $1 * $LumenBucket
      IntOp $1 $1 / 100
      IntOp $2 ${LZ_WELL_X2} - ${LZ_WELL_X1}
      IntOp $2 $2 - 24
      IntOp $2 $2 * $LumenBucket
      IntOp $2 $2 / 100
      IntOp $3 ${LZ_WELL_Y2} - ${LZ_WELL_Y1}
      IntOp $3 $3 - 16
      IntOp $3 $3 * $LumenBucket
      IntOp $3 $3 / 100
      System::Call 'user32::CreateWindowEx(i 0, t "EDIT", t "$INSTDIR", i 0x50010080, i r0, i r1, i r2, i r3, p $LumenDialog, p 0, p 0, p 0)p .r4'
      StrCpy $LumenDirEdit $4
      IntOp $5 17 * $LumenBucket
      IntOp $5 $5 / 100
      StrCpy $6 "Segoe UI"
      ${If} ${FileExists} "$WINDIR\Fonts\SegUIVar.ttf"
        StrCpy $6 "Segoe UI Variable Text"
      ${EndIf}
      System::Call 'gdi32::CreateFont(i -r5, i 0, i 0, i 0, i 400, i 0, i 0, i 0, i 1, i 0, i 0, i 5, i 0, t "$6")p .r7'
      SendMessage $LumenDirEdit ${WM_SETFONT} $7 1
      SetCtlColors $LumenDirEdit "${LUMEN_TEXT}" "${LUMEN_WELL}"
      !insertmacro LUMEN_END ""
    FunctionEnd
    Function LumenOptionsLeave
      GetFunctionAddress $0 LumenTick
      nsDialogs::KillTimer $0
      System::Call 'user32::GetWindowText(p $LumenDirEdit, t .r0, i 1024)'
      ; Like the stock directory page: the folder always ends in a "Lumen" sub-folder.
      ${StrContains} $1 "${APP_FILENAME}" "$0"
      ${If} $1 == ""
        StrCpy $0 "$0\${APP_FILENAME}"
      ${EndIf}
      StrCpy $INSTDIR $0
    FunctionEnd
  !macroend

  !macro customHeader
    ManifestDPIAware true
    !insertmacro LUMEN_CODE ""
    !insertmacro LUMEN_OPTIONS
  !macroend
  ; Lumen is per-user only: never show the stock install-mode (all users / current user) page.
  !macro customInstallMode
    StrCpy $isForceCurrentInstall "1"
  !macroend
  !macro customInit
    Call LumenInit
  !macroend
  !macro customWelcomePage
    Page custom LumenWelcomeShow LumenWelcomeLeave
    Page custom LumenOptionsShow LumenOptionsLeave
  !macroend
  !macro customPageAfterChangeDir
    !define MUI_PAGE_CUSTOMFUNCTION_SHOW LumenProgressShow
  !macroend
  !macro customFinishPage
    Page custom LumenFinishShow LumenFinishLeave
  !macroend
  ; The installer always creates both shortcuts: drop the ones the options page switched off.
  !macro customInstall
    ${IfNot} ${Silent}
      ${If} $LumenDesktop == 0
        Delete "$newDesktopLink"
      ${EndIf}
      ${If} $LumenMenu == 0
        Delete "$newStartMenuLink"
        StrCpy $launchLink "$INSTDIR\${APP_EXECUTABLE_FILENAME}"
      ${EndIf}
    ${EndIf}
  !macroend
!else
  !macro customHeader
    ManifestDPIAware true
    !insertmacro LUMEN_CODE "un."
  !macroend
  !macro customInstallMode
    StrCpy $isForceCurrentInstall "1"
  !macroend
  !macro customUnInit
    Call un.LumenInit
  !macroend
  !macro customUnWelcomePage
    UninstPage custom un.LumenWelcomeShow un.LumenWelcomeLeave
    !define MUI_PAGE_CUSTOMFUNCTION_SHOW un.LumenProgressShow
  !macroend
  !macro customUninstallPage
    !define MUI_PAGE_CUSTOMFUNCTION_PRE un.LumenSkipPage
    UninstPage custom un.LumenFinishShow un.LumenFinishLeave
  !macroend
!endif
