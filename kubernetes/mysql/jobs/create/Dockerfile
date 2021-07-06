FROM alpine:3.13.5

RUN  apk update && \
     apk add mysql-client mariadb-connector-c

ADD cmd.sh /cmd.sh

RUN chmod 755 /cmd.sh

WORKDIR /

ENTRYPOINT ["/cmd.sh"]
