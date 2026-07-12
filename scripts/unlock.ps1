# envseal — Windows unlock helper (PowerShell)
#
# direnv works in pwsh, but if you'd rather unlock explicitly on Windows, run this
# from the repo root inside a shell where you'll start your AI session:
#
#   . .\scripts\unlock.ps1
#
# It decrypts secrets/secrets.enc.env with sops and sets $env: vars for THIS session
# only. Nothing is written to disk. Requires sops + your age key on this machine.

if (-not (Get-Command sops -ErrorAction SilentlyContinue)) {
  Write-Error "sops not installed"; return
}
$secretsFile = "secrets/secrets.enc.env"
if (-not (Test-Path $secretsFile)) {
  Write-Error "$secretsFile not found"; return
}

# dotenv output: KEY=VALUE lines
$lines = & sops -d --output-type dotenv $secretsFile
foreach ($line in $lines) {
  if ($line -match '^\s*#') { continue }
  if ($line -match '^\s*$') { continue }
  $idx = $line.IndexOf('=')
  if ($idx -lt 1) { continue }
  $name  = $line.Substring(0, $idx).Trim()
  $value = $line.Substring($idx + 1)
  Set-Item -Path "Env:$name" -Value $value
}
Write-Host "🔓 envseal: secrets loaded into environment for this session"
