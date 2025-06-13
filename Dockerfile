FROM rust:alpine AS builder

RUN apk add --no-cache musl-dev gcc gcc openssl-dev openssl-libs-static
WORKDIR /usr/src/app
COPY Cargo.toml Cargo.lock ./
COPY . .
COPY ./grist-client/ ./grist-client
RUN cargo build --release

FROM alpine:3

RUN apk add --no-cache libwebp-tools openssl
WORKDIR /usr/src/app
COPY --from=builder /usr/src/app/target/release/grist-image-optimizer .
COPY docker-entrypoint.sh .
RUN chmod +x docker-entrypoint.sh
RUN mkdir "/tmp/images"
ENTRYPOINT ["./docker-entrypoint.sh"]