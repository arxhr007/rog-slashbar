# Installs the rog-slashbar binary into the plugin data directory, then records a session start.
#
# The release version and its SHA-256 are pinned below. A download that does not match the
# hash is deleted and never run. The binary is built from this repository's source; see
# the release page and the README ("Privacy" and "Verify the binary").
#
# Runs on SessionStart. It must print nothing to stdout, because SessionStart output is
# added to Claude's context.

$ErrorActionPreference = 'Stop'

$Version = '0.2.0'
$Sha256  = '0eb3d16655c3e4c7750bc2e85c3121bea95d0f959e3456c4c54cfa87392910bb'
$Url     = "https://github.com/arxhr007/rog-slashbar/releases/download/v$Version/rog-slashbar-v$Version-windows-x64.exe"

$data = $env:CLAUDE_PLUGIN_DATA
if (-not $data) {
    [Console]::Error.WriteLine('rog-slashbar: CLAUDE_PLUGIN_DATA is not set')
    exit 1
}
New-Item -ItemType Directory -Force -Path $data | Out-Null

$exe   = Join-Path $data 'rog-slashbar.exe'
$stamp = Join-Path $data 'installed.txt'
$want  = "$Version $Sha256"

$current = $false
if ((Test-Path $exe) -and (Test-Path $stamp)) {
    $current = ((Get-Content $stamp -Raw).Trim() -eq $want)
}

if (-not $current) {
    try {
        [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
        $tmp = Join-Path $data ('download-' + [guid]::NewGuid().ToString('N') + '.exe')
        Invoke-WebRequest -Uri $Url -OutFile $tmp -UseBasicParsing

        $actual = (Get-FileHash -Algorithm SHA256 -Path $tmp).Hash.ToLower()
        if ($actual -ne $Sha256.ToLower()) {
            Remove-Item $tmp -Force
            throw "checksum mismatch (expected $Sha256, got $actual)"
        }

        if (Test-Path $exe) {
            # Stop the old daemon so it releases its lock, then move the exe aside:
            # a running exe can be renamed but not overwritten.
            & $exe ctl stop | Out-Null
            Start-Sleep -Seconds 4
            Move-Item -Force $exe (Join-Path $data ('old-' + [guid]::NewGuid().ToString('N') + '.exe'))
        }
        Move-Item -Force $tmp $exe
        Set-Content -Path $stamp -Value $want -NoNewline

        Get-ChildItem $data -Filter 'old-*.exe' -ErrorAction SilentlyContinue | ForEach-Object {
            try { Remove-Item $_.FullName -Force } catch { }
        }
    } catch {
        [Console]::Error.WriteLine("rog-slashbar: install failed: $_")
        exit 1
    }
}

& $exe hook start | Out-Null
exit 0
