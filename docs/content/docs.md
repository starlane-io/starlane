
# DOCUMENTATION

## GETTING STARTED

### INSTALL RUST
To build starlane you will need to have **rust** and **make** installed.  Follow the official Rust instructions to [install Rust](https://www.rust-lang.org/tools/install).

### INSTALL MAKE
You will need **Make** installed to execute the Makefile.  This should be installed by default on Mac and other Unix based OSes. Sorry, if you are a windows user you will have to figure out how to install make yourself!

### INSTALL WASM PACK
In order to build executable WebAssembly actors knowns as Mechtrons, you will need to install wasm-pack.  Follow the [wasm-pack installation instructions](https://rustwasm.github.io/wasm-pack/installer/) for your platform. 


### BUILD AND INSTALL STARLANE
To install Starlane run ```make install``` in the directory where you checked out this repository:

```bash
make install
```

Congrats! You now have Starlane installed on your machine! Why don't you try running the example next?

## RUN THE EXAMPLE
The following is a simple runnable example that illustrates the basic utility of Starlane.  

NOTE: Starlane works most of the time, however, this software is still in development and about 1 out of every 20 runs Starlane has a failure causing the example to break.  If you notice a problem, maybe try rerunning the example from the start.

### START A STARLANE SERVER INSTANCE
Open a terminal and run the following command to start a server instance of Starlane:

```bash
starlane serve
```

At this point starlane should be serving a Http Server on port 8080.  Open a browser and point it to [http://localhost:8080/](http://localhost:8080/).  You should see a "Welcome" message that also indicates that the 'localhost' space has not yet been created.

### CREATE LOCALHOST SPACE
The webserver takes the Host directive from an http request header and uses it to determine which Space's router configuration to use.  Since we are doing local development we need to create a Space named 'localhost'.

Open a NEW terminal (since your previous terminal is still running the starlane server.)

```bash
starlane create "localhost<Space>"
```

You can see that we are naming the resource 'localhost' and we use the greater than/less than delimeters to indicate which type we want to create. 

Now refresh your browser pointed to [http://localhost:8080/](http://localhost:8080/)

You should see a new message reading "The 'localhost' Space is there, but it doesn't have a router config assigned yet."

### CREATE A FILESYSTEM
We want a place where we can upload and serve files, so let's provision a filesystem:

```bash
starlane create "localhost:my-files<FileSystem>"
```

You can see we are creating a filesystem which is a child resource of 'localhost' called 'my-files' and again we pass the type as FileSystem.

### UPLOAD A FILE
Let's upload a file (which will serve as our entire website)  from the example directory we want to upload 'example/websites/simple-site1/index.html''

IMPORTANT: you MUST run this command from the directory where you checked out the starlane git respository

```bash
starlane cp expample/websites/simple-site1/index.html "localhost:my-files:/index.html"
```

Here you can see we are uploading file index.html to a File newly created File resource which is a child of the FileSystem 'my-files.'

### ROUTER CONFIG
Before we go on to the next step lets take a moment to look at the router config we want to apply to our localhost.

Here are the contents of example/localhost-config/routes.conf

```
GET /files/(.*) -> localhost:my-files:/$1;
GET /app/ -> localhost:my-app:main;
```

Let's take a look at the first directive `GET /files/(.*) -> localhost:my-files:/$1;`   This is telling the router to take any GET request whose path matches the regex pattern ``` /files/(.*)  ``` Keen regex experts will notice that the parenthesis is a regex capture. 

The -> points to the starlane resource address to route the request, and this is where the regex captures are used, the $1 takes the value from the capture and appends it to the resource path.

### PUBLISHING AN ARTIFACT
Now we need to get the routes.conf into starlane so the router can actually use it.

To do so we need to publish an artifact bundle.  An artifact bundle is a versioned zip file containing configurations and assets.

So let's publish version 1.0.0 of our resource bundle by running this command:

```bash 
starlane publish ./example/localhost-config "localhost:config:1.0.0"
``` 

The 'publish' command takes a directory and resource path as an argument and automatically zips up the contents of the directory and publishes as an ArtifactBundle to the resource path.

Notice that 'config' is the artifact bundle series name and version '1.0.0' is the version. ArtifactBundle path's are required to follow the convension of 'artifact-bundle-series-name:semver'

### BINDING THE CONFIG TO LOCALHOST
For the filesystem to be accessable by the Http server the router config must be bound to the localhost space.

```bash 
starlane set "localhost::config=localhost:config:1.0.0:/routes.conf"
```

Refresh the browser pointed at [http://localhost:8080/](http://localhost:8080/).  Now you should see a message saying 'CONFIGURED' indicating that localhost is configured. 

And now the fun part:  change your browser location to [http://localhost:8080/files/index.html](http://localhost:8080/files/index.html) And you should see a message saying "SOMETHING DIFFERENT" which is the entire simple-site1 webiste.  

You have now served your first static file resource from Starlane.

### AN APP AND A MECHTRON
Static files are fun but you can't do much with them because they are static.  Starlane has another type of resource called a Mechtron which is a framework for executing client and server side WebAssembly. 

In Starlane an App resource is composed of Mechtrons which can handle http requests, access other resources and message other mechtrons.  

Let's build and deploy an App composed of a single Mechtron and serve some mechtron content on our webserver.

### BUILDING THE MECHTRON
It's beyond the scope of this guide to explain how to create a Mechtron, so we are going to build and deploy a preexisting example located in example/app


Run the Makefile:

```bash
cd example/app
make all
cd ../..
```

### PUBLISH THE APP AND MECHTRON ARTIFACT BUNDLE
The build process creates a Wasm file and lays out the configuration files in a directory called 'example/app/bundle'  Next we will publish that bundle so it can be referenced by our app:

```bash
starlane publish example/app/bundle "localhost:app-config:1.0.0"
```

### CREATE THE APP
Finally we will create our App which by configuration references the mechtron we just built.  

```bash
starlane create "localhost:my-app<App>" "localhost:app-config:1.0.0:/app/my-app.yaml"
```

Here we have created an App resource called my-app using the usual syntax, but additionally we have passed a reference to "ocalhost:app-config:1.0.0:/app/my-app.yaml" which is the configuration artifact for the app which we just published.

A little reminder, the second line in the routes.conf file looked like this: 
```
GET /app/ -> localhost:my-app:main;
```

Which  was telling the router to take the /app/ path and route it's request to the 'main' mechtron of 'my-app'

So let's see it!  Point your browser to [http://localhost:8080/app/](http://localhost:8080/app/)

You should see a very different looking webpage saying 'MECHTRON' with the footer saying "This page was served by a Mechtron."

### MORE TO COME
Congratulations! You have just deployed a static website and a dynamic mechtron in Starlane.

Stay tuned for more features and better examples of dynamic Mechtrons that can interact with eachother..



































