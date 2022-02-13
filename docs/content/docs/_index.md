
# DOCUMENTATION

## GETTING STARTED

### INSTALL RUST
To build starlane you will need to have **rust** and **make** installed.  Follow the official Rust instructions to [install Rust](https://www.rust-lang.org/tools/install).

### INSTALL MAKE
You will need **Make** installed to execute the Makefile.  This should be installed by default on Mac and other Unix based OSes. Sorry, if you are a windows user you will have to figure out how to install make yourself!

### BUILD AND INSTALL STARLANE
To install Starlane run ```make install``` in the directory where you checked out this repository:

```bash
make install
```

Congrats! You now have Starlane installed on your machine! Why don't you try running the example next?

### START A STARLANE SERVER INSTANCE
Open a terminal and run the following command to start a server instance of Starlane:

```bash
starlane serve
```
At this point starlane should be serving a Http Server on port 8080.  Open a browser and point it to [http://localhost:8080/](http://localhost:8080/).  You should see a "404" page (since there isn't a localhost space or routing bind.)

NOTE: Starlane works most of the time, however, this software is still in development and about 1 out of every 20 runs Starlane has a failure causing the example to break.  If you notice a problem, maybe try rerunning the example from the start.



[Getting Started]({{< ref "/docs/tutorial" >}} "Docs")




