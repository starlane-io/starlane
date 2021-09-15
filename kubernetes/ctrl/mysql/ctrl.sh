#!/bin/sh

COMMAND=$1

# we use snake case for the DB as that is what MYSQL likes
DATABASE_NAME=$STARLANE_RESOURCE_SNAKE_KEY

# verify that the db does not exist
echo "quit" | mysql --host=$HOST --user=$ROOT_USER --port $PORT --password=$ROOT_PASSWORD $DATABASE_NAME 2> /dev/null > /dev/null

ret=$?
if [ $ret -eq 0 ]; then
        echo "Database $DATABASE_NAME already exists"
        exit 1
fi

set -e

USER=$DATABASE_NAME
PASSWORD=`openssl rand -base64 8`

#ENCODED_PASSWORD=`echo -n $PASSWORD | base64`

SECRET_YAML="
apiVersion: v1
kind: Secret
metadata:
  name: $STARLANE_RESOURCE_NAME
  ownerReferences:
  - apiVersion: $STARLANE_RESOURCE_API_VERSION
    blockOwnerDeletion: true
    controller: true
    kind: StarlaneResource
    name: $STARLANE_RESOURCE_NAME
    uid: $STARLANE_RESOURCE_UID
type: kubernetes.io/basic-auth
stringData:
  password: \"$PASSWORD\"
"

echo "$SECRET_YAML" | kubectl create -f -

CREATE_DB="
CREATE DATABASE $DATABASE_NAME;
USE $DATABASE_NAME;
CREATE USER '$USER'@'%' IDENTIFIED BY '$PASSWORD';
GRANT ALL PRIVILEGES ON $DATABASE_NAME . * TO '$USER'@'%';
FLUSH PRIVILEGES;
"

echo "$CREATE_DB" | mysql --host=$HOST --port $PORT --user=$ROOT_USER --password=$ROOT_PASSWORD

# verify that the db was created
echo "quit" | mysql --host=$HOST --port $PORT --user=$ROOT_USER --password=$ROOT_PASSWORD $DATABASE_NAME

# annotate the starlaneresource
kubectl annotate starlaneresource $STARLANE_RESOURCE_NAME url="$HOST:$PORT/$DATABASE_NAME"
kubectl annotate starlaneresource $STARLANE_RESOURCE_NAME host="$HOST"
kubectl annotate starlaneresource $STARLANE_RESOURCE_NAME secret="$STARLANE_RESOURCE_NAME"

exit 0
