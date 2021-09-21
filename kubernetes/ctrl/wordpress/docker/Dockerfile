FROM alpine/helm

ADD ctrl.sh /ctrl.sh
ADD wordpress.tar.gz /wordpress.tar.gz

RUN chmod 755 /ctrl.sh

WORKDIR /

ENTRYPOINT ["/ctrl.sh"]
