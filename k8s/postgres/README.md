# POSTGRES ON STARLANE
The Starlane Operator has the ability to create postgres instances to support a variety of other services (Starlane itself, Keycloak, Provisioning etc.)

You can create an instance of postgres that isn't even used by Starlane if you just want to use the Starlane operator as a Postgres operator.

## DOCKER DESKTOP EXAMPLE
Assuming you have Docker Desktop setup in Kubernetes mode and already installed the Starlane operator... then proceed here:

### CUSTOM PASSWORD
If you skip this step the operator will auto generate a password for you.   If you want a custom password you will need to create a secret BEFORE you create the postgres instance:

```yaml
apiVersion: v1
data:
  # you will need to generate a new base64 encoded password value if you want a password other than `password`
  password: cGFzc3dvcmQ=
kind: Secret
metadata:
  name: my-postgres
```

This will create a password called `password`... to customize you must base64 encode the value passed to `password` in the yaml configuration. 

There is an example `password-hack.yaml` in this directory that you can set by running:

```bash
kubectl create -f password-hack.yaml
```

### POSTGRES INSTANCE
To create a postgres instance you will need this resource configuraiton:

```yaml
apiVersion: starlane.starlane.io/v1alpha1
kind: Postgres
metadata:
  # this will be the name of the postgres Deployment, Service, Secret & Pvc
  name: my-postgres
spec:
  service-type: ClusterIP
  # Docker takes `hostpath` as its storage class, this will differ on every implementation
  # of kubernetes 
  storage-class: hostpath

  # here we are saying to delete the Pvc if the postgres instance gets deleted. 
  # this is not the default behavior
  manage-pvc: true
```

There is an example `postgres.yaml` in this directory that you can run:

```bash
kubectl create -f postgres.yaml
```

Wait a little bit then check if Deployment & Service is created named `my-postgres`  Then check if pods are created they will be named in the pattern `my-postgres-<some-unique-generated-value>`

```bash
kubectl get deployment
kubectl get services
kubectl get pods
```

### CONNECTING TO POSTGRES 
Before you can connect you need to port-forward the deployment like this:

```bash
kubectl port-forward deployment/my-postgres 5432:5432
```

Assuming you have `psql` installed on your desktop you need to run:

```bash
psql -h localhost -p 5432 -U postgres
```

And you should connect.

### RANDOM PASSWORD GENERATION 
If you didn't set a custom password the operator will auto generate a random one for you.

To get the value of that password run the shell script in this directory:

```bash
./postgres-password.sh
```

This will copy the password into your clipboard (works for Mac only)

## KUBERNETES PLATFORMS
The only big difference between installing on Docker Desktop and some other Kubernetes service will be the `storage-class` which will differ on about every Kubernetes environment.

Some examples:
* GKE: 'standard' or 'ssd'
* EKS: 'gp2'

if you wanted to install postgres on GKE your resource should look like this:

```yaml
apiVersion: starlane.starlane.io/v1alpha1
kind: Postgres
metadata:
  name: my-postgres
spec:
  storage-class: standard
  manage-pvc: true
```

