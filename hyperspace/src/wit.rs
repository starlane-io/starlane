pub mod status_api {

    /// this is a hack because [wit_bindgen] does not generate a convenient `Host` trait for imports in the same way
    /// that it generates a `Guest` trait for exports... so in this case we use a world called `status-host` and generate
    /// a `Guest` which we rename into a `StatusHost`
     pub mod host {
        mod bindings {
            wit_bindgen::generate!({
            path: ["../wit/hyperspace.wit"],
            world: "status-host",
        });
        }

        pub use bindings::exports::starlane::hyperspace::status_api::Guest as StatusHost;
        pub use bindings::exports::starlane::hyperspace::status_api::*;
    }
}

pub mod registry {
    pub mod guest {
        mod bindings {
            wit_bindgen::generate!({
                path: ["../wit/hyperspace.wit"],
                world: "registry",
                async: true
            });
        }

        pub use bindings::exports::starlane::hyperspace::registry_api::Guest as RegistryGuest;
        pub use bindings::exports::starlane::hyperspace::registry_api::*;
    }
}
