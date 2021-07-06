#!/bin/bash

docker build . --tag starlane/mysql-provisioner:latest

docker push starlane/mysql-provisioner:latest
