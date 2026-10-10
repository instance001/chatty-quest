param(
    [Parameter(Mandatory = $true)]
    [string]$Model
)

$ErrorActionPreference = 'Stop'

$projectRoot = Split-Path -Parent $PSCommandPath
$server = Join-Path $projectRoot 'runtime\bin\windows\llama-server.exe'
$modelPath = if ([System.IO.Path]::IsPathRooted($Model)) {
    $Model
} else {
    Join-Path $projectRoot (Join-Path 'models\local' $Model)
}

if (-not (Test-Path -LiteralPath $server -PathType Leaf)) {
    throw "Missing project-local llama.cpp server: $server"
}
if (-not (Test-Path -LiteralPath $modelPath -PathType Leaf)) {
    throw "Missing local GGUF model: $modelPath"
}

& $server -m $modelPath --host 127.0.0.1 --port 18080 --ctx-size 8192 --gpu-layers 99 --reasoning off
