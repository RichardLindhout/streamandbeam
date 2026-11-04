@echo off
rem Start Stream & Beam on this PC (GStreamer DLLs on PATH, data in .\data).
cd /d "%~dp0"
set "PATH=C:\Program Files\gstreamer\1.0\msvc_x86_64\bin;%PATH%"
target\release\streamandbeam.exe %*
pause
