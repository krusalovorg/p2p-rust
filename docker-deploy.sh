# Останавливаем и удаляем старый контейнер
docker stop p2p-server
docker rm p2p-server

# Пересобираем образ с новыми файлами
docker build --no-cache -t p2p-server /root/p2p-server

# Запускаем signal сервер
docker run -d \
    --name p2p-server \
    --restart unless-stopped \
    -p 8081:8081 \
    -p 8080:8080 \
    -p 3031:3031 \
    -p 80:80 \
    -e CONTAINER_TYPE=signal \
    p2p-server

# Проверяем логи
docker logs -f p2p-server