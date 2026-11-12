# Compile vita/bootstrap en VPK sous Windows natif, sans WSL ni CMake.
# Prerequis : VitaSDK (build mingw officiel) + paquet sdl2 extraits, VITASDK defini
# ou passe en parametre. N'ecrit que dans vita/bootstrap/build.
param(
  [string]$VitaSdk = $env:VITASDK,
  [string]$TitleId = 'HRZN00001',
  [string]$AppVer = '00.02',
  [string]$AppName = 'Horizon Vita DEV'
)
$ErrorActionPreference = 'Stop'
if (-not $VitaSdk -or -not (Test-Path -LiteralPath "$VitaSdk\bin\arm-vita-eabi-gcc.exe")) {
  throw "VitaSDK introuvable. Passe -VitaSdk <dossier> ou definis VITASDK."
}
$src = Join-Path $PSScriptRoot '..\bootstrap' | Resolve-Path | Select-Object -ExpandProperty Path
$out = Join-Path $src 'build'
New-Item -ItemType Directory -Force $out | Out-Null
$env:PATH = "$VitaSdk\bin;$env:PATH"
$env:VITASDK = $VitaSdk

function Run([string]$exe, [string[]]$a) {
  Write-Host "> $exe $($a -join ' ')"
  & $exe @a
  if ($LASTEXITCODE -ne 0) { throw "$exe a echoue (code $LASTEXITCODE)" }
}

$elf = "$out\horizon_vita_bootstrap.elf"
$velf = "$out\horizon_vita_bootstrap.velf"
$self = "$out\eboot.bin"
$sfo = "$out\param.sfo"
$vpk = "$out\horizon_vita_bootstrap.vpk"
# Liste de liens = celle du sdl2.pc du paquet VitaSDK.
$libs = '-lSDL2 -lSceGxm_stub -lSceDisplay_stub -lSceCtrl_stub -lSceAppMgr_stub -lSceAppUtil_stub -lSceAudio_stub -lSceAudioIn_stub -lSceSysmodule_stub -lSceIofilemgr_stub -lSceCommonDialog_stub -lSceTouch_stub -lSceHid_stub -lSceMotion_stub -lScePower_stub -lSceProcessmgr_stub -lSceSysmem_stub -lm'.Split(' ')

Run 'arm-vita-eabi-gcc.exe' (@('-std=gnu11', '-O2', '-Wall', '-Wextra', '-Wl,-q', "$src\src\main.c", '-o', $elf) + $libs)
Run 'vita-elf-create.exe' @($elf, $velf)
Run 'vita-make-fself.exe' @('-c', '-s', $velf, $self)
Run 'vita-mksfoex.exe' @('-s', "APP_VER=$AppVer", '-s', "TITLE_ID=$TitleId", $AppName, $sfo)
$sys = "$src\sce_sys"
Run 'vita-pack-vpk.exe' @('-s', $sfo, '-b', $self,
  '-a', "$sys\icon0.png=sce_sys/icon0.png",
  '-a', "$sys\livearea\contents\bg.png=sce_sys/livearea/contents/bg.png",
  '-a', "$sys\livearea\contents\startup.png=sce_sys/livearea/contents/startup.png",
  '-a', "$sys\livearea\contents\template.xml=sce_sys/livearea/contents/template.xml",
  $vpk)
$h = Get-FileHash -LiteralPath $vpk -Algorithm SHA256
"VPK    : $vpk"
"Taille : $((Get-Item -LiteralPath $vpk).Length) octets"
"SHA256 : $($h.Hash.ToLower())"
