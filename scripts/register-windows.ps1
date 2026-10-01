[CmdletBinding(SupportsShouldProcess)]
param([string]$EditorPath)

$ErrorActionPreference = 'Stop'
if ($env:OS -ne 'Windows_NT') { throw 'File registration requires Windows.' }

if (-not $EditorPath) {
    $code = Get-Command code -ErrorAction SilentlyContinue
    if ($code) {
        $candidate = Join-Path (Split-Path (Split-Path $code.Source)) 'Code.exe'
        if (Test-Path -LiteralPath $candidate -PathType Leaf) { $EditorPath = $candidate }
    }
    if (-not $EditorPath) { $EditorPath = Join-Path $env:WINDIR 'System32\notepad.exe' }
}
$editor = Get-Item -LiteralPath $EditorPath
if ($editor.PSIsContainer -or $editor.Extension -ne '.exe') {
    throw 'EditorPath must name an existing editor .exe.'
}
$command = '"{0}" "%1"' -f $editor.FullName
$existing = [Microsoft.Win32.Registry]::ClassesRoot.OpenSubKey('.ubi')
$default = $null
if ($existing) {
    try { $default = $existing.GetValue('') }
    finally { $existing.Dispose() }
}
if (-not $PSCmdlet.ShouldProcess('HKCU\Software\Classes', ".ubi -> Ubi.Source; open with $command")) { return }

# Register the handler before publishing the extension association.
$entries = @(
    @('Ubi.Source', '', 'Ubi source file'),
    @('Ubi.Source\DefaultIcon', '', ('"{0}",0' -f $editor.FullName)),
    @('Ubi.Source\shell', '', 'open'),
    @('Ubi.Source\shell\open\command', '', $command),
    @('.ubi\OpenWithProgids', 'Ubi.Source', '')
)
foreach ($entry in $entries) {
    $key = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey('Software\Classes\' + $entry[0])
    try { $key.SetValue($entry[1], $entry[2], [Microsoft.Win32.RegistryValueKind]::String) }
    finally { $key.Dispose() }
}
$extension = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey('Software\Classes\.ubi')
try {
    if (-not $default) { $extension.SetValue('', 'Ubi.Source') }
    $extension.SetValue('Content Type', 'text/plain')
    $extension.SetValue('PerceivedType', 'text')
} finally { $extension.Dispose() }

if (-not ('Ubi.ShellRegistration' -as [type])) {
    Add-Type @'
using System;
using System.Runtime.InteropServices;
namespace Ubi {
    public static class ShellRegistration {
        [DllImport("shell32.dll")]
        public static extern void SHChangeNotify(uint eventId, uint flags, IntPtr item1, IntPtr item2);
    }
}
'@
}
[Ubi.ShellRegistration]::SHChangeNotify(0x08000000, 0, [IntPtr]::Zero, [IntPtr]::Zero)
Write-Output "Registered .ubi as Ubi source file. Editor: $($editor.FullName)"
$choice = Get-ItemProperty -LiteralPath 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.ubi\UserChoice' -ErrorAction SilentlyContinue
if (($default -and $default -ne 'Ubi.Source') -or ($choice -and $choice.ProgId -ne 'Ubi.Source')) {
    Write-Output 'Existing default preserved. Use Open with > Choose another app to change it.'
}
