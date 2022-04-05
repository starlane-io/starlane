use mesh_portal::version::latest::command::common::{PropertyMod, SetProperties};
use mesh_portal::version::latest::id::Address;
use mesh_portal::version::latest::resource::{ResourceStub, Status};
use sqlx::{Connection, Executor, Pool, Postgres, Transaction};
use sqlx::postgres::{PgArguments, PgPoolOptions};
use crate::error::Error;
use crate::star::core::resource::registry::{RegError, Registration, RegistryParams};
use crate::star::StarKey;

lazy_static! {
    pub static ref STARLANE_POSTGRES_URL: String= std::env::var("STARLANE_POSTGRES_URL").unwrap_or("localhost".to_string());
    pub static ref STARLANE_POSTGRES_USER: String= std::env::var("STARLANE_POSTGRES_USER").unwrap_or("postgres".to_string());
    pub static ref STARLANE_POSTGRES_PASSWORD: String= std::env::var("STARLANE_POSTGRES_PASSWORD").unwrap_or("password".to_string());
    pub static ref STARLANE_POSTGRES_DATABASE: String= std::env::var("STARLANE_POSTGRES_DATABASE").unwrap_or("postgres".to_string());
}

pub struct Registry {
    pool: Pool<Postgres>
}

impl Registry {
    pub async fn new() -> Result<Self,Error> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(format!("postgres://{}:{}@{}/{}",STARLANE_POSTGRES_USER.as_str(),STARLANE_POSTGRES_PASSWORD.as_str(),STARLANE_POSTGRES_URL.as_str(),STARLANE_POSTGRES_DATABASE.as_str()).as_str()).await?;
        let registry = Self{
            pool
        };

        match registry.setup().await {
            Ok(_) => {
                info!("registry setup complete.");
            }
            Err(err) => {
                let message = err.into_database_error().unwrap().message().to_string();
                error!("database setup failed {} ", message);
                return Err(message.into());
            }
        }

        Ok(registry)
    }

    async fn setup(&self) -> Result<(), sqlx::Error> {
//        let database= format!("CREATE DATABASE IF NOT EXISTS {}", STARLANE_POSTGRES_DATABASE );

        let resources = r#"CREATE TABLE IF NOT EXISTS resources (
         id SERIAL PRIMARY KEY,
         address_segment TEXT NOT NULL,
         parent TEXT NOT NULL,
         resource_type TEXT NOT NULL,
         kind TEXT,
         vendor TEXT,
         product TEXT,
         variant TEXT,
         version TEXT,
         version_variant TEXT,
         star TEXT,
         status TEXT NOT NULL,
         sequence INTEGER DEFAULT 0,
         UNIQUE(parent,address_segment)
        )"#;

        let labels = r#"
       CREATE TABLE IF NOT EXISTS labels (
          id SERIAL PRIMARY KEY,
	      resource_id INTEGER NOT NULL,
	      key TEXT NOT NULL,
	      value TEXT,
          UNIQUE(key,value),
          FOREIGN KEY (resource_id) REFERENCES resources (id)
        )"#;

        /// note that a tag may reference an address NOT in this database
        /// therefore it does not have a FOREIGN KEY constraint
        let tags = r#"
       CREATE TABLE IF NOT EXISTS tags(
          id SERIAL PRIMARY KEY,
          parent TEXT NOT NULL,
          tag TEXT NOT NULL,
          address TEXT NOT NULL,
          UNIQUE(tag)
        )"#;


        let properties = r#"CREATE TABLE IF NOT EXISTS properties (
         id SERIAL PRIMARY KEY,
	     resource_id INTEGER NOT NULL,
         key TEXT NOT NULL,
         value TEXT NOT NULL,
         lock INTEGER NOT NULL,
         FOREIGN KEY (resource_id) REFERENCES resources (id),
         UNIQUE(resource_id,key)
        )"#;

        let address_index = "CREATE UNIQUE INDEX IF NOT EXISTS resource_address_index ON resources(parent,address_segment)";

        println!("starting setup....");
        let mut conn = self.pool.acquire().await?;
        let mut transaction = conn.begin().await?;
        println!("setup resources...");
        transaction.execute(resources).await?;
        /*
        println!("setup labels...");
        transaction.execute(labels).await?;
        println!("setup tags...");
        transaction.execute(tags).await?;
         */
        println!("setup properties...");
        transaction.execute(properties).await?;
        println!("setup address_index ...");
        transaction.execute(address_index).await?;
        transaction.commit().await?;
        println!("Setup complete.");

        Ok(())
    }

    async fn nuke( &self) -> Result<(),Error>{
        let mut conn = self.pool.acquire().await?;
        let mut trans = conn.begin().await?;
        trans.execute("DELETE FROM resources").await?;
        trans.commit().await?;
        Ok(())
    }

    async fn register( &self, registration: &Registration ) -> Result<(),Error> {
        /*
        async fn check<'a>( registration: &Registration,  trans:&mut Transaction<Postgres>, ) -> Result<(),RegError> {
            let params = RegistryParams::from_registration(registration)?;
            let count:u64 = sqlx::query_as("SELECT count(*) as count from resources WHERE parent=? AND address_segment=?").bind(params.parent).bind(params.address_segment).fetch_one(trans).await?;
            if count > 0 {
                Err(RegError::Dupe)
            } else {
                Ok(())
            }
        }
         */

        let address = registration.address.clone();
        let mut conn = self.pool.acquire().await?;
        let mut trans = conn.begin().await?;
            let params = RegistryParams::from_registration(&registration)?;
            let statement = format!("INSERT INTO resources (address_segment,resource_type,kind,vendor,product,variant,version,version_variant,parent,status) VALUES ('{}','{}',{},{},{},{},{},{},'{}','Pending')", params.address_segment,params.resource_type,opt(&params.kind),opt(&params.vendor),opt(&params.product),opt(&params.variant),opt(&params.version),opt(&params.version_variant),params.parent);
println!("statement: {}",statement);
            trans.execute(statement.as_str()).await?;

                for (_,property_mod) in registration.properties.iter() {
                    match property_mod {
                        PropertyMod::Set{ key, value,lock } => {
                            let lock:usize = match lock {
                                true => 1,
                                false => 0
                            };
                            let statement = format!("INSERT INTO properties (resource_id,key,value,lock) VALUES ((SELECT id FROM resources WHERE parent='{}' AND address_segment='{}'),'{}','{}',{})", params.parent,params.address_segment,key.to_string(),value.to_string(),lock);
                            trans.execute(statement.as_str()).await?;

                        }
                        PropertyMod::UnSet(key) => {
                            let statement = format!("DELETE FROM properties WHERE resource_id=(SELECT id FROM resources WHERE parent='{}' AND address_segment='{}') AND key='{}' AND lock=false", params.parent,params.address_segment,key.to_string());
                            trans.execute(statement.as_str()).await?;
                        }
                    }
                }
        trans.commit().await?;
        Ok(())
    }

    pub async fn assign(&self, address: &Address, host: &StarKey) -> Result<(),Error> {

            let parent = address.parent().ok_or("expecting parent since we have already established the segments are >= 2")?;
            let address_segment = address.last_segment().ok_or("expecting a last_segment since we know segments are >= 2")?;
        let statement = format!("UPDATE resources SET star='{}' WHERE parent='{}' AND address_segment='{}'", host.to_string(),parent.to_string(),address_segment.to_string());
        let mut conn = self.pool.acquire().await?;
        let mut trans = conn.begin().await?;
        trans.execute(statement.as_str()).await?;
        trans.commit().await?;
        Ok(())
    }

    async fn set_status(&self, address: &Address, status: &Status ) -> Result<(),Error> {
            let parent = address.parent().ok_or("resource must have a parent")?.to_string();
            let address_segment = address.last_segment().ok_or("resource must have a last segment")?.to_string();
            let status = status.to_string();
            let statement = format!("UPDATE resources SET status='{}' WHERE parent='{}' AND address_segment='{}'", status.to_string(), parent, address_segment );
            let mut conn = self.pool.acquire().await?;
            let mut trans = conn.begin().await?;
            trans.execute(statement.as_str()).await?;
            trans.commit().await?;
            Ok(())
    }

}

fn opt( opt: &Option<String> ) -> String {
    match opt {
        None => {
            "null".to_string()
        }
        Some(value) => {
            format!("'{}'",value)
        }
    }
}

#[cfg(test)]
pub mod test {
    use std::str::FromStr;
    use mesh_portal::version::latest::id::Address;
    use mesh_portal::version::latest::resource::Status;
    use crate::error::Error;
    use crate::registry::Registry;
    use crate::resource::Kind;
    use crate::star::core::resource::registry::Registration;
    use crate::star::StarKey;


    #[tokio::test]
    pub async fn test_nuke() -> Result<(),Error> {
        let registry = Registry::new().await?;
        registry.nuke().await?;
        Ok(())
    }

    #[tokio::test]
    pub async fn test_create() -> Result<(),Error> {
        let registry = Registry::new().await?;
        registry.nuke().await?;
        let address = Address::from_str("localhost:mechtron")?;
        let registration = Registration {
            address: address.clone(),
            kind: Kind::Mechtron,
            registry: Default::default(),
            properties: Default::default()
        };
        registry.register(&registration).await?;

        let star = StarKey::central();
        registry.assign(&address,&star).await?;
        registry.set_status( &address, &Status::Ready ).await?;

        Ok(())
    }
}

