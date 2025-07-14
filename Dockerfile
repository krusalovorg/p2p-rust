FROM debian:latest

RUN apt-get update && apt-get install -y \
    libssl-dev \
    ca-certificates \
    software-properties-common \
    wget \
    build-essential \
    zlib1g-dev \
    libncurses5-dev \
    libgdbm-dev \
    libnss3-dev \
    libreadline-dev \
    libffi-dev \
    libsqlite3-dev \
    libbz2-dev \
    && rm -rf /var/lib/apt/lists/*

# Устанавливаем Python 3.10
RUN wget https://www.python.org/ftp/python/3.10.13/Python-3.10.13.tgz && \
    tar xzf Python-3.10.13.tgz && \
    cd Python-3.10.13 && \
    ./configure --enable-optimizations --enable-shared LDFLAGS="-Wl,-rpath /usr/local/lib" && \
    make -j $(nproc) && \
    make install && \
    cd .. && \
    rm -rf Python-3.10.13 Python-3.10.13.tgz && \
    ldconfig

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