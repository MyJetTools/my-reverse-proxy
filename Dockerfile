FROM ubuntu:22.04
COPY ./target/release/my-reverse-proxy ./target/release/my-reverse-proxy
ENTRYPOINT ["./target/release/my-reverse-proxy"]