#!/bin/bash

set -e

echo "deploying CRDs..."
kubectl apply -f https://raw.githubusercontent.com/mysql/mysql-operator/trunk/deploy/deploy-crds.yaml

echo "wating 5 seconds for CRDs to propogate..."
sleep 5

echo "deploying operator..."
kubectl apply -f https://raw.githubusercontent.com/mysql/mysql-operator/trunk/deploy/deploy-operator.yaml

echo "creating secret... [generic password is 'password' for now]"
kubectl create secret generic  mysql-passwd\
        --from-literal=rootUser=root \
        --from-literal=rootHost=% \
        --from-literal=rootPassword="password"

kubectl apply -f conf/cluster.yaml

echo "done."
echo 

echo "#to track the INIT progress of the new cluster:"
echo "kubectl get innodbcluster --watch"
echo

