<#
    GitTrend Radar - Configurador do Agendador de Tarefas do Windows
    Cria uma tarefa automatica para atualizar os repositorios do dia todos os dias as 08:00
#>

$taskName = "GitTrendRadarDailySync"
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$pythonPath = (Get-Command python).Source
$scriptPath = Join-Path $scriptDir "update_now.py"

Write-Host "Configurando atualizacao automatica diaria para GitTrend Radar..." -ForegroundColor Cyan

# Verifica se a tarefa ja existe
$existingTask = Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue

if ($existingTask) {
    Write-Host "Removendo versao anterior da tarefa..." -ForegroundColor Yellow
    Unregister-ScheduledTask -TaskName $taskName -Confirm:$false
}

# Cria a acao da tarefa
$action = New-ScheduledTaskAction -Execute $pythonPath -Argument "`"$scriptPath`" --force" -WorkingDirectory $scriptDir

# Cria o gatilho diario (as 08:00 AM)
$trigger = New-ScheduledTaskTrigger -Daily -At "08:00AM"

# Configuracoes de execucao (rodar em segundo plano, nao parar em bateria se possivel)
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -StartWhenAvailable

# Registra a nova tarefa
Register-ScheduledTask -TaskName $taskName -Action $action -Trigger $trigger -Settings $settings -Description "Atualizacao diaria automatica dos repositorios trending do GitHub para o GitTrend Radar" | Out-Null

Write-Host "Sucesso! A tarefa '$taskName' foi agendada para rodar diariamente as 08:00 AM." -ForegroundColor Green
Write-Host "Comando registrado: $pythonPath $scriptPath --force" -ForegroundColor Gray
