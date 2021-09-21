FROM alpine:3.13.5

RUN  apk update && \
     apk add mysql-client mariadb-connector-c && \
     apk add curl && \
     apk add openssl 

RUN curl -LO "https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/amd64/kubectl" && \
    mv kubectl /usr/local/bin/kubectl && \
    chmod 0755 /usr/local/bin/kubectl

ADD ctrl.sh /ctrl.sh

RUN chmod 755 /ctrl.sh

WORKDIR /

ENTRYPOINT ["/ctrl.sh"]
