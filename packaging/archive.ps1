# Builds the Windows release archive.
#
# This lives in a script rather than inline in the release workflow so that CI
# can run the same commands on every change. The release job only fires on a
# tag, so an archive step written only there would first be exercised during a
# publication, where a mistake is expensive and public.

[CmdletBinding()]
param(
    # Rust target triple, which is also the directory `cargo build` wrote to.
    [Parameter(Mandatory = $true)][string]$Target,
    # File name of the published archive.
    [Parameter(Mandatory = $true)][string]$Archive
)

$ErrorActionPreference = 'Stop'

$binary = "target/$Target/release/normfix.exe"
if (-not (Test-Path $binary)) {
    throw "no release binary at $binary"
}

$stage = "dist/$Target"
New-Item -ItemType Directory -Force -Path $stage | Out-Null
Copy-Item $binary "$stage/normfix.exe"
Copy-Item README.md, LICENSE $stage

$destination = "dist/$Archive"
if (Test-Path $destination) {
    Remove-Item $destination
}
Compress-Archive -Path "$stage/*" -DestinationPath $destination

# An archive missing the binary, or carrying a stale one, would install
# silently and fail later. Check what was actually written.
$entries = [IO.Compression.ZipFile]::OpenRead((Resolve-Path $destination)).Entries.Name
foreach ($expected in @('normfix.exe', 'README.md', 'LICENSE')) {
    if ($entries -notcontains $expected) {
        throw "$Archive does not contain $expected"
    }
}
Write-Output "$destination contains $($entries -join ', ')"
