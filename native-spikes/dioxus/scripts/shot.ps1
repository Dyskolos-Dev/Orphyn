# Runs the release exe (Windows), waits, saves a screenshot of its window. Usage:
#   scripts/shot.ps1 -Out screenshots/win-album.png [-Sheet] [-Delay 15]
param([string]$Out = 'screenshots/win.png', [switch]$Sheet, [int]$Delay = 15)
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System; using System.Runtime.InteropServices;
public class W { [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
 public struct RECT { public int L, T, R, B; } }
'@
[W]::SetProcessDPIAware() | Out-Null
$env:ORPHYN_AUTOPLAY = '1'; $env:ORPHYN_WIN = '840x540'
if ($Sheet) { $env:ORPHYN_OPEN_SHEET = '1' }
$exe = Join-Path $PSScriptRoot '..\target\release\orphyn-dioxus-spike.exe'
$p = Start-Process -FilePath $exe -WorkingDirectory (Join-Path $PSScriptRoot '..') -PassThru
Start-Sleep $Delay
$p.Refresh()
[W]::SetForegroundWindow($p.MainWindowHandle) | Out-Null
Start-Sleep 1
$r = New-Object W+RECT
[W]::GetWindowRect($p.MainWindowHandle, [ref]$r) | Out-Null
$w = $r.R - $r.L; $h = $r.B - $r.T
$bmp = New-Object System.Drawing.Bitmap $w, $h
[System.Drawing.Graphics]::FromImage($bmp).CopyFromScreen($r.L, $r.T, 0, 0, (New-Object System.Drawing.Size $w, $h))
New-Item -ItemType Directory -Force (Split-Path $Out) | Out-Null
$bmp.Save((Join-Path (Get-Location) $Out))
"window ${w}x${h}, working set $([math]::Round($p.WorkingSet64/1MB)) MB"
Stop-Process -Id $p.Id -Force
