@echo off
set SERVER_USER=root
set SERVER_IP=86.110.212.133
set REMOTE_PATH=/root

echo Building the project...
@REM cargo build --release

echo Deploying the build to the server...
scp .\target\release\P2P-Server .\config.toml %SERVER_USER%@%SERVER_IP%:%REMOTE_PATH%

echo Deployment completed.