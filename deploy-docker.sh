# Останавливаем контейнеры
docker stop p2p-server p2p-peer
docker rm p2p-server p2p-peer

# Создаем новый Dockerfile с правами на выполнение
cat > /root/p2p-server/Dockerfile << 'EOL'
FROM debian:latest

RUN apt-get update && apt-get install -y \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY P2P-Server .
COPY config.toml .
COPY signal_servers.json .

# Устанавливаем права на выполнение
RUN chmod +x P2P-Server

EXPOSE 8081 8080 3031 80

# Создаем директорию для хранения данных пира
RUN mkdir -p /app/storage-peer

# Используем скрипт для запуска разных конфигураций
COPY start.sh .
RUN chmod +x start.sh

CMD ["./start.sh"]
EOL

# Пересобираем и запускаем контейнеры
docker build -t p2p-server /root/p2p-server

# Запускаем сигнальный сервер
docker run -d \
    --name p2p-server \
    --restart unless-stopped \
    -p 8081:8081 \
    -p 8080:8080 \
    -p 3031:3031 \
    -p 80:80 \
    -e CONTAINER_TYPE=signal \
    p2p-server

# Запускаем пир
docker run -d \
    --name p2p-peer \
    --restart unless-stopped \
    -e CONTAINER_TYPE=peer \
    -v /root/p2p-server/storage-peer:/app/storage-peer \
    p2p-server