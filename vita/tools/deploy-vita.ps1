# Copie Horizon Vita sur la carte de la PS Vita montee par VitaShell (USB) :
#   <lecteur>:\vpk\HorizonVita.vpk            l'application a installer
#   <lecteur>:\vpk\HorizonVita-dossier        le meme contenu, installable sans lire d'archive
#   <lecteur>:\data\HorizonVita\world\world.hvw   le monde Horizon emballe sur le PC, s'il existe
#   <lecteur>:\data\HorizonVita\resources\26.3     les ressources du test Horizon, si preparees
# N'ecrit nulle part ailleurs et ne supprime que ses propres copies precedentes.
#
# Le monde et les ressources derivent du Minecraft de l'utilisateur : ils vont sur sa
# console, jamais dans le depot.
param(
  [string]$Drive = 'F:',
  [string]$Vpk = (Join-Path $PSScriptRoot '..\..\..\VPK here\HorizonVita.vpk'),
  [string]$World = (Join-Path $PSScriptRoot '..\..\..\_world\world.hvw'),
  [string]$Resources = (Join-Path $PSScriptRoot '..\..\..\_world\resources-min\26.3')
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

# Les ressources d'Horizon pour le test Horizon (prepare-resources.ps1). robocopy ne
# recopie que ce qui a change, et ne supprime rien sur la carte.
if (Test-Path -LiteralPath $Resources) {
  $to = "$Drive\data\HorizonVita\resources\26.3"
  "Copie des ressources Horizon vers $to (la premiere fois : plusieurs minutes)..."
  robocopy (Resolve-Path -LiteralPath $Resources).Path $to /E /NFL /NDL /NJH /NP /R:2 /W:2 | Select-Object -Last 6
  if ($LASTEXITCODE -ge 8) { throw "robocopy a echoue (code $LASTEXITCODE)" }
  $count = (Get-ChildItem -LiteralPath $to -Recurse -File).Count
  "Ressources sur la carte : $count fichiers"
} else {
  "Pas de ressources preparees ($Resources) : lance prepare-resources.ps1 pour le test Horizon."
}
"Sur la Vita : quitte le mode USB, FERME VitaShell completement, relance-le, puis ux0:vpk/HorizonVita.vpk > Croix > installer."
# robocopy laisse 1 ("fichiers copies") dans $LASTEXITCODE : ce n'est pas un echec.
exit 0
