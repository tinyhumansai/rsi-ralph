# Install the rsi-ralph binary on Windows from a GitHub release.
#
#   irm https://raw.githubusercontent.com/tinyhumansai/rsi-ralph/main/scripts/install.ps1 | iex
#
# To pass options, download it first:
#
#   & ([scriptblock]::Create((irm https://raw.githubusercontent.com/tinyhumansai/rsi-ralph/main/scripts/install.ps1))) -Version v0.4.0
#
# It downloads the zip for this machine, checks it against the release's
# .sha256 file, copies rsi-ralph.exe into the install folder and adds that
# folder to the user PATH. Each parameter falls back to an environment
# variable: RSI_RALPH_VERSION, RSI_RALPH_INSTALL_DIR, RSI_RALPH_TARGET and
# RSI_RALPH_DOWNLOAD_BASE (a releases URL, for mirrors and tests).

param(
    [string]$Version = $(if ($env:RSI_RALPH_VERSION) { $env:RSI_RALPH_VERSION } else { 'latest' }),
    [string]$Dir = $(if ($env:RSI_RALPH_INSTALL_DIR) { $env:RSI_RALPH_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'Programs\rsi-ralph\bin' }),
    [string]$Target = $env:RSI_RALPH_TARGET,
    [string]$Base = $(if ($env:RSI_RALPH_DOWNLOAD_BASE) { $env:RSI_RALPH_DOWNLOAD_BASE } else { 'https://github.com/tinyhumansai/rsi-ralph/releases' }),
    [switch]$NoPathUpdate,
    [switch]$DryRun
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

function Say([string]$Message) { Write-Host "rsi-ralph-install: $Message" }

if (-not $Target) {
    # Windows on Arm runs the x86_64 build under emulation; there is no native one yet.
    switch ($env:PROCESSOR_ARCHITECTURE) {
        'AMD64' { $Target = 'x86_64-pc-windows-msvc' }
        'ARM64' { $Target = 'x86_64-pc-windows-msvc' }
        default { throw "rsi-ralph-install: no prebuilt binary for $($env:PROCESSOR_ARCHITECTURE); build from source with cargo install" }
    }
}

$archive = "rsi-ralph-$Target.zip"
$url = if ($Version -eq 'latest') { "$Base/latest/download/$archive" } else { "$Base/download/$Version/$archive" }

Say "target  $Target"
Say "release $Version"
Say "from    $url"
Say "into    $Dir"
if ($DryRun) { return }

$work = Join-Path ([System.IO.Path]::GetTempPath()) ("rsi-ralph-" + [System.Guid]::NewGuid())
New-Item -ItemType Directory -Path $work | Out-Null
try {
    $zip = Join-Path $work $archive
    Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile $zip
    Invoke-WebRequest -UseBasicParsing -Uri "$url.sha256" -OutFile "$zip.sha256"

    $expected = ((Get-Content -Raw "$zip.sha256").Trim() -split '\s+')[0].ToLowerInvariant()
    $actual = (Get-FileHash -Algorithm SHA256 $zip).Hash.ToLowerInvariant()
    if ($expected -ne $actual) {
        throw "rsi-ralph-install: checksum mismatch for ${archive}: expected $expected, got $actual"
    }

    Expand-Archive -Path $zip -DestinationPath $work
    $binary = Join-Path $work "rsi-ralph-$Target\rsi-ralph.exe"
    if (-not (Test-Path $binary)) { throw "rsi-ralph-install: $archive holds no rsi-ralph-$Target\rsi-ralph.exe" }

    New-Item -ItemType Directory -Force -Path $Dir | Out-Null
    Copy-Item -Force $binary (Join-Path $Dir 'rsi-ralph.exe')
    Say "installed $(& (Join-Path $Dir 'rsi-ralph.exe') --version) to $Dir\rsi-ralph.exe"
}
finally {
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}

$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
$onPath = ($userPath -split ';') -contains $Dir
if ($onPath) {
    Say 'next: rsi-ralph doctor'
} elseif ($NoPathUpdate) {
    Say "$Dir is not on PATH; add it to use rsi-ralph from any folder"
} else {
    [Environment]::SetEnvironmentVariable('Path', ($(if ($userPath) { "$userPath;" } else { '' }) + $Dir), 'User')
    $env:Path = "$env:Path;$Dir"
    Say "added $Dir to your user PATH; open a new terminal, then run: rsi-ralph doctor"
}
