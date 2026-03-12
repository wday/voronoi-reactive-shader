@echo off
REM stereo-cam.bat — capture two USB cameras, hstack, publish via Spout
REM
REM Usage:
REM   stereo-cam.bat                          (uses defaults from stereo-cam.conf)
REM   stereo-cam.bat "Left Cam" "Right Cam"   (override camera names)
REM
REM Requires: ffmpeg.exe, spout-publish.exe, SpoutLibrary.dll in PATH
REM
REM Config: edit stereo-cam.conf for camera names, resolution, framerate

setlocal enabledelayedexpansion

REM Load defaults from config file if it exists
set "CAM_LEFT=USB Camera #1"
set "CAM_RIGHT=USB Camera #2"
set "CAM_WIDTH=640"
set "CAM_HEIGHT=480"
set "CAM_FPS=30"
set "SPOUT_NAME=StereoRig"

if exist "%~dp0stereo-cam.conf" (
    for /f "usebackq tokens=1,* delims==" %%a in ("%~dp0stereo-cam.conf") do (
        set "%%a=%%b"
    )
)

REM Override cameras from command line if provided
if not "%~1"=="" set "CAM_LEFT=%~1"
if not "%~2"=="" set "CAM_RIGHT=%~2"

set /a OUT_WIDTH=%CAM_WIDTH% * 2
set OUT_HEIGHT=%CAM_HEIGHT%

echo stereo-cam: %CAM_LEFT% + %CAM_RIGHT% @ %CAM_WIDTH%x%CAM_HEIGHT% %CAM_FPS%fps
echo stereo-cam: output %OUT_WIDTH%x%OUT_HEIGHT% as Spout "%SPOUT_NAME%"

ffmpeg -hide_banner -loglevel warning ^
  -f dshow -framerate %CAM_FPS% -video_size %CAM_WIDTH%x%CAM_HEIGHT% -i video="%CAM_LEFT%" ^
  -f dshow -framerate %CAM_FPS% -video_size %CAM_WIDTH%x%CAM_HEIGHT% -i video="%CAM_RIGHT%" ^
  -filter_complex "[0:v][1:v]hstack=inputs=2" ^
  -f rawvideo -pix_fmt rgba pipe:1 | ^
  spout-publish --name "%SPOUT_NAME%" --width %OUT_WIDTH% --height %OUT_HEIGHT%
