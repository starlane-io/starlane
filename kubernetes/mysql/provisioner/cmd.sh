#!/bin/sh

COMMAND=$1
STARLANE_ADDRESS=$2
KUBERNETES_RESOURCE_NAME=$3
SNAKE_CASE_RESOURCE_NAME=$4

# we use snake case for the DB as that is what MYSQL likes
DATABASE_NAME=$SNAKE_CASE_RESOURCE_NAME

echo $@
echo "verifying..."

# verify that the db does not exist
echo "quit" | mysql --host=$HOST --user=$ROOT_USER --port $PORT --password=$ROOT_PASSWORD $DATABASE_NAME 2> /dev/null > /dev/null

ret=$?
if [ $ret -eq 0 ]; then
        echo "Database $DATABASE_NAME already exists"
        exit 1
fi

set -e


USER=$DATABASE_NAME
PASSWORD=`openssl rand -base64 32`

ENCODED_PASSWORD=`echo -n $PASSWORD | base64`

SECRET_YAML="
apiVersion: v1
kind: Secret
metadata:
  generateName: $KUBERNETES_RESOURCE_NAME-
type: kubernetes.io/basic-auth
stringData:
  password: \"$ENCODED_PASSWORD\"
"

echo "$SECRET_YAML"

SECRET_NAME=`echo "$SECRET_YAML" | kubectl create -o name -f -`
echo $SECRET_NAME

CREATE_DB="
CREATE DATABASE $DATABASE_NAME;
USE $DATABASE_NAME;
CREATE USER '$USER'@'localhost' IDENTIFIED BY '$PASSWORD';
GRANT ALL PRIVILEGES ON $DATABASE_NAME . * TO '$USER'@'localhost';
FLUSH PRIVILEGES;
"

echo "$CREATE_DB"

echo "$CREATE_DB" | mysql --host=$HOST --port $PORT --user=$ROOT_USER --password=$ROOT_PASSWORD

# verify that the db was created
echo "quit" | mysql --host=$HOST --port $PORT --user=$ROOT_USER --password=$ROOT_PASSWORD $DATABASE_NAME



# annotate the starlaneresource
kubectl annotate starlaneresources $KUBERNETES_RESOURCE_NAME url="$HOST:$PORT/$DATABASE_NAME"
kubectl annotate starlaneresources $KUBERNETES_RESOURCE_NAME secret="$SECRET_NAME"

echo "done"

exit 0

