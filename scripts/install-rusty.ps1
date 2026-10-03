# Install or refresh the `rusty` command on Windows x64, the Rusty Engine product workflow.
#
#   irm https://raw.githubusercontent.com/FuzzySlipper/rusty-engine/main/scripts/install-rusty.ps1 | iex
#   & ([scriptblock]::Create((irm .../install-rusty.ps1))) -Version 0.1.0-dev.abc123def456
#
# The Windows counterpart of install-rusty.sh. Downloads the newest published
# pair's win-x64 archive (or -Version), checks its SHA-256, hands it to
# `rusty install --archive`, so the pair lands in the shared cache, and puts
# that pair's rusty.exe in ~\.local\bin (or -BinDir), which it adds to the
# user PATH. It never changes a product's pin. `-Releases <url>` installs from
# a release mirror and records it in the cache's config.json, where `rusty`
# reads it. Everything after this is `rusty --help`.
param(
    [string]$Version = "",
    [string]$Releases = "",
    [string]$BinDir = (Join-Path $HOME ".local\bin")
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

if ($env:PROCESSOR_ARCHITECTURE -ne "AMD64") {
    throw "install-rusty: published pairs target Windows x64; this machine is $env:PROCESSOR_ARCHITECTURE."
}
if (-not (Get-Command tar -ErrorAction SilentlyContinue)) {
    throw "install-rusty: tar is required (Windows 10 1803 and later include it)."
}

$mirror = $Releases.TrimEnd("/")
$releasesUrl = if ($mirror) { $mirror } else { "https://github.com/FuzzySlipper/rusty-engine/releases" }

if (-not $Version) {
    $latest = Invoke-RestMethod "$releasesUrl/latest/download/pair-release.json"
    $Version = $latest.version
    if (-not $Version) {
        throw "install-rusty: the latest pair-release.json names no version."
    }
    # Windows archives are built after a pair is published, within about 30 minutes.
    if (-not $latest.targets."win-x64") {
        throw "install-rusty: pair $Version has no Windows archive yet. Retry in a while, or pass -Version with an earlier pair."
    }
}

$name = "rusty-engine-csharp-pair-$Version-win-x64"
$archive = "$name.tar.gz"
$url = "$releasesUrl/download/csharp-sdk-v$Version/$archive"
$work = Join-Path ([IO.Path]::GetTempPath()) ("install-rusty-" + [Guid]::NewGuid())
New-Item -ItemType Directory -Path $work | Out-Null
try {
    Write-Host "install-rusty: downloading Engine pair $Version"
    $archivePath = Join-Path $work $archive
    Invoke-WebRequest "$url.sha256" -OutFile "$archivePath.sha256" -UseBasicParsing
    Invoke-WebRequest $url -OutFile $archivePath -UseBasicParsing
    $expected = ((Get-Content "$archivePath.sha256" -Raw).Trim() -split "\s+")[0]
    if ((Get-FileHash $archivePath -Algorithm SHA256).Hash -ne $expected) {
        throw "install-rusty: $archive does not match its published SHA-256."
    }

    tar -xzf $archivePath -C $work "$name/runtime-pack/bin/rusty.exe"
    if ($LASTEXITCODE -ne 0) {
        throw "install-rusty: tar could not extract rusty.exe from $archive."
    }
    $candidate = Join-Path $work "$name\runtime-pack\bin\rusty.exe"

    if ($mirror) {
        # The same cache `rusty` uses: XDG_CACHE_HOME, else HOME or USERPROFILE.
        $cache = if ($env:XDG_CACHE_HOME) { Join-Path $env:XDG_CACHE_HOME "rusty-engine" }
                 elseif ($env:HOME) { Join-Path $env:HOME ".cache\rusty-engine" }
                 else { Join-Path $env:USERPROFILE ".cache\rusty-engine" }
        New-Item -ItemType Directory -Force -Path $cache | Out-Null
        Set-Content -Path (Join-Path $cache "config.json") -Value "{`"releases`": `"$mirror`"}" -Encoding Ascii
    }
    & $candidate install --archive $archivePath
    if ($LASTEXITCODE -ne 0) {
        throw "install-rusty: rusty install --archive failed."
    }

    # A running rusty.exe cannot be overwritten, but it can be renamed.
    New-Item -ItemType Directory -Force -Path $BinDir | Out-Null
    $target = Join-Path $BinDir "rusty.exe"
    $previous = Join-Path $BinDir "rusty.previous.exe"
    Remove-Item $previous -Force -ErrorAction SilentlyContinue
    if (Test-Path $target) {
        Move-Item $target $previous -Force
    }
    Copy-Item $candidate $target
}
finally {
    Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Host "install-rusty: installed $target from pair $Version."
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if (-not (($userPath -split ";") -contains $BinDir)) {
    $entries = @($userPath -split ";" | Where-Object { $_ }) + $BinDir
    [Environment]::SetEnvironmentVariable("Path", ($entries -join ";"), "User")
    $env:Path = "$env:Path;$BinDir"
    Write-Host "install-rusty: added $BinDir to your user PATH; new terminals find rusty by name."
}
Write-Host "Next, in a product repository: rusty status"
