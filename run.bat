@echo off
title AI Evolution Simulator 3D
echo Starting AI Evolution Simulator...
timeout /t 1 /nobreak >nul
start http://127.0.0.1:7890
if exist evolution_sim.exe (
    evolution_sim.exe
) else if exist target\release\evolution_sim.exe (
    target\release\evolution_sim.exe
) else (
    echo Building project...
    cargo run --release
)
pause
