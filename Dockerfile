FROM rust:alpine as build

RUN apk update && \
    apk add bash \
            curl \
            shellcheck

WORKDIR /usr/src/proot-rs
COPY . /usr/src/proot-rs

CMD ["cargo", "build"]

