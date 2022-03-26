
# INSTALLATION
Start a Starlane server:
```bash
starlane serve
```

In another terminal execute the Makefile (this will build the wasm file for the App and create bundle.zip):

```bash
make all
```

Execute the Starlane script command:

```bash
starlane script script/install.script
```

# TEST
* To view the static site go to `http://localhost:8080/index.html`
* To view the App content go to `http://localhost:8080/app/YourName`
