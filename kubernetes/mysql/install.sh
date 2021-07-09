#!/bin/bash

set -e

helm repo add presslabs https://presslabs.github.io/charts
helm install mysql-operator presslabs/mysql-operator 

echo "creating secret... [generic password is 'password' for now]"
kubectl create secret generic  my-secret\
        --from-literal=rootUser=root \
        --from-literal=rootHost=% \
        --from-literal=rootPassword="password"

kubectl apply -f conf/cluster.yaml

echo "done."
echo 

echo "#to track the INIT progress of the new cluster:"
echo "kubectl get innodbcluster --watch"
echo

