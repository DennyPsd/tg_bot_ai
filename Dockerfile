
# Этап 1: Сборка приложения
FROM rust:1.75-slim as builder

# Установка необходимых системных зависимостей
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Создание рабочей директории
WORKDIR /app

# Копирование файлов конфигурации Cargo
COPY Cargo.toml Cargo.lock ./

# Создание фиктивного main.rs для предварительной сборки зависимостей
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Сборка зависимостей (кэширование)
RUN cargo build --release
RUN rm src/main.rs

# Копирование исходного кода
COPY src ./src

# Финальная сборка приложения
RUN cargo build --release

# Этап 2: Минимальный runtime образ
FROM debian:bookworm-slim

# Установка минимальных системных зависимостей
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# Создание пользователя для безопасности
RUN useradd -r -s /bin/false botuser

# Создание рабочей директории
WORKDIR /app

# Копирование скомпилированного бинарника из stage builder
COPY --from=builder /app/target/release/bot_tg ./bot_tg

# Копирование .env файла (если есть)
COPY .env ./

# Установка прав доступа
RUN chown -R botuser:botuser /app
USER botuser

# Переменные окружения по умолчанию
ENV RUST_LOG=info
ENV TG_TOKEN=${TG_TOKEN}
ENV API_TOKEN=${API_TOKEN}
ENV MODEL=${MODEL:-gpt-3.5-turbo}
ENV PASS_GM=${PASS_GM}

# Открытие порта если потребуется healthcheck
EXPOSE 8080

# Healthcheck
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD ./bot_tg --version || exit 1

# Запуск приложения
CMD ["./bot_tg"]