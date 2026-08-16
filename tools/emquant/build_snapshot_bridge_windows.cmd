@echo off
setlocal EnableExtensions

if "%~1"=="" (
  echo usage: build_snapshot_bridge_windows.cmd SDK_ROOT [OUTPUT] 1>&2
  exit /b 2
)

set "SDK_ROOT=%~f1"
set "SDK_SAMPLE=%SDK_ROOT%\EmQuantAPISample"
set "SDK_INCLUDE=%SDK_SAMPLE%\include"
set "SDK_LIB=%SDK_SAMPLE%\lib"
set "SCRIPT_DIR=%~dp0"
set "REPO_ROOT=%SCRIPT_DIR%..\.."
if "%~2"=="" (
  set "OUTPUT=%REPO_ROOT%\target\emquant\emquant-snapshot.exe"
) else (
  set "OUTPUT=%~f2"
)

if not exist "%SDK_INCLUDE%\EmQuantAPI.h" (
  echo missing SDK header: %SDK_INCLUDE%\EmQuantAPI.h 1>&2
  exit /b 2
)
if not exist "%SDK_LIB%\EmQuantAPI_x64.dll" (
  echo missing SDK runtime: %SDK_LIB%\EmQuantAPI_x64.dll 1>&2
  exit /b 2
)

for %%I in ("%OUTPUT%") do set "OUTPUT_DIR=%%~dpI"
set "RUNTIME_DIR=%OUTPUT_DIR%runtime"
set "ACTIVATOR_DIR=%RUNTIME_DIR%\APIActivator"

if not exist "%OUTPUT_DIR%" mkdir "%OUTPUT_DIR%"
if errorlevel 1 exit /b 3
if not exist "%RUNTIME_DIR%" mkdir "%RUNTIME_DIR%"
if errorlevel 1 exit /b 3

set "VSWHERE=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe"
if not exist "%VSWHERE%" (
  echo Visual Studio vswhere.exe is not installed 1>&2
  exit /b 3
)
for /f "usebackq tokens=*" %%I in (`"%VSWHERE%" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set "VS_ROOT=%%I"
if not defined VS_ROOT (
  echo Visual Studio C++ x64 build tools are not installed 1>&2
  exit /b 3
)
call "%VS_ROOT%\VC\Auxiliary\Build\vcvars64.bat" >nul
if errorlevel 1 exit /b 3

cl /nologo /std:c++17 /EHsc /W4 /WX /I"%SDK_INCLUDE%" "%SCRIPT_DIR%snapshot_bridge.cpp" /Fo:"%OUTPUT_DIR%snapshot_bridge.obj" /Fe:"%OUTPUT%"
if errorlevel 1 exit /b 4

copy /y "%SDK_LIB%\EmQuantAPI_x64.dll" "%RUNTIME_DIR%\EmQuantAPI_x64.dll" >nul
copy /y "%SDK_LIB%\ServerList.json.e" "%RUNTIME_DIR%\ServerList.json.e" >nul
copy /y "%SDK_LIB%\LoginActivator.exe" "%RUNTIME_DIR%\LoginActivator.exe" >nul
if exist "%SDK_LIB%\APIActivator" (
  if not exist "%ACTIVATOR_DIR%" mkdir "%ACTIVATOR_DIR%"
  xcopy /e /i /q /y "%SDK_LIB%\APIActivator" "%ACTIVATOR_DIR%" >nul
)
if exist "%SDK_LIB%\userInfo" if not exist "%RUNTIME_DIR%\userInfo" (
  copy /y "%SDK_LIB%\userInfo" "%RUNTIME_DIR%\userInfo" >nul
)

echo built %OUTPUT%
echo installed SDK runtime %RUNTIME_DIR%
if not exist "%RUNTIME_DIR%\userInfo" (
  echo warning: EMQuant API activation is required; run %RUNTIME_DIR%\LoginActivator.exe 1>&2
)
