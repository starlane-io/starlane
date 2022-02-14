
# DOCUMENTATION

## GETTING STARTED

### INSTALL RUST
To build Starlane you will need to have **rust** installed.  Follow the official Rust instructions to [install Rust](https://www.rust-lang.org/tools/install).

### NIGHTLY TOOLCHAIN
Starlane requires the `nightly` toolchain for compilation.  To switch to the `nightly` toolchain:

```bash
rustup toolchain install nightly
rustup default nightly
```

### BUILD AND INSTALL STARLANE

```
cargo install starlane
```

Congrats! You now have Starlane installed on your machine!

### START A STARLANE SERVER INSTANCE
Open a terminal and run the following command to start a server instance of Starlane:

```bash
starlane serve
```
At this point starlane should be serving a Http Server on port 8080.  Open a browser and point it to [http://localhost:8080/](http://localhost:8080/).  You should see a "404" page (since there isn't a localhost space or routing bind.)

NOTE: Starlane works most of the time, however, this software is still in development and about 1 out of every 20 runs Starlane has a failure causing the example to break.  If you notice a problem, maybe try rerunning the example from the start.


## TUTORIAL
To learn more please follow the online [Tutorial]({{< ref "/docs/tutorial" >}} "Tutorial").




