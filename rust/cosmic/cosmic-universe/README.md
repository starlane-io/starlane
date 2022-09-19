# COSMIC UNIVERSE
`cosmic-universe` is part of [THE COSMIC INITIATIVE](http://thecosmicinitiative.io) a WebAssembly orchestration framework.

Concepts within The Cosmic Initiative framework exist in one of two dimensions: Universal or Hyperversal.

## TERMS
To avoid name collision with other domain many concepts in The Cosmic Initiative are 
given names from Astro Physics concepts:
* Cosmos - Everything that the framework is connected to
* Particle - A Resource that can send and receive Waves and provides some sort of functionality.  
  Particles  have a Kind (Mechtron,Database,File,User,etc...)
* Point - an address usually identifying a Particle
* Wave - A message A wave shell contains routing: **to** & **from** and a variety of other fields 
         to describe handling and security. wave core is either **directed** or **reflected** (request or 
         response) and closely follows http request/response format.
* Mechtron -  A WebAssembly component that implements the `mechtron` & `cosmic-universe` framework and is 
              therefore enabled to discover and communicate with the other Particles in the Cosmos... 
              A Mechtron is also the mechanism for extending the functionality of ANY other by intercepting,  
              interpreting and communicating to the particles underlying resource in a way that it can understand.
* Control - An endpoint Particle for allowing external connections to the Cosmos.  The external Control implements the
            `cosmic-universe` package (or a auto generated serde model for other languages) to communicate with the Cosmos
* Cosmic Fabric - This is the space between the particles where Waves travel 
* Universal Dimension - a simplified view/api of the Cosmos that does not concern it self with security
                        or infrastructure issues
* Hyperversal Dimension - The infrastructure layer which enforces security and orchestration (provisioning, sharding, load balancing, etc). 
 
## UNIVERSAL DIMENSION
This package--the Cosmic Universe--provides an API and utilities for interacting with the 
Cosmic Fabric and other Particles within the Universal Dimension. 

The Universal dimension allows Particles to interact with the Cosmic Fabric and other Particles 
with almost no need to deal with Orchestration or Security.  Concerns like security, provisioning, 
sharding, load balancing and any other concepts that can be deemed common infrastructure concerns 
are managed in the background by the Hyperversal Dimension.

The purpose of this separation of concerns between the Universe and the Hyperverse is to push 
as much system complexity into a central location where all interaction is processed so that
there is less complexity (and variation) with the various Particles.  Particles with less
complexity means that they are easier to implement and there are fewer things that can
go wrong as there is less duplication of logic in each Particle.

Conversely [`cosmic-hyperverse`](https://crates.io/crates/cosmic-hyperverse) Is the package for
managing security and orchestration.

### SECURITY EXAMPLE
For example let's examine authorization between Particles.  We have a source Particle which
sends a Wave to a target Particle and the target Particle requires that directed Waves be authorized.

the source Particle simply sends the Wave without credentials or tokens or anything... 
It behaves as if there IS no security. Behind the scenes as the Wave traverses the 
Cosmic Fabric it enters the Hyperversal dimension and the source's authorization is checked 
and the Wave is either passed to the target or rejected and returned to the source as an error.

Note that the target Particle which is also part of the Universal dimension makes no effort to 
authorize the directed Wave--it simply assumes that any Wave it receives must be authorized or else 
it would not have been delivered.

Of course, the two dimensional isolation of concerns will collide in the case when the source Particle 
component does NOT have permission to Wave the target Particle. For this reason the Universe 
is not completely unaware of the Hyperverse, but at least there is a vast simplification on the code on both ends.

















