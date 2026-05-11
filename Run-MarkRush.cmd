@echo off
setlocal
set "APP_EXE=%~dp0bin\Release\net9.0-windows\win-x64\MarkRush.exe"
set "PUBLISHED_APP_EXE=%~dp0bin\Release\net9.0-windows\win-x64\publish\MarkRush.exe"

if exist "%APP_EXE%" (
    start "" "%APP_EXE%"
) else if exist "%PUBLISHED_APP_EXE%" (
    start "" "%PUBLISHED_APP_EXE%"
) else (
    dotnet run --project "%~dp0MarkRush.csproj"
)
