
use starlane_space::status::Status;

pub mod filter {
    mod bindings {
        use crate::registry::RegistryApi;
        wasmtime::component::bindgen!({
        path: "../wit",
        world: "filter",
    });
    }

    pub use bindings::exports::starlane::hyperspace::filter_api::Guest as FilterGuest;
    pub use bindings::starlane::hyperspace::status_api::{Host as StatusHost,*};
}


mod bindings {
    use crate::registry::RegistryApi;
    wasmtime::component::bindgen!({
        path: "../wit",
        world: "registry",
    });
}
pub use bindings::starlane::hyperspace::space;
pub use bindings::exports::starlane::hyperspace::registry_api::Guest as RegistryGuest;
use crate::wit::filter::FilterGuest;

pub mod convert {
    use itertools::Itertools;
    use super::space as wit;
    use starlane_space as space;
    use starlane_space::AccessGrantKind;

    // Status
    impl From<&space::Status> for wit::Status {
        fn from(status: &space::Status) -> Self {
            match status {
                space::Status::Unknown => wit::Status::Unknown,
                space::Status::Pending => wit::Status::Pending,
                space::Status::Init => wit::Status::Init,
                space::Status::Panic => wit::Status::Panic,
                space::Status::Fatal => wit::Status::Fatal,
                space::Status::Ready => wit::Status::Ready,
                space::Status::Paused => wit::Status::Paused,
                space::Status::Resuming => wit::Status::Resuming,
                space::Status::Done => wit::Status::Done,
            }
        }
    }

    impl From<space::Status> for wit::Status {
        fn from(status: space::Status) -> Self {
            (&status).into()
        }
    }

    struct Blah;




    // Properties
    impl From<space::particle::Property> for wit::Property {
        fn from(prop: space::particle::Property) -> Self {
            wit::Property {
                key: prop.key.to_string(),
                value: prop.value,
                locked: prop.locked,
            }
        }
    }


    // PropertyMod for SetProperties
    impl From<&space::types::property::PropertyMod> for wit::PropertyMod {
        fn from(prop_mod: &space::types::property::PropertyMod) -> Self {
            match prop_mod {
                space::types::property::PropertyMod::Set(property) => {
                   wit::PropertyMod::Set(property.into())
                },
                space::types::property::PropertyMod::UnSet(key) => wit::PropertyMod::Unset(key.to_string()),
            }
        }
    }

    impl From<space::types::property::PropertyMod> for wit::PropertyMod {
        fn from(prop_mod: space::types::property::PropertyMod) -> Self {
            (&prop_mod).into()
        }
    }


    impl From<&space::Property> for wit::Property {
        fn from(value: &space::Property) -> Self {
            Self {
                key: value.key.to_string(),
                value: value.value.clone(),
                locked: value.locked.clone(),
            }
        }
    }


    // Stub
    impl From<&space::Stub> for wit::Stub {
        fn from(stub: &space::Stub) -> Self {
            wit::Stub {
                point: stub.point.to_string(),
                kind: stub.kind.to_string(),
                status: From::from(&stub.status)
            }
        }
    }

    impl From<space::Stub> for wit::Stub {
        fn from(stub: space::Stub) -> Self {
            (&stub).into()
        }
    }

    pub fn properties_space_to_wit(properties: &space::Properties) -> wit::Properties {
        properties.values().map_into().collect()
    }
    // Details
    impl From<&space::Details> for wit::Details {
        fn from(details: &space::Details) -> Self {
            wit::Details {
                stub: (&details.stub).into(),
                properties: properties_space_to_wit(&details.properties),
            }
        }
    }

    impl From<space::Details> for wit::Details {
        fn from(details: space::Details) -> Self {
            (&details).into()
        }
    }

    // ParticleLocation
    impl From<&space::ParticleLocation> for wit::ParticleLocation {
        fn from(loc: &space::ParticleLocation) -> Self {
            todo!();
            /*
            wit::ParticleLocation {
                star: loc.star.map(Into::into),
                host: loc.host.map(Into::into),
            }

             */
        }
    }

    // ParticleRecord
    impl From<&space::ParticleRecord> for wit::ParticleRecord {
        fn from(record: &space::ParticleRecord) -> Self {
            wit::ParticleRecord {
                details: (&record.details).into(),
                location: (&record.location).into(),
            }
        }
    }

    impl From<space::ParticleRecord> for wit::ParticleRecord {
        fn from(record: space::ParticleRecord) -> Self {
            (&record).into()
        }
    }

    // Access
    impl From<&space::EnumeratedAccess> for wit::EnumeratedAccess {
        fn from(access: &space::EnumeratedAccess) -> Self {
           todo!()
        }
    }


    impl From<space::Access> for wit::Access {
        fn from(access: space::Access) -> Self {
            todo!();

        }
    }

    // AccessGrant
    impl From<&space::AccessGrantKind> for wit::AccessGrantKind {
        fn from(kind: &space::AccessGrantKind) -> Self {
            todo!()
/*            match kind {
                AccessGrantKind::Super => wit::AccessGrantKind::Super,
                AccessGrantKind::Privilege(privilege) => wit::AccessGrantKind::Privilege(privilege.into()),
                AccessGrantKind::PermissionsMask(mask) => wit::AccessGrantKind::Permissions(mask.into())
            }

 */
        }
    }

    impl From<space::AccessGrant> for wit::AccessGrant {
        fn from(grant: space::AccessGrant) -> Self {
            todo!();
/*            wit::AccessGrant {
                on_point: (&grant.on_point).into(),
                to_point: (&grant.to_point).into(),
                by_particle: grant.by_particle.into(),
                kind: (&grant.kind).into(),
            }

 */
        }
    }

    // IndexedAccessGrant
    impl From<&space::IndexedAccessGrant> for wit::IndexedAccessGrant {
        fn from(indexed: &space::IndexedAccessGrant) -> Self {
            wit::IndexedAccessGrant {
                id: indexed.id,
                grant: todo!()
            }
        }
    }

    // Strategy
    impl From<space::Strategy> for wit::CreateStrategy {
        fn from(strategy: space::Strategy) -> Self {
            match strategy {
                space::Strategy::Commit => wit::CreateStrategy::Commit,
                space::Strategy::Ensure => wit::CreateStrategy::Ensure,
                space::Strategy::Override => wit::CreateStrategy::Override,
            }
        }
    }

    // SetRegistry


    // Delete
    impl From<&space::Delete> for wit::Delete {
        fn from(delete: &space::Delete) -> Self {
            todo!();
/*            wit::Delete {
                selector: delete.selector.into(),
            }

 */
        }
    }

    impl From<space::Delete> for wit::Delete {
        fn from(delete: space::Delete) -> Self {
            (&delete).into()
        }
    }

    // Query
    impl From<&space::Query> for wit::Query {
        fn from(query: &space::Query) -> Self {
            match query {
                space::Query::PointHierarchy => wit::Query::PointHierarchy,
            }
        }
    }

    impl From<space::Query> for wit::Query {
        fn from(query: space::Query) -> Self {
            (&query).into()
        }
    }

    // QueryResult
    impl From<&space::QueryResult> for wit::QueryResult {
        fn from(result: &space::QueryResult) -> Self {
            todo!()
            /*
            match result {
                space::QueryResult::PointHierarchy(s) => wit::QueryResult::PointHierarchy(s.into()),
            }

             */
        }
    }

    impl From<space::QueryResult> for wit::QueryResult {
        fn from(result: space::QueryResult) -> Self {
            (&result).into()
        }
    }

    // Select







}
pub mod example {


    use wasmtime::Result;
    use wasmtime::component::{bindgen, ResourceTable, Resource};
    use example::imported_resources::logging::{Level, Host, HostLogger};

    bindgen!({
    inline: r#"
        package example:imported-resources;

        interface logging {
            enum level {
                debug,
                info,
                warn,
                error,
            }

            resource logger {
                constructor(max-level: level);

                get-max-level: func() -> level;
                set-max-level: func(level: level);

                log: func(level: level, msg: string);
            }
        }

        world import-some-resources {
            import logging;
        }
    "#,

    with: {
        // Specify that our host resource is going to point to the `MyLogger`
        // which is defined just below this macro.
        "example:imported-resources/logging.logger": MyLogger,
    },

    // Interactions with `ResourceTable` can possibly trap so enable the ability
    // to return traps from generated functions.
    imports: { default: trappable },
});

    /// A sample host-defined type which contains arbitrary host-defined data.
    ///
    /// In this case this is relatively simple but there's no restrictions on what
    /// this type can hold other than that it must be `'static + Send`.
    pub struct MyLogger {
        pub max_level: example::imported_resources::logging::Level,
    }

    #[derive(Default)]
    struct MyState {
        // Manages the mapping of `MyLogger` structures to `Resource<MyLogger>`.
        table: ResourceTable,
    }

    // There are no free-functions on `interface logging`, so this is an empty
    // impl.
    impl Host for MyState {}

    // This separate `HostLogger` trait serves to act as a namespace for just
    // the `logger`-related resource methods.
    impl HostLogger for MyState {
        // A `constructor` in WIT maps to a `new` function in Rust.
        fn new(&mut self, max_level: Level) -> Result<Resource<MyLogger>> {
            let id = self.table.push(MyLogger { max_level })?;
            Ok(id)
        }

        fn get_max_level(&mut self, logger: Resource<MyLogger>) -> Result<Level> {
            debug_assert!(!logger.owned());
            let logger = self.table.get(&logger)?;
            Ok(logger.max_level)
        }

        fn set_max_level(&mut self, logger: Resource<MyLogger>, level: Level) -> Result<()> {
            debug_assert!(!logger.owned());
            let logger = self.table.get_mut(&logger)?;
            logger.max_level = level;
            Ok(())
        }

        fn log(&mut self, logger: Resource<MyLogger>, level: Level, msg: String) -> Result<()> {
            debug_assert!(!logger.owned());
            let logger = self.table.get_mut(&logger)?;
            if (level as u32) <= (logger.max_level as u32) {
                println!("{msg}");
            }
            Ok(())
        }

        fn drop(&mut self, logger: Resource<MyLogger>) -> Result<()> {
            debug_assert!(logger.owned());
            let _logger: MyLogger = self.table.delete(logger)?;
            // ... custom destruction logic here if necessary, otherwise
            // a `Drop for MyLogger` would also work.
            Ok(())
        }
    }

}
#[derive(Default)]
pub struct Bobo;

impl Bobo {
    pub fn hello() {}
}

pub mod yuk {
    pub fn what() {}
}


#[cfg(test)]
pub mod tests {
    #[test]
    pub fn test(){
    }

}


