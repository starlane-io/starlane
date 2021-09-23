# STARLANE
Starlane is a RESOURCE MESH which can also execute client and server side WebAssembly. You can read more about what Starlane is and what it does on Starlane's [about page](http://starlane.io/about/).

## A WORK IN PROGRESS 
Right now Starlane is little more that a toy since there is no way to connect an external service to it--althought that is being worked on and will be availble next.  For now you can do the following with Starlane: 

* Run a local Starlane server instance
* Connect to the server instance via the Starline CLI
* Create a FileSystem
* Upload and/or download a File
* Watch a file for changes

## GETTING STARTED
To build starlane you will need to have rust and make installed.  Follow the official Rust instructions to [install Rust](https://www.rust-lang.org/tools/install).

You will need **Make** installed to execute the Makefile.  Sorry, you will have to figure that out yourself!

## INSTALLING
to install Starlane run this in the directory where you checked out this repository:

```bash
make install
```

## RUNNING
So here's the fun part when you actually get to play with Starlane.  

Let's run it in the same directory where you checked out the Starlane source code.  (NOTE: when the starlane server runs, it will automatically create a new local directory called 'data' You can delete this directory aftter it is finished running)

First start a Starlane server instance:

```bash
starlane serve
```

The command should appear to do nothing (no output is printed, it's just waiting for connections.)

Next, we are doing to create a new FileSystem under the default space called creatively 'space'.  You must open up a brand new terminal so as not to terminate the running starlane server.  In your new terminal run:

```bash
starlane create "space:filesystem<FileSystem>"
```

It should print some output and exit.  Notice we pass the name we want 'filesystem' and the type <FileSystem>.

Let's check to see if that filesystem was actually created by listing the contents of 'space':

```bash
starlane ls space
```

You should expect to see that space has one child resource, the FileSystem you just created.

Okay, now how about uploading a file!  Since you are in the repository directory lets upload this very README.md file:

```bash
starlane cp README.md "space:filesystem:/README.md"
```

So that is the 'cp' command or copy command.  You can also download files in the way you would expect:

```bash
starlane cp "space:filesystem:/README.md" README.md.copy
```

Now we have come to the fun part and the part that makes Starlane really special.  Let's Watch the file for changes.  To do so we run:

```bash
starlane watch "space:filesystem:/README.md"
```

And like when we ran the server we sacrifice this terminal as it now is just listening for changes.  

To test watch open up a new terminal and run:

```bash
starlane cp README.md "space:filesystem:/README.md"
```

Of course the file is already there, but it will be written to again and will therefore trigger a state change event.

If you observe you the watching terminal you should see that it prints out a message: **received notification: State**





















