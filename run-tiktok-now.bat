@echo off
rem v2.1.0: version-agnostic launcher — finds the bundled TikTok-Now
rem executable regardless of which release version it is, so the launcher
rem never goes stale on a version bump.
setlocal
set "EXE="
for %%f in ("%~dp0TikTok-Now_v*_windows-x86_64.exe") do set "EXE=%%f"
if defined EXE (
  start "" "%EXE%"
) else (
  echo TikTok-Now executable not found next to this launcher.
  pause
)
