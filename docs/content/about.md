# ABOUT STARLANE
Starlane makes it easy to deploy and interoperate with secure WebAssembly code in the cloud, the edge, desktop, mobile and IoT environments.

## A SERVER TO CLIENT MESH
Starlane is a special type of Mesh that spans from the server to client.  This special Mesh facilitates the execution of WebAssembly at every level. To accomplish this feat Starlane provides the following :
* Starlane host environments which can run on the client or the server and connect to other Starlane hosts
* An Artifact Bundle repository which allows resources to reference, download and cache configurations, executable Wasm code and assets.
* A simple to use message passing framework enabling WebAssembly actors to interact with any other resources connected to the Mesh.
* A Mesh Portal API for connecting non-WebAssembly backend micro services to the mesh.
* Extendable resources such as Databases, FileSystems, Message Queues etc-- all of which can process Starlane messages.
* A Simple to implement Framework which makes it easy for WebAssembly to interoperate with the host Starlane Mesh.

### WHY WEBASSEMBLY?
In case you haven't heard of WebAssembly, this section is for you to help you understand why WebAssembly is truly the future and something to get excited about!

WebAssembly is a binary instruction set that can be executed consistently and securely anywhere: meaning in the browser client and on the server almost any architecture and OS. It’s an amazing breakthrough that is already working on all the major browsers and all the major Os and architecture combinations. If you want to learn more about WebAssembly check it out on the WebAssembly.org website.

One reason that WebAssembly is secure is that it is executed in a “host” environment, which also serves as its sandbox. WebAssembly cannot by itself access any files or connect to the network. A WebAssembly program cannot even create a thread and update itself automatically, it relies upon its host to provide it with data and execution time. The host also provides an interface for WebAssembly to call for additional custom interaction between the host and the WebAssembly guest.

This brings us back to one of the original problems Starlane was created to solve: WebAssembly needs a standard way to interact with varied enterprise resources. And to that end the Mechtron was invented…

### MECHTRONS
The Mechtron is an open source framework of the [Actor model](https://en.wikipedia.org/wiki/Actor_model).  Starlane implements the host Mechtron interface and can communicate to any WebAssembly binary that was compiled with an implementation of the guest Mechtron interface. 

Additionally a Mechtron standard defines:
* A bind configuration which tells the Resource Mesh what kind of messages the Mechtron can receive and the schema of the expected message payloads 
* A mechtron deployment configuration file which defines which WebAssembly binary artifact to execute and references a bind config artifact to implement as well as some additional custom parameters
* An application configuration which defines the mechtron initialization and composition of the app as well as some other required resources like FileSystems, Databases etc.

Importantly the WebAssembly binary can now use the Mechtron interface to access The Resource Mesh in order to access any Resource in the enterprise and talk to other Mechtrons in an agreed upon message payload serialization thanks to the bind references

An Application that executes within Starlane's Mesh can be composed of many multiple mechtrons that may be written by completely different people in completely different languages yet they will be able to communicate and coordinate with each other.  

And... The Application can even swap out mechtrons while it is running...  

And...  Tenants of a Starlane Cloud Service could be given permission to overwrite the default Mechtron that managed something specific to them: let's say for example users of a social media site would like to write custom code to render their Profile page. They could create a custom mechtron that implements the profile bind referenced by the application and then deploy and let the social medias sites Starlane instance securely execute their custom code infrastructure!  
Mechtrons are made to encourage creativity and collaboration through code!  The possibilities are very exciting!



## TRY IT OUT
Why not try out the [Getting Started]({{< ref "/docs" >}} "Docs") guide today where you can deploy Starlane, publish some static files to an Artifact Bundle, serve those files from an http server and then deploy and create an actual Mechtron and serve an html page from that mechtron.














