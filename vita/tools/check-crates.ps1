# Verifie crate par crate quelles parties d'Horizon compilent (cargo check) pour la
# cible PS Vita armv7-sony-vita-newlibeabihf. Type-check uniquement : ni edition de
# liens, ni execution. Resultats dans vita/docs/crate-check-results.txt.
param(
  [string]$VitaSdk = $env:VITASDK,
  [string]$TargetDir = (Join-Path $PSScriptRoot '..\..\..\_toolchain\target-vita')
)
if (-not $VitaSdk) { throw 'Passe -VitaSdk <dossier> ou definis VITASDK.' }
$env:VITASDK = $VitaSdk
$env:PATH = "$VitaSdk\bin;$env:PATH"
$env:CARGO_TARGET_DIR = $TargetDir
$repo = Join-Path $PSScriptRoot '..\..' | Resolve-Path | Select-Object -ExpandProperty Path
$out = Join-Path $repo 'vita\docs\crate-check-results.txt'
$logs = Join-Path $TargetDir 'check-logs'
New-Item -ItemType Directory -Force $logs | Out-Null
Set-Location -LiteralPath $repo

# nom ; arguments supplementaires
$crates = @(
  @('horizon-core', ''), @('horizon-terrain-data', ''), @('horizon-hud-data', ''),
  @('horizon-threads', '--no-default-features'), @('horizon-assets', ''), @('horizon-world', ''),
  @('horizon-client-core', ''), @('horizon-items', ''), @('horizon-resource-zip', ''),
  @('horizon-terrain-mesh', '--no-default-features'), @('horizon-client-platform', '--no-default-features'),
  @('horizon-client-input', '--no-default-features'), @('horizon-client-font', ''), @('horizon-shaders', ''),
  @('horizon-client-hud', ''), @('horizon-client-ui', ''), @('horizon-protocol', ''), @('horizon-command', ''),
  @('horizon-server', ''), @('horizon-client-render', '--no-default-features'), @('horizon-client', '--no-default-features')
)
"cargo check --target armv7-sony-vita-newlibeabihf -Z build-std=std,panic_abort" | Set-Content -LiteralPath $out -Encoding utf8
"$(rustc +nightly --version) ; commit $(git rev-parse --short HEAD) ; $(Get-Date -Format s)" | Add-Content -LiteralPath $out -Encoding utf8
foreach ($c in $crates) {
  $name = $c[0]
  $extra = @(); if ($c[1]) { $extra = $c[1].Split(' ') }
  $log = Join-Path $logs "$name.log"
  $sw = [Diagnostics.Stopwatch]::StartNew()
  $a = @('+nightly', 'check', '-p', $name, '--lib', '--target', 'armv7-sony-vita-newlibeabihf', '-Z', 'build-std=std,panic_abort') + $extra
  $p = Start-Process -FilePath cargo -ArgumentList $a -NoNewWindow -Wait -PassThru -RedirectStandardError $log -RedirectStandardOutput "$log.out"
  $sw.Stop()
  $status = if ($p.ExitCode -eq 0) { 'OK  ' } else { 'FAIL' }
  $why = ''
  if ($p.ExitCode -ne 0) {
    $why = (Select-String -LiteralPath $log -Pattern '^error' | Select-Object -First 1).Line
    $n = (Select-String -LiteralPath $log -Pattern '^error(\[|:)').Count
    $why = "[$n erreurs] $why"
  }
  $line = '{0} {1,-26} {2,-22} {3,5:n0}s  {4}' -f $status, $name, $c[1], $sw.Elapsed.TotalSeconds, $why
  $line
  $line | Add-Content -LiteralPath $out -Encoding utf8
}
