# Используем минимальный базовый образ
FROM debian:latest

# Устанавливаем необходимые зависимости
RUN apt-get update && apt-get install -y \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Устанавливаем рабочую директорию
WORKDIR /app

# Копируем все необходимые файлы
COPY target/release/P2P-Server .
COPY config.toml .
COPY signal_servers.json .

# Открываем необходимые порты
EXPOSE 8081 8080 3031 80

# Указываем команду для запуска сервера
CMD ["./P2P-Server", "--signal"]