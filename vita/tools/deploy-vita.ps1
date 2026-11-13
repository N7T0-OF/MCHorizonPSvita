# Copie Horizon Vita sur la carte de la PS Vita montee par VitaShell (USB) :
#   <lecteur>:\vpk\HorizonVita.vpk            l'application a installer
#   <lecteur>:\vpk\HorizonVita-dossier        le meme contenu, installable sans lire d'archive
#   <lecteur>:\data\HorizonVita\world\world.hvw   le monde Horizon emballe sur le PC, s'il existe
# N'ecrit nulle part ailleurs et ne supprime que ses propres copies precedentes.
#
# Le fichier du monde derive du Minecraft de l'utilisateur : il va sur sa console,
# jamais dans le depot.
param(
  [string]$Drive = 'F:',
  [string]$Vpk = (Join-Path $PSScriptRoot '..\..\..\VPK here\HorizonVita.vpk'),
  [string]$World = (Join-Path $PSScriptRoot '..\..\..\_world\world.hvw')
)
$ErrorActionPreference = 'Stop'
if (-not (Test-Path -LiteralPath $Vpk)) { throw "VPK introuvable : $Vpk" }
# Garde-fou : ne rien ecrire sur un lecteur qui n'est pas la carte de la Vita.
if (-not (Test-Path -LiteralPath "$Drive\VitaShell") -or -not (Test-Path -LiteralPath "$Drive\app")) {
  "La carte de la Vita n'est pas montee sur $Drive (active le mode USB de VitaShell)."
  exit 1
}

function Copy-Verified([string]$from, [string]$toDir) {
  New-Item -ItemType Directory -Force $toDir | Out-Null
  Copy-Item -LiteralPath $from -Destination $toDir -Force
  $to = Join-Path $toDir (Split-Path $from -Leaf)
  if ((Get-FileHash -LiteralPath $from -Algorithm SHA256).Hash -ne (Get-FileHash -LiteralPath $to -Algorithm SHA256).Hash) {
    throw "La copie $to differe du fichier du PC."
  }
  "Copie verifiee : $to ($((Get-Item -LiteralPath $from).Length) octets)"
}

$dest = "$Drive\vpk"
Copy-Verified $Vpk $dest
# Meme contenu en dossier : VitaShell peut l'installer sans lire d'archive
# (Triangle > Plus > Installer le dossier), utile si l'installation du VPK echoue.
$folder = "$dest\HorizonVita-dossier"
if (Test-Path -LiteralPath $folder) { Remove-Item -LiteralPath $folder -Recurse -Force }
Add-Type -AssemblyName System.IO.Compression.FileSystem
[IO.Compression.ZipFile]::ExtractToDirectory((Resolve-Path -LiteralPath $Vpk).Path, $folder)
"Dossier installable : $folder"

if (Test-Path -LiteralPath $World) {
  Copy-Verified (Resolve-Path -LiteralPath $World).Path "$Drive\data\HorizonVita\world"
} else {
  "Pas de monde emballe ($World) : JOUER affichera le terrain de test."
}
"Sur la Vita : quitte le mode USB, FERME VitaShell completement, relance-le, puis ux0:vpk/HorizonVita.vpk > Croix > installer."
