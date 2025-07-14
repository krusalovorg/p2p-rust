#!/bin/bash

# Конфигурация
SERVER="root@212.109.220.163"
REMOTE_DIR="/root/p2p-server"
LOCAL_BINARY="target/release/P2P-Server"

# Собираем релиз
echo "Building release..."
cargo build --release

# Проверяем, что бинарник существует
if [ ! -f "$LOCAL_BINARY" ]; then
    echo "Error: $LOCAL_BINARY not found. Build failed."
    exit 1
fi

# Копируем бинарник на сервер
echo "Copying binary to server..."
scp "$LOCAL_BINARY" "$SERVER:$REMOTE_DIR/"

# Запускаем деплой на сервере
echo "Deploying on server..."
ssh "$SERVER" "cd $REMOTE_DIR && \
    docker stop p2p-server || true && \
    docker rm p2p-server || true && \
    docker build --no-cache -t p2p-server . && \
    docker run -d \
        --name p2p-server \
        --restart unless-stopped \
        -p 8081:8081 \
        -p 8080:8080 \
        -p 3031:3031 \
        -p 80:80 \
        -e CONTAINER_TYPE=signal \
        p2p-server && \
    docker logs -f p2p-server" 