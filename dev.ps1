# Run Volvix Home locally for quick changes. No Docker, nothing to host.
#
#   powershell -File dev.ps1            # server + sample music
#   powershell -File dev.ps1 -Mocks     # also fake Lidarr + MusicBrainz, so Search -> "Get more music" works
#
# Then open http://localhost:4747 yourself. The website is served straight from web/, so just refresh
# the page after editing web/js or web/css. Only Rust changes (server/src) need a restart (Ctrl+C, run again).
# Everything lives in .dev/ (sample music, accounts). Delete that folder for a clean first-run setup.
param([switch]$Mocks, [int]$Port = 4747)

$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
$dev = Join-Path $root '.dev'
$music = Join-Path $dev 'music'
$data = Join-Path $dev 'data'
New-Item -ItemType Directory -Force $music, $data | Out-Null

if (-not (Get-ChildItem $music -Recurse -File -ErrorAction SilentlyContinue)) {
  Write-Host 'Making sample music...'
  node (Join-Path $root 'tools/dev/make-music.js') $music | Out-Null
}

$env:VH_MUSIC_DIR = $music
$env:VH_DATA_DIR = $data
$env:VH_WEB_DIR = Join-Path $root 'web'
$env:VH_PORT = "$Port"
$env:VH_RESCAN_MINUTES = '0'

$procs = @()
try {
  if ($Mocks) {
    $procs += Start-Process node -ArgumentList @((Join-Path $root 'tools/dev/mock-lidarr.js'), $music, $data, '8686') -PassThru -WindowStyle Hidden
    $procs += Start-Process node -ArgumentList @((Join-Path $root 'tools/dev/mock-mb.js'), '3100') -PassThru -WindowStyle Hidden
    $procs += Start-Process node -ArgumentList @((Join-Path $root 'tools/dev/mock-lrclib.js'), '3101') -PassThru -WindowStyle Hidden
    $procs += Start-Process node -ArgumentList @((Join-Path $root 'tools/dev/mock-wiki.js'), '3102') -PassThru -WindowStyle Hidden
    $env:VH_WIKIPEDIA_URL = 'http://127.0.0.1:3102'; $env:VH_WIKIDATA_URL = 'http://127.0.0.1:3102'
    $env:VH_MUSICBRAINZ_URL = 'http://127.0.0.1:3100/ws/2'
    $env:VH_LIDARR_URL = 'http://127.0.0.1:8686'   # already connected, so Search -> Get works right away
    $env:VH_LIDARR_API_KEY = 'testkey'
    $env:VH_LRCLIB_URL = 'http://127.0.0.1:3101'
    Write-Host 'Fake Lidarr (port 8686), MusicBrainz (3100) and LRCLIB (3101) are running.'
    Write-Host 'In the app: Settings -> Lidarr -> address 127.0.0.1:8686, API key  testkey'
  }
  Write-Host ''
  Write-Host "Open http://localhost:$Port  (first visit creates the admin account)"
  Write-Host 'Ctrl+C to stop.'
  Write-Host ''
  cargo run --manifest-path (Join-Path $root 'server/Cargo.toml')
}
finally {
  # stop only the helper processes this script started
  foreach ($p in $procs) { if ($p -and -not $p.HasExited) { $p.Kill() } }
}
