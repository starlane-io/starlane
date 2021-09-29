# ABOUT STARLANE
Starlane is a ***Resource Mesh*** that enables micro services to create, find, watch and use various micro resources across the enterprise as well as message other micro services.

Starlane also provides mechanisms for deploying, executing and connecting client and server side WebAssembly actors known as Mechtrons.

Understanding what Starlane is and does can be a bit confusing because of the duality of its missions of Resource Mesh AND ubiquitous WebAssembly host.  A little history clears it up somewhat:  The origin of the Starlane project was an attempt to create an environment for client and server side WebAssembly actors to deploy , to securely access network resources, observe network resources for changes and message other WebAssembly actors.  

In the journey to enable WebAssembly actors it became apparent that Starlane's proposition would be useful to traditional Micro Services as well and that is when the second concept of the Resource Mesh became a first class feature in Starlane.  

Let's start by explaining what a Resource Mesh is:

## WHAT IS A RESOURCE MESH?
An enterprise is composed of **Services**, **Resources** and the **Mesh** that binds them all together.  

You may have heard of Service Meshes before. Whereas the raw network is just a mechanism of information transmission the Service Mesh's innovation was that knowledge could be enshrined **between** things instead of in things. For example: In a Service Mesh the credentials for the database don't need to be known by various Micro Services. Trusted Applications can connect to the mesh and the Service Mesh will proxy that connection to the database with the correct credentials filled in.  This makes things HUGELY easier to configure since multiple Micro Services use the same database. 

Service meshes are amazing in many other ways.  Their varied benefits can be boiled down into a single commonality: Service Meshes move complexity from Micro Services--which there are many of--to the Mesh which there is one of. As a rule complexity is easier to manage in one place and removing complexity from the Micro Services means faster development time, easier to understand code and fewer bugs.

The Resource Mesh is meant to build upon the Service Mesh concept.  A Resource Mesh allows a Micro Service to find, create and access various "Micro Resources."  Micro Resources are smaller concepts than the typical Kubernetes Resource.... so for example a Kubernetes Resource might represet a Pod, a PersistenVolumeClaim etc, whereas a Resource Mesh might reference an individual File, a Database table, a MessageQueue and many more things.  We will refer to Micro Resources as plain old Resources for the rest of this document.

When a Micro Service creates a resource via a Resource Mesh it does not need to know anything about where that resource lives and in most cases doesn't need to know the specifics on how the resource is created.  It merely supplies a binding address to reference the resource later.  The resource itself can even be moved to a new location by the Resource Mesh without disturbing the services since the services will continue to refernce the resource using the bound path.

## WEB ASSEMBLY
Now let's talk WebAssembly.  WebAssembly is a binary instruction set that can be executed consistently and securely anywhere: meaning in the browser client and on the server.  It's an amazing breakthrough that is already working on all the major browsers.  If you want to learn more about WebAssembly check it out on the [WebAssembly.org website](https://webassembly.org/).

One reason that WebAssembly is secure is that it is executed in a "host" environment, which also serves as its sandbox.  WebAssembly cannot by itself access any files or connect to the network.  A WebAssembly program cannot even create a thread and update itself automatically, it relies upon its host to provide it with data and execution time. The host also provides an interface for WebAssembly to call for additional custom interaction between the host and the WebAssembly guest.

This brings us back to one of the orignal problems Starlane was created to solve:  WebAssembly needs a standard way to interact with varied enterprise resources.  And to that end the **Mechtron** was invented...

### MECHTRONS
The Mechtron is an open source standard implementation of the [Actor model](https://en.wikipedia.org/wiki/Actor_model) for interacting with a Resource Mesh and thus accessing the Resources managed by the mesh.  Starlane implements the host Mechtron interface and can communicate to any WebAssembly binary that was compiled with an implementation of the guest Mechtron interface. 

Additionally a Mechtron standard defines:
* A bind configuration which tells the Resource Mesh what kind of messages the Mechtron can receive and the schema of the expected message payloads 
* A mechtron deployment configuration file which defines which WebAssembly binary artifact to execute and references a bind config artifact to implement as well as some additional custom parameters
* A standard for referencing Artifact Bundles that may contain useful assets for the Mechtron--including in some cases the schema definition files used to serialize and deserialize message payloads
* An application configuration which defines the mechtron initialization and composition of the app as well as some other required resources like FileSystems, Databases etc.

Importantly the WebAssembly binary can now use the Mechtron interface to access The Resource Mesh in order to access any Resource in the enterprise and talk to other Mechtrons in an agreed upon message payload serialization thanks to the bind references.

An Application that executes within Starlane's Mesh can be composed of many multiple mechtrons that may be written by completely different people in completely differen languages yet they will be able to communicate and coordinate with each other.  

And... the Application can even swap out mechtrons while it is running...  

And...  Tenants of a Starlane Cloud Service could be given permission to overwrite the default Mechtron that managed something specific to them: let's say for example users of a social media site would like to write custom code to render their Profile page. They could create a custom mechtron that implements the profile bind referenced by the application and then deploy and let the social medias sites Starlane instance securely execute their custom code infrastructure!  

Mechtrons are made to encourage creativity and collaboration through code!  The possibilities are very exciting!


## TRY IT OUT
Why not try out the [Getting Started]({{< ref "/docs" >}} "Docs") guide today where you can deploy Starlane, upload some Files to a FileSystem, serve those files from an http server and then deploy an actual Mechtron and serve an html page from that mechtron.














