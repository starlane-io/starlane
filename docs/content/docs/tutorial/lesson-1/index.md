---
title: "Lesson 1"
date: 2022-02-12T21:29:31-06:00
draft: false
---

# LESSON 1 -- DEPLOY A STATIC WEBSITE

The source code for this lesson can be found on github here:

[https://github.com/mechtronium/Starlane/tree/main/tutorial/lesson-1/](https://github.com/mechtronium/Starlane/tree/main/tutorial/lesson-1/)


## START A STARLANE SERVER

Open a terminal and start serving a Starlane instance:

```bash
Starlane serve
```

Open a browser window and point it to `http://localhost:8080/index.html` --  you should see a STARLANE 404 page which is expected since we haven't configured anything yet.

## CREATE THE LOCALHOST SPACE
Next we are going to use the Starlane command line to issue a command to create the 'localhost' space.  A **Space** in Starlane is both the top of the address hierarchy used for Starlane's messaging AND the host or domain name.  Since we are running locally we will create a `localhost` space.

```bash
Starlane exec "create localhost<Space>"
```

Let's break the last command down before we move on:

First when we want to send a command to Starlane we use the subcommand `Starlane exec` and then we put the actual command in quotes.      

``` 
       Space address
 command   |      Type 
|--^-| |---^---||--^--|
create localhost<Space>"
```

Here we issue the `create` command to create a new resource.  The resource's address is simply going to be `localhost` and the resource type is in angle brackets which is a `Space` type.

We can see that if the space was created successfully by running the `select` command like so:

```bash
Starlane exec "select *"
```

The output should look something like this:
```
space<Space>
localhost<Space>
```

The `space` Space is automatically created by Starlane on startup.

## REPO & ARTIFACT BUNDLES

You may notice that we are still returning a 404 page, that's because the localhost Space lacks a Bind--which is a configuration that tells Starlane how the resource should process messages (including Http requests.)  Next We will create a bind file and package it in an artifact bundle which can be deployed to a Starlane repository-- we will also create a simple HTML page to be served from the artifact bundle.

### THE HTML PAGE

In an empty directory create a subdirectory called `html`

```bash
mkdir html
```

And within the html directory create a simple `index.html` file with the content:

```html
<html>
  <head>
    <title>Static Page Served from Artifact</title>
  </head>
  <body>
    <p>This page is being served from an artifact.</p>
  </body>
</html>
```

### BIND FILE

Create another subdirectory called `bind` (on the same level as the `html` directory)

Create a file named `localhost.bind` with the content:

```
Bind {

  Http {

    <Get>/(.*) -> localhost:repo:tutorial:1.0.0:/html/$1 => &;

  }

}
```

So once again, let's break some things down before we move on.   

First you can see the `Bind` selector which identifies that this is a Bind file... Similarly the `Http` selector identifies the some scoped content as http configuration...

Let's break down the most interesting line:

```
    <Get>/(.*) -> localhost:repo:tutorial:1.0.0:/html/$1 => &;
```

This is a configuration for a request pipeline. The first part is the Request Pattern `<Get>/(.*)`  If the incoming request matches this pattern then it will be forwarded through the pipeline.

```
method      
  |    path regex  
|-^-||-^-|
<Get>/(.*)        
```


The first part of the Request Pattern is `<Get>` which represents the Http Method we are trying to match.  The second part of the pattern `/(.*)` is for matching the Http Request Path. This is actually a proper Regex for matching any path and importantly the parenthesis indicate to perform a CAPTURE on that portion of the path. If a request came in such as `<Get>/index.html` the regex would capture the string `index.html`.  

```
                ArtifactBundle address segments
pass request operator      | 
        |                  |         Artifact address segments
       |^|  |--------------^------------||--^----|  
        ->  localhost:repo:tutorial:1.0.0:/html/$1   
```

Next we have the first Pipeline *Step* which is a simple `->` arrow. This particular pipeline step indicates that Starlane should pass the request on without any modification.

Next we have the first Pipeline *Stop* which points to a resource address `localhost:repo:tutorial:1.0.0:/html/$1`.   This may be confusing because we haven't created this resource yet, we will be doing that next and it will become more clear how the addressing system works.  For now just notice the `$1` at the end of the address which is a regex expression telling the pipeline to replace with the first capture from the Request Pattern.   So again if the http request was `<Get>/index.html` then `index.html` would be the captured string and the address would be resolved to `localhost:repo:tutorial:1.0.0:/html/index.html`.

Finally the `=>` pass response Pipeline Step says to pass whatever response to `&` which means return the response to the requester.

```
pass response operator  
        |   return to requestor
       |^| |^|
        =>  &
```

### ZIP UP THE BUNDLE
You should have two nice directories `html` and `bind`.  We need to zip them up into a bundle so we can publish this content for Starlane to use.

```bash
zip -r bundle.zip .
```

## REPO & ARTIFACT BUNDLE SERIES

Next we need to create some address segments which to some extent act like special folder/directories in Starlane since an Artifact Bundle is required to be uploaded to a special versioned address.

Create a Repo Base:
```bash
Starlane exec "create localhost:repo<Base<Repo>>"
```

In this case we are creating a `Base` (which is kind of like a directory) -- it's just a resource for containing other resources.   Notice that base takes a Kind which is akin to a SubType.  The Kind is `Repo` which indicates that only resources of type ArtifactBundleSeries will be allowed in this Base.

Now let's create an ArtifactBundleSeries:
```bash
Starlane exec "create localhost:repo:tutorial<ArtifactBundleSeries>"
```

An ArtifactBundleSeries will only accept a series of versioned ArtifactBundles.

## PUBLISHING AN ARTIFACT BUNDLE

Finally we can now publish the `bundle.zip` we created with this command:

```bash
Starlane exec "publish ^[ bundle.zip ]-> localhost:repo:tutorial:1.0.0"
```

The `publish` command is a special variant of the `create` command which understands that we are creating an ArtifactBundle (which is why we don't need to explicitly include the Type after the address)

After publish we see the use of the arrow notation `^[ bundle.zip ]->` Pipeline Step Block.  The `^` symbol indicates that we want to upload a file and make it the body of our command request.  

Since `^[ bundle.zip ]->` points to our newly created address `localhost:repo:tutorial:1.0.0` bundle.zip will become the stateful content of that address.  (And the ArtifactBundle resource knows how to extract the zip file and create addressable Artifacts for each file.)

## SETTING LOCALHOST'S BIND

We now have everything in place to configure localhost's routing.

Use this command to set the `bind` property of localhost:

```bash
Starlane exec "set localhost{ +bind=localhost:repo:tutorial:1.0.0:/bind/localhost.bind }"
```

This time the command is `set` and the address is `localhost`.  The curly brace scope indicates we are going to manipulate the resource properties.  `+bind` creates a new property called `bind` and sets its value to  `localhost:repo:tutorial:1.0.0:/bind/localhost.bind` which should make some sense to you at this point.  `localhost:repo:tutorial:1.0.0` is the ArtifactBundle we just published and you can see that the trailing portion of the address references the bind file we created inside the ArtifactBundle: `:/bind/localhost.bind`.

Try refreshing your browser location `http://localhost:8080/index.html` this time you should see the HTML page you created and bundled before.

## SUMMARY

You can deploy an entire static website through an ArtifactBundle.  And in the next tutorial we will cover how to combine the static webpage you just created along with a dynamic piece of WebAssembly code known as a Mechtron App.


[NEXT : DEPLOY A 'HELLO WORLD' APP]({{< ref "/docs/tutorial/lesson-2" >}} "Lesson 2")

