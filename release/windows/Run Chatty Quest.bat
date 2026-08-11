@echo off
setlocal

cd /d "%~dp0"
set "CHATTY_QUEST_BASE_PATH=%~dp0"

if not exist "chatty-quest.exe" (
    echo Missing chatty-quest.exe in %~dp0
    pause
    exit /b 1
)

start "" "chatty-quest.exe"
