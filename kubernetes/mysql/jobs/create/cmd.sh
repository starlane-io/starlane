#!/bin/sh


# this is not particularly secure at the moment, we will be able to pass in custom passwords in a future release
echo "CREATE DATABASE $DATABASE_NAME;" | mysql --host=$HOST --user=root --password=password
