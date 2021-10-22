# PRODUCT ROADMAP

## WORK IN PROGRESS 

### MORE MECHTRONY GOODNESS version: 0.2.0 [in progress]
* ability for Mechtrons to create new resources
* ability for Mechtrons to message other resources
* ability for Mechtrons to message each other
* ability for Mechtrons to Watch for changes in other resources
* ability to lock and unlock resources 
* resources can be watched for changes in their child list [add or remove]
* split Mechtron guest framework into a new repository
* create a new example mechtron [upload,display profile pic]


## BACKLOG

### KUBERNETES AND DATABASES version: 0.3.0 [backlog]
* Introduction of the Starlane Kubernetes Operator 
* ability to provision a Database
* ability to extend Starlane's provisioning system through the Starlane Kubernetes Operator in order to support any relational Database that runs in a docker container
* ability to send an SQL message to a Database instance and receive a response

### WEB SERVER IMPROVEMENTS version: 0.4.0 [backlog]
* ability to serve a static site via an ArtifactBundle
* ability to tag a specific artifact (such as taging it to the 'latest' version) so that router configs can refernce by tag instead of exact version number
* FIX: return proper HttpResponse headers

### REMOTE SERIALIZATION & API version: 0.5.0 [backlog]
* create a serialization for accessing resources remotely 
* autogenerate library serializations for common languages like Rust, Java, C#, JavaScript, Python, Ruby, etc.
* mechanism for negotiating serialization version
* mechanism for downgrading serialization on the server side if the client has a lower max version
* implementation of API in RUST
* implementation of API in Java
* ability to watch for changes in Starlane resources from the new APIs

### AUTHENTICATION AND AUTHORIZATION version: 0.6.0 [backlog]
* Creation of UserBase resource which has a list of User resources as it's children
* Autheticator resource (allow for logins)
* Credentials resource (username & password)
* resource ownership chain (every resource must have an owning resource leading to a User resource)
* creation of Role resource
* ability to bind a role to User, App and Mechtron resources
* ability to grant permissions to a Role for a resource [read,write,execute]
* enforcement of permissions rules when accessing resources 
* OAuth support in Kubernetes cluster (via Keycloak)
* Ability for the WebService to enforce a login and create an HttpSession 
* usage metering on resources and resource ownership (network traffic, cpu cycles, total file size, total database size)
* quotas - set the maximum resource size that an owner can have on a particular resource (and that resource's children)
* mechtron execution quota set via bind config (specifies how many milliseconds a mechtron has to respond before it is killed)
* mechtron execution isolation & recovery... ability to stop a mechtron's execution if it has exceeded it's execution quota

### FOR THE BETTERMENT OF ALL ERROR MESSAGES version: 0.7.0 [backlog]
This release will be focussed on making sure that problems with Starlane are as easy to diagnose as possible.  The goals involve improvment of the logs, status states and making sure there is clear feedback and error and error messages if the user does something wrong.
* implementation of consistent logs that can be turned on and off for diagnosis
* ensure that the meaning of error messages are not lost when serilizing responses over the network
* clear error messages and feedback for common user mistakes like creating an improperly formatted Resource path or schematically incorrect router config file
* ability to query the status of resources [Unknown, Creating, Ready, Panic, Destroyed, etc.] also include descriptive error messages for some states like Panic describing the nature of the panic.
* ability to query the status of Stars 
* improved message delivery reliability

### MECHTRON HUB (for artifact bundles) version: 0.8.0 [backlog]
* ability to publish and share artifact bundles via on mechtronhub.io
* ability to reference an external Space via address i.e. mechtronhub.io::uberscott.com:my-favorite-mechtron:1.0.0:/Mechtron.wasm
* ability to tag an external Space i.e. [hub]::uberscott.com:my-favorite-mechtron:1.0.0:/Mechtron.wasm
* domain verification so that people cannot publish to domains unless they control said domains
* create a space based on an email address: mechtronhub.io::scott@mightydevco.com:my-mechtron:1.0.0:/Mechtron.wasm
* email verification so that people can verify they control given email
* artifact bundle metadata (name, description,image,authors) bundle.yaml file
* a webpage for each ArtifactBundleVersions sourced from README.md
* webpage has different links to previous versions of ArtifactBundle
* ability to apply searchable keywords to an ArtifactBundle (in bundle.yaml)
* ability to search hub for a particular ArtifactBundle
* publish a REST api for accesing ArtifactBundles so that languages other than Rust can download and access 
* rust client library for new rest API for downloading and unzipping ArtifactBundles as well as caching Artifacts 
* Implementation of the 'Data Conveyor' (Starlane's support for large files & messages)

### PRODUCTION version: 1.0.0-beta [backlog]
The first version where Starlane is ready for production environments.
* resource registry persistence (before resource registry was handled by in-memory SQLite)
* Starlane Operator updated to provision persistent storage
* Starlane Operator capable of distributing Stars accross multiple container
* Lot's and Lot's of testing the robustness of the app

### PRODUCTION version: 1.0.0 [backlog]
* Lot's and Lot's of more testing....

### CLIENT SIDE STARLANE version: 1.1.0 [backlog]
* implementation of WebSocket for message passing messages between a starlane client and server 
* introduction of client-side Mechtrons
* client side libaries for running starlane inside iOS and Android

### STARLANE IN JAVASCRIPT version: 1.2.0 [backlog]
* compilating and execution starlane-core in a Wasm guest
* JavaScript engine for running starlane-core Wasm

### SCHEDULER version: 1.3.0 [backlog]
* addition of the of the Scheduler, Cron, Timer & Future resources 
* addition of a Broadcast resource which broadcasts messages sent to it to all watching resources
* ability to create a Cron resource which will send a message to another resource at a configured time
* abillity to create a Timer resource [Fixed & Delayed] which will send a message to another resource repeatedly based on an interval
* ability to create a Future which will send a message after it receives a response from a potentially long running message. It can also be configured to timeout

### QUEUES & WORKERS version: 1.4.0 [backlog]
* addition of a Queue resource which will consume and hold messages
* addition of a Worker resource which will pull messages from a Queue and send them to another reasource and wait till it receives a response before pulling from the Queue again

### OPERATIONS version: 1.5.0 [backlog]
* ability to create an Operation resource which has pre-defind child resources known as Task, Wave and TaskResult
* Operation is bound to a mechtron that builds a Task list called a Wave when it is invoked with parameters
* Operation executes each task in its Wave and produces a TaskResult
* Only one version of the operation can be excecuting a Wave at any given time
* Tasks have the ability to execute Mechtrons for processing
* add ability for Operations (and resources in general) to have a 'circuit' which can be tripped making the resource inaccessable for a configured amout of time or until a condition is met... this will be useful for operations that do things like download large amounts of data when they encouter a 'rate limit' which requires pausing the operation for a while.

### EXTERNAL SERVICE version: 1.6.0 [backlog]
* ability to hook an external Service to Starlane via the Starlane Kubernetes Operator  [RestService,WebService,etc]
* ability to exchange messages with the external "Service Resource"  
* ability to add a router proxy pass to an external WebService 
* example of a Wordpress site being served through Starlane web 

### STATEFUL MECHTRONS version: 1.7.0 [backlog]
* Mechtrons can be configured as Stateful
* new configuration rules for mechtrons governing state persistence [None, ReplicateToFile] 
* ability to replicate Mechtrons accross hosts in order to survive individual machine crashes


## DONE

### INTRODUCTORY RELEASE version: 0.1.0 [done]
* ability to build starlane
* create resources: [Space, FileSystem]
* upload & download File
* publish artifact bundle
* run a webserver
* publish an http router config for a Space
* set and query a Resource property
* watch a resource for state changes
* deploy and run an App using a WebAssembly Mechtron
* webserver to serve pages from Files and Mechtrons


