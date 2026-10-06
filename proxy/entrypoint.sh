#!/bin/sh
set -eu
mkdir -p /state /trust
if [ ! -s /state/hudsucker-ca.key ] || [ ! -s /state/hudsucker-ca.crt ]; then
    openssl req -x509 -newkey rsa:2048 -sha256 -nodes -days 3650 \
        -keyout /state/hudsucker-ca.key.tmp -out /state/hudsucker-ca.crt.tmp \
        -subj '/CN=Lightpanda local HTTPS interception CA' \
        -addext 'basicConstraints=critical,CA:TRUE' \
        -addext 'keyUsage=critical,keyCertSign,cRLSign'
    chmod 0600 /state/hudsucker-ca.key.tmp
    mv /state/hudsucker-ca.key.tmp /state/hudsucker-ca.key
    mv /state/hudsucker-ca.crt.tmp /state/hudsucker-ca.crt
fi
cp /state/hudsucker-ca.crt /trust/proxy-ca.pem.tmp
mv /trust/proxy-ca.pem.tmp /trust/proxy-ca.pem
exec /usr/local/bin/hudsucker-proxy
