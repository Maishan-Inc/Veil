param([Parameter(Mandatory = $true)][string]$File)

$ErrorActionPreference = 'Stop'
if (-not $env:WINDOWS_CERTIFICATE_BASE64 -or -not $env:WINDOWS_CERTIFICATE_PASSWORD) {
  throw 'Windows code-signing certificate and password are required.'
}
if (-not (Test-Path -LiteralPath $File -PathType Leaf)) {
  throw 'The file to sign does not exist.'
}

$signTool = Get-Command signtool.exe -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source -First 1
if (-not $signTool) {
  $sdkRoot = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
  $signTool = Get-ChildItem -LiteralPath $sdkRoot -Filter signtool.exe -Recurse -File |
    Where-Object { $_.FullName -match '\\x64\\signtool\.exe$' } |
    Sort-Object FullName -Descending |
    Select-Object -ExpandProperty FullName -First 1
}
if (-not $signTool) { throw 'signtool.exe was not found.' }

$certificate = Join-Path $env:RUNNER_TEMP ([Guid]::NewGuid().ToString() + '.pfx')
try {
  [IO.File]::WriteAllBytes($certificate, [Convert]::FromBase64String($env:WINDOWS_CERTIFICATE_BASE64))
  & $signTool sign /fd SHA256 /f $certificate /p $env:WINDOWS_CERTIFICATE_PASSWORD /tr http://timestamp.digicert.com /td SHA256 $File
  if ($LASTEXITCODE -ne 0) { throw 'signtool sign failed.' }
  & $signTool verify /pa /v $File
  if ($LASTEXITCODE -ne 0) { throw 'signtool verification failed.' }
} finally {
  Remove-Item -LiteralPath $certificate -Force -ErrorAction SilentlyContinue
}
