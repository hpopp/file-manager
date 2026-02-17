# Build stage
FROM rust:1-alpine AS builder

WORKDIR /app

# Install build dependencies for musl
RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static pkgconf

# Cache dependencies in a separate layer
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && echo "" > src/lib.rs \
    && cargo build --release \
    && rm -rf src

# Build actual source
COPY src ./src
RUN touch src/main.rs src/lib.rs && cargo build --release

# Runtime stage
FROM alpine:3.21

ARG CREATED
ARG VERSION

LABEL org.opencontainers.image.authors="Henry Popp <henry@hpopp.dev>"
LABEL org.opencontainers.image.created="${CREATED}"
LABEL org.opencontainers.image.description="Unified internal API for file storage and CMS-like file management"
LABEL org.opencontainers.image.documentation="https://github.com/hpopp/file-manager"
LABEL org.opencontainers.image.licenses="MIT"
LABEL org.opencontainers.image.source="https://github.com/hpopp/file-manager"
LABEL org.opencontainers.image.title="File Manager"
LABEL org.opencontainers.image.url="https://github.com/hpopp/file-manager"
LABEL org.opencontainers.image.vendor="Henry Popp"
LABEL org.opencontainers.image.version="${VERSION}"

RUN apk add --no-cache ca-certificates curl

# Create non-root user and directories
RUN addgroup -S -g 993 file-manager && adduser -S -u 993 file-manager -G file-manager \
    && mkdir -p /app /data /files \
    && chown file-manager:file-manager /app /data /files

WORKDIR /app

# Copy binary from builder
COPY --from=builder --chown=file-manager:file-manager /app/target/release/file-manager /usr/local/bin/file-manager

USER file-manager

EXPOSE 8080 9993

CMD ["file-manager"]
