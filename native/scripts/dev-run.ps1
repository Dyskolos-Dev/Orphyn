# Builds and starts the debug app on a throwaway profile (never the real accounts / keychain),
# then takes a screenshot. From native/:
#   scripts/dev-run.ps1 -Route /music/album/<id> -Out screenshots/x.png [-Size 1180x640] [-Profile <dir>]
param(
  [string]$Route = '/',
  [string]$Out = 'screenshots/_dev.png',
  [string]$Size = '1180x640',
  [string]$Profile = "$env:TEMP\orphyn-native-dev-profile",
  [int]$Wait = 10
)
if (-not $env:PYTHON3) { $env:PYTHON3 = "$env:LOCALAPPDATA\Programs\Python\Python314\python.exe" }
Stop-Process -Name orphyn-native -ErrorAction SilentlyContinue
cargo build -p orphyn-native 2>&1 | Select-String -Pattern '^(error|warning)' | Select-Object -First 10
$env:ORPHYN_USER_DATA_DIR = $Profile
$env:ORPHYN_ROUTE = $Route
$env:ORPHYN_WIN = $Size
Start-Process -FilePath .\target\debug\orphyn-native.exe -RedirectStandardError "$Profile-run.log"
Start-Sleep -Seconds $Wait
powershell -ExecutionPolicy Bypass -File scripts/shot-window.ps1 -Title Orphyn -Out $Out
