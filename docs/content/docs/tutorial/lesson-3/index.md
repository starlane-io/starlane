---
title: "Lesson 3"
date: 2022-02-12T21:29:31-06:00
draft: false 
---

# LESSON 3 -- CREATE AND DEPLOY A DYNAMIC APP

The source code for this lesson can be found on github here:

[https://github.com/mechtronium/starlane/tree/main/tutorial/lesson-3/](https://github.com/mechtronium/starlane/tree/main/tutorial/lesson-3/)


### GENERATE A BOILERPLATE MECHTRON

Make sure you have cargo *generate* installed:

```bash
cargo install generate
```

Next generate the mechtron:

```bash
cargo generate --git https://github.com/mechtronium/mechtron-skel.git --name my-mechtron
```

### MODIFY THE CODE
By default the generated Mechtron will return a response error for every type of request.   We are going to change it so that It will process an Http request.

Fine the Http Request handler code:

```rust
    /// Write custom Http request handler code here
    fn handle_http_request(
        &self,
        ctx: &dyn MechtronCtx,
        request: HttpRequest,
    ) -> Result<ResponseCore, Error> {
        Ok(request.fail(format!(
            "Mechtron '{}' does not have an Http handler implementation",
            ctx.stub().address.to_string()
        ).as_str()))
    }
```

And let's modify it to return a string based on the submitted path:

```rust
    fn handle_http_request(
        &self,
        ctx: &dyn MechtronCtx,
        request: HttpRequest,
    ) -> Result<ResponseCore, Error> {

        let mut name = request.path.clone();
        name.remove(0); // remove leading slash
        let response = request.ok( format!("Hello {}",name).as_str() );

        Ok(response)
    }
```

It's not an award winning HTML response, but you get the point.

### BIND & CONFIG
A ready made config and bind exist to run this Mechtron as an App in bundle/config and bundle/bind respectively.

### CREATE A NEW LOCALHOST BIND
We need to recreate our localhost bind for this project since the generator doesn't do that.

Create a new file called *bundle/bind/localhost.bind* with the content:

```
Bind {

  Http {

    <Get>/(.*) -> localhost:my-app^Http<Get>/$1 => &;

  }

}
```

As you can see in this case we are directing all Http traffic to my-app

### CREATE A DEPLOYMENT SCRIPT

create new file called *install.script* with the following contents:

```
? create localhost<Space>;
? create localhost:repo<Base<Repo>>;
? create localhost:repo:tutorial<ArtifactBundleSeries>;
? publish ^[ target/bundle.zip ]-> localhost:repo:tutorial:3.0.0;
set localhost{ +bind=localhost:repo:tutorial:3.0.0:/bind/localhost.bind };
? create localhost:my-app<App>{ +config=localhost:repo:tutorial:3.0.0:/config/mechtron.app,
                                +bind=localhost:repo:tutorial:3.0.0:/bind/mechtron.bind };

```

Now point your browser to http://localhost:8080/YourName  and you should see a rather plain looking HTML page that says "Hello YourName."

### AESTETIC IMPROVEMENT
Let's improve the page by using a templating engine to render a properly formatted HTML page.

First copy this file from your checked out starlane repository: tutorial/lesson-3/wasm/my-app/src/html.rs and place it in your my-mechtron project in the src directory. This is a nifty little bit of rust code that uses a templating engine called handlebars to render an HTML page with some custom output.

Next we need to update the Cargo.toml file and add a few dependencies.

Open Cargo.toml file and you should see a dependencies section that looks like this:

```toml
[dependencies]
mechtron= "0.2.0-rc1"
mechtron-common= "0.2.0-rc1"
wasm_membrane_guest = "0.2.0"
mesh-portal = "0.2.0-rc1"
```

We are going to add 3 dependencies so the final section looks like this:

```toml
[dependencies]
mechtron= "0.2.0-rc1"
mechtron-common= "0.2.0-rc1"
wasm_membrane_guest = "0.2.0"
mesh-portal = "0.2.0-rc1"

handlebars = "4.2.1"
serde_json = "1.0.79"
lazy_static = "1.4.0"
```
Now let's go back to lib.rs and incorporate our new utility:

First add the following 'use' statement near the top of the lib.rs file:

```rust
use crate::html::greeting;
```

Next modify the handle_http_request function so it calls the greeting function:

```rust
    fn handle_http_request(
        &self,
        ctx: &dyn MechtronCtx,
        request: HttpRequest,
    ) -> Result<ResponseCore, Error> {

        let mut name = request.path.clone();
        name.remove(0); // remove leading slash

        match greeting(name.as_str() ) {
            Ok(response) => {
                Ok(response)
            }
            Err(err) => {
                Ok(request.fail("Rendering Error" ))
            }
        }

        Ok(response)
    }

```

You will need to modify the install script in order to increment the ArtifactBundle version (of course you can also just restart the starlane server):

```
? create localhost<Space>;
? create localhost:repo<Base<Repo>>;
? create localhost:repo:tutorial<ArtifactBundleSeries>;
? publish ^[ target/bundle.zip ]-> localhost:repo:tutorial:3.1.0;
set localhost{ +bind=localhost:repo:tutorial:3.1.0:/bind/localhost.bind };
? create localhost:my-app<App>{ +config=localhost:repo:tutorial:3.1.0:/config/mechtron.app,
                                +bind=localhost:repo:tutorial:3.1.0:/bind/mechtron.bind };

```

Now checkout https://localhost:8080/YourName and you should see the same greeting in a very red and stylized Mechtron page.





