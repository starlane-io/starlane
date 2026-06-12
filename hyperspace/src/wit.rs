
mod bindings {
    wasmtime::component::bindgen!({
        path: "../wit",
        world: "registry",
    });
}

use wasmtime::component::HasData;
pub use bindings::exports::starlane::hyperspace::registry_api::Guest as RegistryGuest;
pub use bindings::starlane::hyperspace::status_api::Host as StatusHost;
pub use bindings::starlane::hyperspace::space;




