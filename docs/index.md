## Welcome to Star Lane

Starlane is a framework for implementing a highly distributed [actor model](https://en.wikipedia.org/wiki/Actor_model). 

Starlane can simultaneously serve multiple hetrogenius actor-based applications as well as distribute the actors accross multiple server nodes in a cluster.

A Starlane application can be extended into the client realm via a websocket gateway allowing for easy message passing throughout the entire application stack.

### How it works
Starlane allows for provisioning of actors over a cluster of 'stars' and provides messaging between actors via 'lanes.'

Within every starlane "galaxy" everything starts with the "Central" star.  There is one and only one Central star in a starlane galaxy cluster.   The Central star manages the creation, assignment and handles lookups for Applications.

When a new starlane application is created that application will be assigned by Central to a Supervisor star.  A Supervisor star provisions and tracks actors within it's local Server constellation.

A Server star hosts the actual actors. 

In order to faciliate message passing between stars there is also a Mesh star which does little more than relay messages.   

Finally there are three types of stars that allow for clients to interface with Starlane:  The Gateway, Link and Client stars.

A Gateway star works as a Mesh star that also enforces some authentication and authorization.  A Link star merely connects to a Gateway via a Websocket.  

The Client Star works a lot like the Server star except it allows for the creation of client-side managed actors.  




