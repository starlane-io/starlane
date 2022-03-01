# PRODUCT ROADMAP


### FOR THE BETTERMENT OF ALL ERROR MESSAGES version: 0.3.0 [backlog]
This release will be focussed on making sure that problems with Starlane are as easy to diagnose as possible.  The goals involve improvment of the logs, status states and making sure there is clear feedback and error and error messages if the user does something wrong.

* better error messages when parsing commands
  - clear message when a syntax error occurs 
  - point out exact location of 'confusion' just like Rust compiler does

* Actually enforce Mechtron Bind released in 'MORE MECHTRONY GOODNESS'
  - ensure clear error message if there is a bind failure

* descriptive hierarchy 
  - differentiation between User & System error
  - fail hierarchy (as in what aspect of the resource failed [Create,Select,Request, etc]
  - component hierarchy to help describe exactly where the failure occured

* improved Logs
  - new Rc\<Append\> logs a file
  - creation of Rc\<Watch\<Stream\>\> to stream changes
  - command line "tail log" (which is "watch log..." for streaming)

* ability to query the status of resources [Unknown, Creating, Ready, Panic, Destroyed, etc.] also include descriptive error messages for some states like Panic describing the nature of the panic.
* ability to query the status of Stars 

* improved message reliability:
  - send message acknowledgements when a message is received by a Star
  - message will be resent if it has not been acknowledged after a timeout
  - ensure message delivery will happen once or not at all in case of a failure
  - if a message has not received an acknowledgement and is out of retries, return a Timeout failure to the sender
  - ability to configure ack timeout, retries and max retry 


### SECURITY, AUTHENTICATION AND AUTHORIZATION version: 0.4.0 [backlog]
* Persistent Storage
* Authentication
  - Credentials resource (username & password)
  - OAuth support in Kubernetes cluster (via Keycloak)
  - starlane cli login
  - Authenticator resource (allow for logins within the mesh)

* Permissions
  - resource ownership chain (every resource must have an owning resource leading to a User resource)
  - Create, Read, Update, Delete and Execute resource permission for resource and children: (crudx/crudx) ... how it will work is still being heavily revised ...
  - creation of Role resource
  - ability to bind a role to User, App and Mechtron resources
  -  ability to grant permissions to a Role for a resource [read,write,execute]
  - ability to grant additional priviledges to a message request
  - enforcement of permissions rules when accessing resources 

* Http Session
  - Ability for the WebService to enforce a login and create an HttpSession 

* Mechtron Bind Improvements:
  - whitelisting of external resource calls, including grant permissions from within the Port Definition Block
  - ability to grant additional resource permissions to a request
  - ability to set a timeout for mechtron port's execution time
  - ability to set a timeout for mechtron port's total time to respond to a message (includes whatever is being done with other resources)


### MECHTRON HUB (for artifact bundles) version: 0.5.0 [backlog]
* ability to publish and share artifact bundles on mechtronhub.io
* ability to reference an external Space via address i.e. mechtronhub.io::uberscott.com:my-favorite-mechtron:1.0.0:/Mechtron.wasm
* ability to tag an external Space i.e. [hub]::uberscott.com:my-favorite-mechtron:1.0.0:/Mechtron.wasm
* create a space based on an email address: mechtronhub.io::scott@mightydevco.com:my-mechtron:1.0.0:/Mechtron.wasm

* user accounts (this may be a command line thingy)
  - ability to create a new account
  - ability to login
  - email verification 
  - domain verification to prove that user is controller of domain

* bundles
  - ability to publish a resource bundle (cli)

* REST api
  - publish a REST api for accesing ArtifactBundles so that languages other than Rust can download and access 
  - rust client library for new rest API for downloading and unzipping ArtifactBundles as well as caching Artifacts 
  - incorporate rust client library into starlane itself 

### STARLANE IN JAVASCRIPT [backlog]
* ability to use MechtronHub (to download & unzip packages)
* compilating and execution starlane-wasm-portal in a Wasm guest
* example project with starlane-paralax
* example project with bindgen


### PRODUCTION version: 1.0.0-beta [backlog]
The first version where Starlane is ready for production environments.
* support for larger binaries
  - Implementation of the 'Data Conveyor' (Starlane's support for large files & messages)
* Lot's and Lot's of testing the robustness of the app


### PRODUCTION version: 1.0.0 [backlog]
* Lot's and Lot's of more testing....

### MECHTRON HUB 2 (for artifact bundles) [backlog]

* bundles webpage
  - artifact bundle metadata (name, description,image,authors) bundle.yaml file
  - an html page for each ArtifactBundleVersion's sourced from {{ bundle }}/README.md
  - page has different links to previous versions of ArtifactBundle

* specific
  - ability to publish a 'Specific' specific.yaml: title, authors, meta data, search keys, artifact references, README.md (which renderes a page) 

* search
  - ability to apply searchable keywords to an ArtifactBundle (in bundle.yaml)
  - ability to search hub for a particular ArtifactBundle
  - SearchKey: vendor:[MySql], product:[PostgreSQL], website:[mysql.org], solution:[Database], solution:[Message Broker], author:[Scott Williams], 


### CLIENT SIDE STARLANE [backlog]
* implementation of WebSocket for message passing messages between a starlane client and server 
* introduction of client-side Mechtrons
* client side libaries for running starlane inside iOS and Android


### KUBERNETES AND DATABASES [backlog]
* Introduction of the Starlane Kubernetes Operator 
  - ability to specify the number of stars that serve mechtrons
* resource registry persistence (before resource registry was handled by in-memory SQLite)
* ability to provision a Database
* ability to extend Starlane's provisioning system through the Starlane Kubernetes Operator in order to support any relational Database that runs in a docker container
* ability to send an SQL message to a Database instance and receive a response

* App Bind:
  - New InstallStatus for App (which may later become an UpgradeStatus)
  - App can ensure the creation of required resources (like a <Database>, <FileSystem> or a <Mechtron<Util>> ) before it is in it's Ready state
  - App can 'require' provisioners (like MySql) or deployment will Fail Installation)

* Mechtron Bind Introduction:
  - bind can 'require' specific types of resources to be available before it is Ready 
  - bind can refer to a resource by it's context name and it will be replaced with actual address 
  - pipeline DSL for message ports
  - pipeline DSL for http requests & responses


### REMOTE SERIALIZATION & API [backlog]
* create a serialization for accessing resources remotely 
* autogenerate library serializations for common languages like Rust, Java, C#, JavaScript, Python, Ruby, etc.
* mechanism for negotiating serialization version
* mechanism for downgrading serialization on the server side if the client has a lower max version
* implementation of API in RUST
* implementation of API in Java
* ability to watch for changes in Starlane resources from the new APIs
* ability to Lock & Unlock Resources

* Mechtron Bind Improvements:
  - Addition of Payload block to pipelines
  - Inbound request is required to match request Payload pattern
  - Outbound response is required to match response Payload pattern
  - Payload blocks can enfornced Welformedness to a Schema for Text & Bins.  For example a Pipeline Payload Block may verify that a Bin payload is an Image format, and another might verify that a Text payload is wellformed json 
  - Payload Structured Schema Validation: a payload can be validated according to a Schema document provided via an Artifact such as Json Schema
  - A Payload can have be validated by a utility Mechtron in order to handle any type of custom Schema


### SCHEDULER [backlog]
* addition of the of the Scheduler, Cron, Timer & Future resources 
* addition of a Broadcast resource which broadcasts messages sent to it to all watching resources
* ability to create a Cron resource which will send a message to another resource at a configured time
* abillity to create a Timer resource [Fixed & Delayed] which will send a message to another resource repeatedly based on an interval
* ability to create a Future which will send a message after it receives a response from a potentially long running message. It can also be configured to timeout

### QUEUES & WORKERS [backlog]
* addition of a Queue resource which will consume and hold messages
* addition of a Worker resource which will pull messages from a Queue and send them to another reasource and wait till it receives a response before pulling from the Queue again

### OPERATIONS [backlog]
* ability to create an Operation resource which has pre-defind child resources known as Task, Wave and TaskResult
* Operation is bound to a mechtron that builds a Task list called a Wave when it is invoked with parameters
* Operation executes each task in its Wave and produces a TaskResult
* Only one version of the operation can be excecuting a Wave at any given time
* Tasks have the ability to execute Mechtrons for processing
* add ability for Operations (and resources in general) to have a 'circuit' which can be tripped making the resource inaccessable for a configured amout of time or until a condition is met... this will be useful for operations that do things like download large amounts of data when they encouter a 'rate limit' which requires pausing the operation for a while.

### EXTERNAL SERVICE [backlog]
* ability to hook an external Service to Starlane via the Starlane Kubernetes Operator  [RestService,WebService,etc]
* ability to exchange messages with the external "Service Resource"  
* ability to add a router proxy pass to an external WebService 
* example of a Wordpress site being served through Starlane web 

### STATEFUL MECHTRONS [backlog]
* Mechtrons can be configured as Stateful
* new configuration rules for mechtrons governing state persistence [None, ReplicateToFile] 
* ability to replicate Mechtrons accross hosts in order to survive individual machine crashes
* creation of 'Mechtron Cache API' which allows mechtrons to cache work between message requests
  - mechtron can specify a memory block to 'cache'
  - host will copy the memory block out of the mechtron
  - when the memory block is needed again the mechtron can request to copy it back in
  - at the end of the request the memory block will be once again evicted
  - mechanism needed for a mechtron to 'release' it's claim on cached memory

### SCALE [backlog]
* Kubernetes operator gains ability to distribute stars accross multiple containers
  - ability to 'resize & reshape' the Starlane cluster based on Kubernetes resource definition 
* Sharding of resources
  - Multiple Database clusters & FileSystem providers etc, can be available
  - Provisioners will spread creations accross available hosts
  - ability to 'move' resources from one host to another
* Slow Lanes: creation of a new type of 'lane' that delivers less urgent messages (like Logs)
* Mesh Star Sharding: ability to shard the mesh star 
  - reduce traffic by load ballancing accross lanes
  - increase redundancy in case an individual Mesh Star crashes
* Resource state replication

## DONE

### MORE MECHTRONY GOODNESS version: 0.2.0 [done]
* ability for Mechtrons to create new resources
* ability for Mechtrons to message other resources
* ability for Mechtrons to message each other
* split Mechtron guest framework into a new repository
* create a new example mechtron [upload,display profile pic]
* ability to serve a static site via an ArtifactBundle
* ability to tag a specific artifact (such as tagging it to the 'latest' version) so that router configs can refernce by tag instead of exact version number
* FIX: return proper HttpResponse headers
* FIX: some issues with router cause it to crash
* improvement of router configuration to make it look more like bind configuration 



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


