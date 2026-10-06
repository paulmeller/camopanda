FROM lightpanda/browser:nightly
COPY --chmod=0755 entrypoint.sh /app/entrypoint.sh
ENTRYPOINT ["/bin/sh", "/app/entrypoint.sh"]
CMD ["serve", "--host", "0.0.0.0", "--port", "9222"]
