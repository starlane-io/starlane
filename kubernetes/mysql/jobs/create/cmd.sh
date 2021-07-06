#!/bin/sh


COMMAND=$1
STARLANE_RESOURCE_ADDRESS=$2
NAME=$3

echo $@
echo "verifying..."

# verify that the db does not exist
echo "quit" | mysql --host=$HOST --user=$USER --password=$PASSWORD $NAME 2> /dev/null > /dev/null

ret=$?
if [ $ret -eq 0 ]; then
        echo "Database $NAME already exists"
        exit 1
fi

set -e

echo "creating..."
echo "CREATE DATABASE $NAME;" | mysql --host=$HOST --user=$USER --password=$PASSWORD

# verify that the db was created
echo "quit" | mysql --host=$HOST --user=$USER --password=$PASSWORD $NAME

echo "done"


exit 0

