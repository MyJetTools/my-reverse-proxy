FROM ubuntu:22.04
COPY ./target/release/my-reverse-proxy ./target/release/my-reverse-proxy
COPY ./ip_v6 ./ip_v6
ENTRYPOINT ["./target/release/my-reverse-proxy"]