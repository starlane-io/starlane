# MECHTRON SKEL
This is a starter template for creating a Mechtron using the Rust language.  


What is a Mechtron?  It's WebAssembly that implements the Mechtron framework allowing it to connect to a Mesh Portal via messaging.  

Presently the only implementation of the Mesh Portal is Starlane which you can install and learn more about here: [http://starlane.io](http://starlane.io)

# GENERATION
To generate this rust project make sure you have cargo generate installed:

```bash
cargo install cargo-generate
```

Then run cargo generate:

```bash
cargo generate https://github.com/mechtronium/mechtron-skel.git
```

# BUILD YOUR MECHTRON
You can build a mechtron in `debug` or `release` modes:

```
make release
```

This will also zip up the Mechtron config and bind located in bundle/config & bundle/bind respectively.  


# DEPLOYING
There is a nice tutorial on starlane.io that describes how to deploy a Mechtron as an App: [http://starlane.io](Starlane Tutorial) (in Lesson #3 it references THIS github repository showing how to generate & customize a Mechtron.)


# EXAMPLE DEPLOYMENT SCRIPT







