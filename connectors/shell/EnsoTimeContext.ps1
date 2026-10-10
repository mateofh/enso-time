# Dot-source this file from the PowerShell profile and add Invoke-EnsoTimeContext to your prompt.
$script:EnsoTimeContextScript = Join-Path $PSScriptRoot "enso-context.py"
function Invoke-EnsoTimeContext {
    Start-Process -FilePath "python" -ArgumentList @($script:EnsoTimeContextScript, "--send") -WindowStyle Hidden
}
