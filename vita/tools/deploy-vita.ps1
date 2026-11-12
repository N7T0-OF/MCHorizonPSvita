# Copie HorizonVita.vpk sur la carte de la PS Vita montee par VitaShell (USB).
# Ne supprime rien et n'ecrit que dans <lecteur>:\vpk.
param(
  [string]$Drive = 'F:',
  [string]$Vpk = (Join-Path $PSScriptRoot '..\..\..\VPK here\HorizonVita.vpk')
)
$ErrorActionPreference = 'Stop'
if (-not (Test-Path -LiteralPath $Vpk)) { throw "VPK introuvable : $Vpk" }
# Garde-fou : ne rien ecrire sur un lecteur qui n'est pas la carte de la Vita.
if (-not (Test-Path -LiteralPath "$Drive\VitaShell") -or -not (Test-Path -LiteralPath "$Drive\app")) {
  "La carte de la Vita n'est pas montee sur $Drive (active le mode USB de VitaShell)."
  exit 1
}
$dest = "$Drive\vpk"
New-Item -ItemType Directory -Force $dest | Out-Null
Copy-Item -LiteralPath $Vpk -Destination $dest -Force
$a = (Get-FileHash -LiteralPath $Vpk -Algorithm SHA256).Hash
$b = (Get-FileHash -LiteralPath "$dest\HorizonVita.vpk" -Algorithm SHA256).Hash
if ($a -ne $b) { throw 'La copie sur la carte differe du fichier du PC.' }
"Copie verifiee : $dest\HorizonVita.vpk ($((Get-Item -LiteralPath $Vpk).Length) octets)"
# Meme contenu en dossier : VitaShell peut l'installer sans lire d'archive
# (Triangle > Plus > Installer le dossier), utile si l'installation du VPK echoue.
$folder = "$destHorizonVita-dossier"
if (Test-Path -LiteralPath $folder) { Remove-Item -LiteralPath $folder -Recurse -Force }
Add-Type -AssemblyName System.IO.Compression.FileSystem
[IO.Compression.ZipFile]::ExtractToDirectory((Resolve-Path -LiteralPath $Vpk).Path, $folder)
"Dossier installable : $folder"
"Sur la Vita : quitte le mode USB, FERME VitaShell completement, relance-le, puis ux0:vpk/HorizonVita.vpk > Croix > installer."
