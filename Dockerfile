# Используем минимальный базовый образ
FROM debian:latest

# Устанавливаем необходимые зависимости
RUN apt-get update && apt-get install -y libssl-dev

# Устанавливаем рабочую директорию
WORKDIR /app

# Копируем скомпилированный бинарный файл в контейнер
COPY ./P2P-Server .

# Указываем команду для запуска сервера
CMD ["./P2P-Server", "--signal"]