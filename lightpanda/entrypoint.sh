#!/bin/sh
set -eu
attempt=0
while [ ! -s /trust/proxy-ca.pem ]; do
    attempt=$((attempt + 1))
    if [ "$attempt" -ge 30 ]; then
        echo 'proxy CA not ready after 30 seconds' >&2
        exit 1
    fi
    sleep 1
done
exec lightpanda "$@" --http-proxy http://proxy:8080 --ca-cert /trust/proxy-ca.pem
