FROM scratch
COPY target/x86_64-unknown-linux-musl/release/rss-funnel \
    /rss-funnel
CMD ["/rss-funnel"]
