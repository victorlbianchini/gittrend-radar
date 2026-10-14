<#
    GitTrend Radar - PowerShell Launcher
    Executa a aplicacao e abre o painel no Brave Browser
#>

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $scriptDir

Write-Host "========================================================" -ForegroundColor Cyan
Write-Host "   GitTrend Radar - Tendencias Diarias do GitHub" -ForegroundColor White
Write-Host "   Iniciando servidor local e abrindo no Brave..." -ForegroundColor Green
Write-Host "========================================================" -ForegroundColor Cyan

python run.py
