# Emballe une region d'un vrai monde Horizon pour la PS Vita (vita/world-tools, `pack`),
# puis en fait deux images de controle avec le rendu logiciel (`preview`).
#
# Les ressources lues sont celles que le launcher Horizon a preparees depuis le
# Minecraft de l'utilisateur. Le resultat (_world\world.hvw) en derive : il reste sur
# son PC et sa console, et ne doit jamais etre versionne ni partage.
param(
  [long]$Seed = 20261112,
  [int]$Radius = 2,                 # chunks autour du point d'apparition : 2 = 5x5 chunks
  [string]$Resources,
  [string]$OutDir = (Join-Path $PSScriptRoot '..\..\..\_world')
)
$ErrorActionPreference = 'Stop'
if (-not $Resources) {
  $versions = Join-Path $env:APPDATA 'horizon-launcher\versions'
  $found = Get-ChildItem -LiteralPath $versions -Directory -ErrorAction SilentlyContinue |
    Where-Object { Test-Path (Join-Path $_.FullName '.local\horizon-client\26.3\asset-index.json') } |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
  if (-not $found) { throw "Aucune ressource preparee sous $versions. Passe -Resources <dossier 26.3>." }
  $Resources = Join-Path $found.FullName '.local\horizon-client\26.3'
}
New-Item -ItemType Directory -Force $OutDir | Out-Null
$OutDir = (Resolve-Path -LiteralPath $OutDir).Path
$root = Join-Path $PSScriptRoot '..\..\..' | Resolve-Path | Select-Object -ExpandProperty Path
$env:CARGO_TARGET_DIR = Join-Path $root '_toolchain\target-world-tools'
Push-Location -LiteralPath (Join-Path $PSScriptRoot '..\world-tools')
try {
  $ErrorActionPreference = 'Continue'   # cargo ecrit sa progression sur stderr
  cmd /c "cargo build --release --bin pack --bin preview 2>&1" | Select-String 'error|Finished' -Context 0, 8
  if ($LASTEXITCODE -ne 0) { throw "cargo build a echoue (code $LASTEXITCODE)" }
  $ErrorActionPreference = 'Stop'
} finally { Pop-Location }
$bin = Join-Path $env:CARGO_TARGET_DIR 'release'
"Ressources : $Resources"
& "$bin\pack.exe" $Resources "$OutDir\world.hvw" $Seed $Radius
if ($LASTEXITCODE -ne 0) { throw "pack a echoue (code $LASTEXITCODE)" }
& "$bin\preview.exe" "$OutDir\world.hvw" "$OutDir\view-a.png" 0 -20 6
& "$bin\preview.exe" "$OutDir\world.hvw" "$OutDir\view-b.png" 135 -25 14
