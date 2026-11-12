# Audit non destructif. A lancer a partir de la racine du projet dans PowerShell.
$root = (Get-Location).Path
Write-Output "ROOT: $root"
foreach ($p in @('switch\switch', 'switch', 'vita-bootstrap', 'rust-platform')) {
  $full = Join-Path $root $p
  Write-Output "CHECK $p : $(Test-Path -LiteralPath $full)"
  if ((Test-Path -LiteralPath $full) -and (Test-Path -LiteralPath (Join-Path $full 'Cargo.toml'))) {
    Write-Output "  Cargo.toml détecté"
  }
}
if (Get-Command git -ErrorAction SilentlyContinue) { git --version }
if (Get-Command wsl -ErrorAction SilentlyContinue) { wsl --status }
