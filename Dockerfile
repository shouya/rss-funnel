FROM scratch
COPY %RELEASE_BINARY% /rss-funnel
ENTRYPOINT ["/rss-funnel"]
EXPOSE 8080
HEALTHCHECK CMD ["/rss-funnel", "health-check"]
CMD ["server"]
