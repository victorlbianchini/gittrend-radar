@echo off
title GitTrend Radar - Inicializador
cd /d "%~dp0"
echo ========================================================
echo   Iniciando GitTrend Radar...
echo   Atualizacao Diaria e Analise dos Melhores Repositorios
echo ========================================================
python run.py
pause
