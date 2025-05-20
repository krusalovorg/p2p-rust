#!/bin/bash

# Собираем проект
echo "Building Rust project..."
cargo build --release

# Создаем Dockerfile на сервере
echo "Creating Dockerfile on server..."
ssh root@212.109.220.163 "cat > /root/p2p-server/Dockerfile << 'EOL'
FROM debian:latest

RUN apt-get update && apt-get install -y \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY P2P-Server .
COPY config.toml .
COPY signal_servers.json .

EXPOSE 8081 8080 3031 80

CMD [\"./P2P-Server\", \"--signal\"]
EOL"

# Копируем файлы на сервер
echo "Copying files to server..."
scp target/release/P2P-Server root@212.109.220.163:/root/p2p-server/
scp config.toml root@212.109.220.163:/root/p2p-server/
scp signal_servers.json root@212.109.220.163:/root/p2p-server/

# Собираем и запускаем Docker контейнер на сервере
echo "Building and running Docker container..."
ssh root@212.109.220.163 "cd /root/p2p-server && \
    docker stop p2p-server 2>/dev/null || true && \
    docker rm p2p-server 2>/dev/null || true && \
    docker build -t p2p-server . && \
    docker run -d \
        --name p2p-server \
        --restart unless-stopped \
        -p 8081:8081 \
        -p 8080:8080 \
        -p 3031:3031 \
        -p 80:80 \
        p2p-server"

echo "Deployment completed!"
echo "You can check logs with: ssh root@212.109.220.163 'docker logs -f p2p-server'" 