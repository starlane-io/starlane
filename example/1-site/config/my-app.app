App {

  Set {
    +wasm.src=${self.config.bundle}:/wasm/my-app.wasm,
    +mechtron.name=my-app,
    +bind=${self.config.bundle}:/bind/app.bind
  }

  Install {
    create ${self}:users<Base<User>>;
    create ${self}:files<FileSystem>;
  }

}
