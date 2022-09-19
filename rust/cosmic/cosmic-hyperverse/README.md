# THE COSMIC HYPERVERSE
`cosmic-hyperverse` is part of [THE COSMIC INITIATIVE](http://thecosmicinitiative.io) a WebAssembly orchestration framework.

The Cosmic Hyperverse is the infrastructure component of The Cosmic Initiative framework that orchestrates and enforces
security.  It is responsible for making the universe painless to extend by supplying goodies such as provisioning, 
sharding, load balancing, routing, discovery & of course security.

### WORK IN PROGRESS
*this framework is a work in progress and not ready for production. And it is not yet fully documented for feedback and discussion .*
*right now there is little in the way of Drivers other than creating a few generic Particles and passing Waves between them* 

## BUILDS ON THE COSMIC UNIVERSE
The concepts in the `cosmic-hyperverse` package build upon the concepts in the [cosmic-universe](../cosmic-universe/README.md) 
package so familiarity with The Cosmic Universe is recommended before grappling with Hyperversal concepts.

## TERMS
To avoid name collision with other domain many concepts in The Cosmic Initiative are 
given names from Astro Physics concepts:

* **Hyperverse** - A Platform implementation of The Cosmic Initiative.
* **Star** - A node/container for managing state and execution of Particles.  
             The Hyperverse distributes its provisioning of Particles amongst its Stars in order to spread computation 
             load for storage, cpu & memory.
* **Lane** - Stars are connected via Lanes which serves as the transit mechanism for Waves 
* **Machine** - Although Stars are the 'node' component for managing Particles--the stars are more of 'virtual nodes' 
                that live inside a Machine. The Machine will connect the internal Lanes between Stars, provide a service Stars 
                within other Machines to connect & the Machine manages clients on behalf of its stars that are required to connect 
                to other external Stars.   This architecture facilitates the rearrangement of infrastructure without the Stars needing 
                any special knowledge of the Hyperverse cluster that it resides in.  For example in the standalone configuration 
                ALL of the Stars execute on one Machine, and in yet another configuration each Star may have it's own Machine but 
                in both cases the Stars see the Hyperverse the same without needing any special knowledge of how the Hyperverse 
                cluster is composed.
* **Registry** - The Registry holds important information on where a Particle lives and security rules. It is used
                 by the Hyperversal dimension to route Waves and synchronize the provisioning of Particles
* **Driver** Particles are supported through Drivers.  Each Kind has exactly one Driver.  Drivers dwell within Stars

## HYPERVERSE COMPOSITION
To create a new Hyperverse composition (which means you are creating a new Platform) you need to implement the 
Hyperverse trait.  An incomplete example can be found in the [test package](src/test/hyperverse.rs). It's incomplete
because not every feature is needed for testing at this moment, however, it does show how the DriverFactories are
created and a basic in memory registry.   

## MACHINE
A Point of interest is how the [Machine](src/machine.rs) works.  It relies heavily on the Hyperverse to provide it with all of its
customizations. 

## DRIVER EXAMPLE
A ver simple example of a Driver is the [BaseDriver](src/base.rs).  It does nothing but allows other particles
to be created as children in it's point hierarchy.

## MORE TO COME
More documentation will  be forthcoming on the `cosmic-hyperverse` as it is tested in it's first [Starlane](http://starlane.io) 
reference implementation

