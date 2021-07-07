FROM alpine:3.13.5

RUN  apk update && \
     apk add mysql-client mariadb-connector-c && \
     apk add curl && \
     apk add openssl 

RUN curl -LO "https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/amd64/kubectl" && \
    mv kubectl /usr/local/bin/kubectl && \
    chmod 0755 /usr/local/bin/kubectl

#FROM ubuntu:latest

#RUN apt-get update && \
#    apt-get install -y apt-transport-https ca-certificates curl

#RUN curl -fsSLo /usr/share/keyrings/kubernetes-archive-keyring.gpg https://packages.cloud.google.com/apt/doc/apt-key.gpg && \
#    echo "deb [signed-by=/usr/share/keyrings/kubernetes-archive-keyring.gpg] https://apt.kubernetes.io/ kubernetes-xenial main" | tee /etc/apt/sources.list.d/kubernetes.list && \
#    apt-get update && \
#    apt-get install -y kubectl


ADD cmd.sh /cmd.sh

RUN chmod 755 /cmd.sh

WORKDIR /

ENTRYPOINT ["/cmd.sh"]
