@echo off
(goto) 2>nul & powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0setup.ps1" -Action Uninstall %*
