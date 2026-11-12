# Compile une crate Rust de vita/ en VPK sous Windows.
#
# cargo-vita 0.2.2 sait compiler l'eboot, mais son empaquetage ecrit sous Windows des
# noms d'entrees zip avec des antislashs (sce_sys\icon0.png) : VitaShell refuse alors
# l'installation avec l'erreur 0xFFFFFFFF. Ce script empaquette lui-meme, avec des '/'.
param(
  [Parameter(Mandatory)][string]$Crate,          # dossier sous vita/, ex. rust-probe
  [Parameter(Mandatory)][string]$TitleId,
  [Parameter(Mandatory)][string]$AppName,
  [string]$AppVer = '00.01',
  [string]$VitaSdk = $env:VITASDK,
  [string]$TargetDir
)
$ErrorActionPreference = 'Stop'
if (-not $VitaSdk -or -not (Test-Path -LiteralPath "$VitaSdk\bin\arm-vita-eabi-gcc.exe")) {
  throw 'VitaSDK introuvable. Passe -VitaSdk <dossier> ou definis VITASDK.'
}
$crateDir = Join-Path $PSScriptRoot "..\$Crate" | Resolve-Path | Select-Object -ExpandProperty Path
$root = Join-Path $PSScriptRoot '..\..\..' | Resolve-Path | Select-Object -ExpandProperty Path
if (-not $TargetDir) { $TargetDir = Join-Path $root "_toolchain\target-$Crate" }
$env:VITASDK = $VitaSdk
$env:PATH = "$VitaSdk\bin;$env:PATH"
# rustc ne trouve pas le linker par le PATH seul dans cet environnement.
$env:CARGO_TARGET_ARMV7_SONY_VITA_NEWLIBEABIHF_LINKER = "$VitaSdk\bin\arm-vita-eabi-gcc.exe"
$env:CARGO_TARGET_DIR = $TargetDir

function Run([string]$exe, [string[]]$a) {
  Write-Host "> $exe $($a -join ' ')"
  & $exe @a
  if ($LASTEXITCODE -ne 0) { throw "$exe a echoue (code $LASTEXITCODE)" }
}

Push-Location -LiteralPath $crateDir
try {
  $ErrorActionPreference = 'Continue'   # cargo ecrit sa progression sur stderr
  cmd /c "cargo +nightly vita build eboot --release 2>&1" | Select-Object -Last 12
  if ($LASTEXITCODE -ne 0) { throw "cargo vita build a echoue (code $LASTEXITCODE)" }
  $ErrorActionPreference = 'Stop'
} finally { Pop-Location }

$rel = Join-Path $TargetDir 'armv7-sony-vita-newlibeabihf\release'
$self = Get-ChildItem -LiteralPath $rel -Filter *.self | Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (-not $self) { throw "aucun .self dans $rel" }
$sfo = Join-Path $rel 'param.sfo'
$vpk = Join-Path $rel "$($self.BaseName).vpk"
# ATTRIBUTE2=12 : budget memoire etendu, comme le fait cargo-vita.
Run 'vita-mksfoex.exe' @('-d', 'ATTRIBUTE2=12', '-s', "APP_VER=$AppVer", '-s', "TITLE_ID=$TitleId", $AppName, $sfo)
$packArgs = @('-s', $sfo, '-b', $self.FullName)
$static = Join-Path $crateDir 'static'
if (Test-Path -LiteralPath $static) {
  Get-ChildItem -LiteralPath $static -Recurse -File | ForEach-Object {
    $inside = $_.FullName.Substring($static.Length + 1).Replace('\', '/')
    $packArgs += @('-a', "$($_.FullName)=$inside")
  }
}
Run 'vita-pack-vpk.exe' ($packArgs + $vpk)

# Garde-fou : aucun nom d'entree ne doit contenir d'antislash.
Add-Type -AssemblyName System.IO.Compression.FileSystem
$zip = [IO.Compression.ZipFile]::OpenRead($vpk)
$bad = @($zip.Entries | Where-Object { $_.FullName.Contains('\') })
$names = $zip.Entries | ForEach-Object { $_.FullName }
$zip.Dispose()
if ($bad.Count) { throw "VPK invalide : entrees avec antislash" }

$dest = Join-Path $root 'VPK here'
if (Test-Path -LiteralPath $dest) { Copy-Item -LiteralPath $vpk -Destination $dest -Force }
"Entrees : $($names -join ', ')"
"VPK     : $vpk"
"Taille  : $((Get-Item -LiteralPath $vpk).Length) octets"
"SHA256  : $((Get-FileHash -LiteralPath $vpk -Algorithm SHA256).Hash.ToLower())"
