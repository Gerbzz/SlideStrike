param(
    [Parameter(Mandatory = $true)][string]$Executable,
    [Parameter(Mandatory = $true)][string]$OutputDirectory
)

# Hosted Windows only. Capture real application pixels; never redraw, crop or replace artwork.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if (-not $IsWindows -or -not $env:RUNNER_TEMP) { throw 'Use the hosted Windows runner.' }
$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$exe = (Resolve-Path -LiteralPath $Executable).Path
$runnerRoot = [IO.Path]::GetFullPath($env:RUNNER_TEMP).TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
$output = [IO.Path]::GetFullPath($OutputDirectory)
if (-not $output.StartsWith($runnerRoot, [StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $output)) {
    throw 'Use a fresh output directory inside RUNNER_TEMP.'
}
$null = New-Item -ItemType Directory -Path $output
$privateProfile = Join-Path $runnerRoot ('slidestrike-profile-' + [Guid]::NewGuid().ToString('N'))
$roaming = Join-Path $privateProfile 'roaming'
$local = Join-Path $privateProfile 'local'
$null = New-Item -ItemType Directory -Path $roaming, $local
$sampleSource = Join-Path $repositoryRoot 'native/slidestrike/crates/engine/src/sample.rs'
$sampleHash = (Get-FileHash -LiteralPath $sampleSource -Algorithm SHA256).Hash
$revision = (& git -C $repositoryRoot rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0 -or $revision -notmatch '^[0-9a-f]{40}$') { throw 'No actual checkout revision.' }
$tree = (& git -C $repositoryRoot rev-parse 'HEAD^{tree}').Trim()
if ($LASTEXITCODE -ne 0 -or $tree -notmatch '^[0-9a-f]{40}$') { throw 'No actual source tree.' }
$appTree = (& git -C $repositoryRoot rev-parse 'HEAD:native/slidestrike').Trim()
if ($LASTEXITCODE -ne 0 -or $appTree -notmatch '^[0-9a-f]{40}$') { throw 'No actual native application tree.' }
$process = $null
$stdout = $null
$stderr = $null
$captures = @()
$status = 'failed'
$failure = $null
$window = [IntPtr]::Zero
$windowTitle = $null
$windowClass = $null

Add-Type -AssemblyName System.Windows.Forms, System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class SlideStrikeCaptureWindow {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    public delegate bool EnumProc(IntPtr window, IntPtr parameter);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc callback, IntPtr parameter);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window, out uint processId);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr window, StringBuilder text, int count);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr window, StringBuilder text, int count);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr window, out RECT rectangle);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr window, out RECT rectangle);
    [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr window, int x, int y, int width, int height, bool repaint);
    [DllImport("user32.dll")] public static extern bool ShowWindowAsync(IntPtr window, int command);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr window);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
    public static string Title(IntPtr window) {
        var text = new StringBuilder(1024); GetWindowText(window, text, text.Capacity); return text.ToString();
    }
    public static string ClassName(IntPtr window) {
        var text = new StringBuilder(256); GetClassName(window, text, text.Capacity); return text.ToString();
    }
    public static IntPtr[] Find(int processId) {
        var windows = new List<IntPtr>();
        EnumWindows((window, parameter) => {
            uint owner; RECT rectangle;
            GetWindowThreadProcessId(window, out owner);
            if (owner == processId && IsWindowVisible(window) &&
                Title(window).IndexOf("SlideStrike", StringComparison.OrdinalIgnoreCase) >= 0 &&
                ClassName(window) != "ConsoleWindowClass" && GetClientRect(window, out rectangle) &&
                rectangle.Right - rectangle.Left >= 850 && rectangle.Bottom - rectangle.Top >= 500) windows.Add(window);
            return true;
        }, IntPtr.Zero);
        return windows.ToArray();
    }
}
'@
$previousDpi = [SlideStrikeCaptureWindow]::SetThreadDpiAwarenessContext([IntPtr]::new(-4))

function Invoke-SlideStrike([string]$Method, [hashtable]$Parameters = @{}, [int]$ReadTimeout = 20000) {
    $client = [Net.Sockets.TcpClient]::new()
    try {
        if (-not $client.ConnectAsync([Net.IPAddress]::Loopback, $controlPort).Wait(1000)) { throw 'Loopback connect timed out.' }
        $stream = $client.GetStream()
        $stream.ReadTimeout = $ReadTimeout
        $stream.WriteTimeout = 5000
        $writer = [IO.StreamWriter]::new($stream, [Text.UTF8Encoding]::new($false), 4096, $true)
        $reader = [IO.StreamReader]::new($stream, [Text.UTF8Encoding]::new($false), $false, 4096, $true)
        try {
            $writer.WriteLine((@{ id = 1; method = $Method; params = $Parameters } | ConvertTo-Json -Compress -Depth 8))
            $writer.Flush()
            $line = $reader.ReadLine()
            if (-not $line) { throw "No control reply to $Method." }
            $reply = $line | ConvertFrom-Json -AsHashtable
            if (-not $reply.ok) { throw "Native app rejected $Method." }
            return $reply.result
        } finally { $reader.Dispose(); $writer.Dispose() }
    } finally { $client.Dispose() }
}

function Assert-OwnedForegroundWindow {
    [uint32]$owner = 0
    $null = [SlideStrikeCaptureWindow]::GetWindowThreadProcessId($window, [ref]$owner)
    if ($owner -ne $process.Id -or -not [SlideStrikeCaptureWindow]::IsWindowVisible($window) -or
        [SlideStrikeCaptureWindow]::GetForegroundWindow() -ne $window -or
        [SlideStrikeCaptureWindow]::Title($window) -notmatch 'SlideStrike') {
        throw 'The verified SlideStrike window is not the foreground app.'
    }
}

function Capture-View([string]$Name, [int]$SlideIndex) {
    Assert-OwnedForegroundWindow
    $clientPng = Join-Path $output ("slidestrike-$Name-client.png")
    $client = Invoke-SlideStrike 'ui.screenshot' @{ path = $clientPng }
    if ($client.width -lt 850 -or $client.height -lt 500) { throw 'Native client capture is incomplete.' }
    Start-Sleep -Milliseconds 300
    Assert-OwnedForegroundWindow
    $rectangle = [SlideStrikeCaptureWindow+RECT]::new()
    if (-not [SlideStrikeCaptureWindow]::GetWindowRect($window, [ref]$rectangle)) { throw 'No native window bounds.' }
    $work = [Windows.Forms.Screen]::PrimaryScreen.WorkingArea
    if ($rectangle.Left -lt $work.Left -or $rectangle.Top -lt $work.Top -or
        $rectangle.Right -gt $work.Right -or $rectangle.Bottom -gt $work.Bottom) { throw 'Native window does not fit the display.' }
    $outerPng = Join-Path $output ("slidestrike-$Name-window.png")
    $bitmap = [Drawing.Bitmap]::new($rectangle.Right - $rectangle.Left, $rectangle.Bottom - $rectangle.Top)
    try {
        $graphics = [Drawing.Graphics]::FromImage($bitmap)
        try { $graphics.CopyFromScreen($rectangle.Left, $rectangle.Top, 0, 0, $bitmap.Size, [Drawing.CopyPixelOperation]::SourceCopy) }
        finally { $graphics.Dispose() }
        $bitmap.Save($outerPng, [Drawing.Imaging.ImageFormat]::Png)
        $outerWidth = $bitmap.Width
        $outerHeight = $bitmap.Height
    } finally { $bitmap.Dispose() }
    Assert-OwnedForegroundWindow
    foreach ($png in @($clientPng, $outerPng)) {
        if ((Get-Item -LiteralPath $png).Length -gt 2MB) { throw 'Native screenshot exceeds the 2 MiB cap.' }
    }
    return [ordered]@{
        view = $Name; slideIndex = $SlideIndex
        client = @{ file = [IO.Path]::GetFileName($clientPng); width = $client.width; height = $client.height; sha256 = (Get-FileHash -LiteralPath $clientPng -Algorithm SHA256).Hash }
        window = @{ file = [IO.Path]::GetFileName($outerPng); width = $outerWidth; height = $outerHeight; sha256 = (Get-FileHash -LiteralPath $outerPng -Algorithm SHA256).Hash }
    }
}

try {
    $listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
    $listener.Start(); $controlPort = $listener.LocalEndpoint.Port; $listener.Stop()
    $start = [Diagnostics.ProcessStartInfo]::new($exe)
    $start.UseShellExecute = $false
    $start.WorkingDirectory = $privateProfile
    $start.Environment['APPDATA'] = $roaming
    $start.Environment['LOCALAPPDATA'] = $local
    $start.Environment['DECKCRAFT_NO_PREFS'] = '1'
    $start.Environment['DECKCRAFT_NO_RECOVERY'] = '1'
    $start.Environment['DECKCRAFT_NO_AUDIO'] = '1'
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $start.ArgumentList.Add('--control'); $start.ArgumentList.Add([string]$controlPort); $start.ArgumentList.Add('--sample')
    $process = [Diagnostics.Process]::Start($start)
    if ($null -eq $process) { throw 'SlideStrike did not start.' }
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    $deadline = [DateTime]::UtcNow.AddSeconds(90)
    $ui = $null
    do {
        if ($process.HasExited) { throw 'SlideStrike exited before its sample was ready.' }
        try { $ui = Invoke-SlideStrike 'ui.inspect' @{} 1000 } catch { Start-Sleep -Milliseconds 200 }
    } while ($null -eq $ui -and [DateTime]::UtcNow -lt $deadline)
    if ($null -eq $ui) { throw 'SlideStrike control channel did not become ready.' }
    $document = Invoke-SlideStrike 'document.inspect'
    if ($document.title -ne 'Northern Lights' -or $document.slides.Count -ne 8 -or $document.dirty -or $document.path) {
        throw 'The expected clean public eight-slide sample is not loaded.'
    }
    if ($ui.documents.Count -ne 1 -or $null -ne $ui.dialog -or $ui.home) { throw 'An unexpected document, dialog or home view is present.' }
    $deadline = [DateTime]::UtcNow.AddSeconds(15)
    do {
        $windows = @([SlideStrikeCaptureWindow]::Find($process.Id))
        if ($windows.Count -eq 1) { $window = $windows[0]; break }
        Start-Sleep -Milliseconds 200
    } while ([DateTime]::UtcNow -lt $deadline)
    if ($window -eq [IntPtr]::Zero) { throw 'A unique owned SlideStrike GUI window was not found.' }
    $work = [Windows.Forms.Screen]::PrimaryScreen.WorkingArea
    $width = [Math]::Min(1440, $work.Width - 48)
    $height = [Math]::Min(900, $work.Height - 48)
    if ($width -lt 920 -or $height -lt 620) { throw 'Hosted display is too small for readable native evidence.' }
    $null = [SlideStrikeCaptureWindow]::ShowWindowAsync($window, 9)
    if (-not [SlideStrikeCaptureWindow]::MoveWindow($window, $work.Left + 24, $work.Top + 24, $width, $height, $true)) { throw 'Cannot position the native app.' }
    $null = Invoke-SlideStrike 'ui.focus'
    $null = [SlideStrikeCaptureWindow]::SetForegroundWindow($window)
    $deadline = [DateTime]::UtcNow.AddSeconds(5)
    while ([SlideStrikeCaptureWindow]::GetForegroundWindow() -ne $window -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 100 }
    Assert-OwnedForegroundWindow
    $captures += Capture-View 'title' 0
    $null = Invoke-SlideStrike 'engine.execute' @{ command = 'slide.go'; params = @{ index = 4 } }
    $inspection = Invoke-SlideStrike 'document.inspect'
    if ($inspection.selection.slide -ne 4) { throw 'The chart sample slide was not selected.' }
    $captures += Capture-View 'chart' 4
    $windowTitle = [SlideStrikeCaptureWindow]::Title($window)
    $windowClass = [SlideStrikeCaptureWindow]::ClassName($window)
    $null = Invoke-SlideStrike 'app.quit'
    if (-not $process.WaitForExit(15000) -or $process.ExitCode -ne 0) { throw 'SlideStrike did not exit cleanly.' }
    if ((Get-FileHash -LiteralPath $sampleSource -Algorithm SHA256).Hash -ne $sampleHash) { throw 'Public sample source changed.' }
    $status = 'captured-for-independent-visual-review'
} catch {
    $failure = $_.Exception.Message
    throw
} finally {
    if ($null -ne $process -and -not $process.HasExited) { $process.Kill(); $null = $process.WaitForExit(5000) }
    if ($null -ne $process) {
        if ($status -eq 'failed' -and $null -ne $stderr -and $stderr.Wait(5000)) {
            $text = $stderr.Result
            Write-Output ('Native stderr (bounded): ' + $text.Substring(0, [Math]::Min(8192, $text.Length)))
        }
        $processId = $process.Id
        $process.Dispose()
    } else { $processId = $null }
    [ordered]@{
        status = $status; error = $failure; sourceRevision = $revision; sourceTree = $tree; nativeAppTree = $appTree
        pullRequestHead = $env:SLIDESTRIKE_CAPTURE_HEAD; runId = $env:GITHUB_RUN_ID; capturedAtUtc = [DateTime]::UtcNow.ToString('o')
        executableSha256 = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash
        publicFixture = 'native/slidestrike/crates/engine/src/sample.rs'; fixtureSha256 = $sampleHash
        processId = $processId; windowHandle = $window.ToInt64(); windowTitle = $windowTitle; windowClass = $windowClass
        foregroundAndProcessOwnershipVerified = ($status -eq 'captured-for-independent-visual-review'); captures = $captures; rasterEdits = $false
        limits = 'Hosted native public-sample capture. Requires independent visual review; not Room, taskbar, alternate DPI, installer or release acceptance.'
    } | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $output 'proof.json') -Encoding utf8
    if ($previousDpi -ne [IntPtr]::Zero) { $null = [SlideStrikeCaptureWindow]::SetThreadDpiAwarenessContext($previousDpi) }
    # Private app profile and console streams are not uploaded; the hosted runner discards them.
}
