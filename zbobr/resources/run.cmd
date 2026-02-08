@echo off
REM Launch zbobr for this domain project.
REM Loads the env file and runs zbobr with the given arguments.

setlocal enabledelayedexpansion

set "SCRIPT_DIR=%~dp0"

REM Load configuration
if exist "%SCRIPT_DIR%.zbobr.env" (
    for /f "usebackq tokens=1,* delims==" %%A in ("%SCRIPT_DIR%.zbobr.env") do (
        set "LINE=%%A"
        if not "!LINE:~0,1!"=="#" (
            if not "%%A"=="" set "%%A=%%B"
        )
    )
) else if exist "%SCRIPT_DIR%zbobr.env" (
    for /f "usebackq tokens=1,* delims==" %%A in ("%SCRIPT_DIR%zbobr.env") do (
        set "LINE=%%A"
        if not "!LINE:~0,1!"=="#" (
            if not "%%A"=="" set "%%A=%%B"
        )
    )
) else (
    echo Error: No .zbobr.env or zbobr.env found in %SCRIPT_DIR% >&2
    exit /b 1
)

REM Run zbobr, passing all arguments through
zbobr %*
