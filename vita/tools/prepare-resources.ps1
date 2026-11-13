# Prepare la part des ressources d'Horizon dont la console a besoin pour faire tourner
# le generateur et le mesher d'Horizon (le test Horizon du menu DIAGNOSTIC).
#
# Source : les ressources que le launcher Horizon a preparees depuis le Minecraft de
# l'utilisateur. Sont laisses de cote : les sons (359 Mo), les langues (86 Mo), les
# classes Java et les journaux. Teste sur PC : le test Horizon donne les memes sommes
# de controle avec ce sous-ensemble qu'avec le dossier complet ; sans data/ il echoue
# (couleurs des biomes).
#
# Le resultat (_world\resources-min) derive du Minecraft de l'utilisateur : il reste sur
# son PC et sa console, jamais dans le depot.
param(
  [string]$Resources,
  [string]$OutDir = (Join-Path $PSScriptRoot '..\..\..\_world\resources-min\26.3')
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
"Source : $Resources"
robocopy $Resources $OutDir /E /NFL /NDL /NJH /NP /XD sounds lang classes client-classes /XF '*.log' '.horizon-prepare.lock' | Select-Object -Last 6
# robocopy : 0 a 7 = reussite, 8 et plus = echec.
if ($LASTEXITCODE -ge 8) { throw "robocopy a echoue (code $LASTEXITCODE)" }
$files = Get-ChildItem -LiteralPath $OutDir -Recurse -File
'{0} : {1} fichiers, {2:n1} Mo' -f $OutDir, $files.Count, (($files | Measure-Object Length -Sum).Sum / 1MB)
exit 0
