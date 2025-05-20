@echo off
echo Building Rust project...
cargo build --release

echo Building Docker image...
docker build -t p2p-server .

echo Stopping and removing existing container if exists...
docker stop p2p-server 2>nul
docker rm p2p-server 2>nul

echo Starting new container...
docker run -d \
    --name p2p-server \
    --restart unless-stopped \
    -p 8081:8081 \
    -p 8080:8080 \
    -p 3031:3031 \
    -p 80:80 \
    p2p-server

echo Container started!
echo You can check logs with: docker logs -f p2p-server 