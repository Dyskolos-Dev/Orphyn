# Clicks inside a running window at window-relative physical pixels (screenshot automation):
#   scripts/click.ps1 -Title Orphyn -X 1200 -Y 530
param([string]$Title = 'Orphyn', [int]$X, [int]$Y)
Add-Type @'
using System; using System.Runtime.InteropServices;
public class C2 { [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
 [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern void mouse_event(int f, int x, int y, int d, int e);
 public struct RECT { public int L, T, R, B; } }
'@
[C2]::SetProcessDPIAware() | Out-Null
$p = Get-Process | Where-Object { $_.MainWindowTitle -like "*$Title*" -and $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $p) { 'window not found'; exit 1 }
$r = New-Object C2+RECT
[C2]::GetWindowRect($p.MainWindowHandle, [ref]$r) | Out-Null
[C2]::SetForegroundWindow($p.MainWindowHandle) | Out-Null
[C2]::SetCursorPos($r.L + $X, $r.T + $Y) | Out-Null
Start-Sleep -Milliseconds 150
[C2]::mouse_event(0x2, 0, 0, 0, 0); Start-Sleep -Milliseconds 60; [C2]::mouse_event(0x4, 0, 0, 0, 0)
