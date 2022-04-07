use crate::error::Error;
use crate::resource::{Kind, ResourceLocation, ResourceRecord, ResourceType};
use crate::star::core::resource::registry::{RegError, Registration, RegistryParams};
use crate::star::StarKey;
use futures::{FutureExt, StreamExt};
use mesh_portal::error::MsgErr;
use mesh_portal::version::latest::command::common::{PropertyMod, SetProperties};
use mesh_portal::version::latest::entity::request::query::{Query, QueryResult};
use mesh_portal::version::latest::entity::request::select::Select;
use mesh_portal::version::latest::entity::request::{Action, Rc};
use mesh_portal::version::latest::id::{Address, Specific, Version};
use mesh_portal::version::latest::messaging::Request;
use mesh_portal::version::latest::pattern::specific::{
    ProductPattern, VariantPattern, VendorPattern,
};
use mesh_portal::version::latest::pattern::{
    AddressKindPath, AddressKindPattern, AddressKindSegment, ExactSegment, KindPattern,
    ResourceTypePattern, SegmentPattern,
};
use mesh_portal::version::latest::payload::{Primitive, PrimitiveList};
use mesh_portal::version::latest::resource::{Property, ResourceStub, Status};
use mesh_portal::version::latest::util::ValuePattern;
use mesh_portal_versions::version::v0_0_1::entity::request::select::SubSelect;
use mesh_portal_versions::version::v0_0_1::security::{AccessGrant, AccessGrantKind, Access, PermissionsMask, Permissions, PermissionsMaskKind, EnumeratedAccess};
use mysql::prelude::TextQuery;
use sqlx::postgres::{PgArguments, PgPoolOptions, PgRow};
use sqlx::{Connection, Executor, Pool, Postgres, Row, Transaction};
use std::collections::{HashMap, HashSet};
use std::convert::TryInto;
use std::num::ParseIntError;
use std::str::FromStr;
use mesh_portal_versions::version::v0_0_1::config::bind::parse::bind;

lazy_static! {
    pub static ref HYPERUSER: Address = Address::from_str("hyperspace:users:hyperuser").expect("address");
    pub static ref STARLANE_POSTGRES_URL: String =
        std::env::var("STARLANE_POSTGRES_URL").unwrap_or("localhost".to_string());
    pub static ref STARLANE_POSTGRES_USER: String =
        std::env::var("STARLANE_POSTGRES_USER").unwrap_or("postgres".to_string());
    pub static ref STARLANE_POSTGRES_PASSWORD: String =
        std::env::var("STARLANE_POSTGRES_PASSWORD").unwrap_or("password".to_string());
    pub static ref STARLANE_POSTGRES_DATABASE: String =
        std::env::var("STARLANE_POSTGRES_DATABASE").unwrap_or("postgres".to_string());
}

pub struct Registry {
    pool: Pool<Postgres>,
}

impl Registry {
    pub async fn new() -> Result<Self, Error> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(
                format!(
                    "postgres://{}:{}@{}/{}",
                    STARLANE_POSTGRES_USER.as_str(),
                    STARLANE_POSTGRES_PASSWORD.as_str(),
                    STARLANE_POSTGRES_URL.as_str(),
                    STARLANE_POSTGRES_DATABASE.as_str()
                )
                .as_str(),
            )
            .await?;
        let registry = Self { pool };

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
         address TEXT NOT NULL,
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
         owner TEXT,
         UNIQUE(address),
         UNIQUE(parent,address_segment)
        )"#;

        let access_grants = r#"
       CREATE TABLE IF NOT EXISTS access_grants (
          id SERIAL PRIMARY KEY,
	      kind TEXT NOT NULL,
	      data TEXT,
	      query_root TEXT NOT NULL,
	      on_point TEXT NOT NULL,
	      to_point TEXT NOT NULL,
	      by_particle INTEGER NOT NULL,
          FOREIGN KEY (by_particle) REFERENCES resources (id)
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
         lock BOOLEAN NOT NULL,
         FOREIGN KEY (resource_id) REFERENCES resources (id),
         UNIQUE(resource_id,key)
        )"#;

        let address_index= "CREATE UNIQUE INDEX IF NOT EXISTS resource_address_index ON resources(address)";
        let address_segment_parent_index = "CREATE UNIQUE INDEX IF NOT EXISTS resource_address_segment_parent_index ON resources(parent,address_segment)";
        let access_grants_index =
            "CREATE UNIQUE INDEX IF NOT EXISTS query_root_index ON access_grants(query_root)";


        let mut conn = self.pool.acquire().await?;
        let mut transaction = conn.begin().await?;
        transaction.execute(resources).await?;
        transaction.execute(access_grants).await?;
        /*
        transaction.execute(labels).await?;
        transaction.execute(tags).await?;
         */
        transaction.execute(properties).await?;
        transaction.execute(address_index).await?;
        transaction.execute(address_segment_parent_index).await?;
        transaction.execute(access_grants_index).await?;
        transaction.commit().await?;

        Ok(())
    }

    async fn nuke(&self) -> Result<(), Error> {
        let mut conn = self.pool.acquire().await?;
        let mut trans = conn.begin().await?;
        trans.execute("DROP TABLE resources CASCADE").await;
        trans.execute("DROP TABLE access_grants CASCADE").await;
        trans.execute("DROP TABLE properties CASCADE").await;
        trans.commit().await?;
        self.setup().await?;
        Ok(())
    }

    async fn register(&self, registration: &Registration) -> Result<(), RegError> {
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
        struct Count(u64);

        impl sqlx::FromRow<'_, PgRow> for Count {
            fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
                let v: i64 = row.get(0);
                Ok(Self(v as u64))
            }
        }

        let mut conn = self.pool.acquire().await?;
        let mut trans = conn.begin().await?;
        let params = RegistryParams::from_registration(&registration)?;

        let count = sqlx::query_as::<Postgres, Count>(
            "SELECT count(*) as count from resources WHERE parent=$1 AND address_segment=$2",
        )
        .bind(params.parent.to_string())
        .bind(params.address_segment.to_string())
        .fetch_one(&mut trans)
        .await?;

        if count.0 > 0 {
            return Err(RegError::Dupe);
        }

        let statement = format!("INSERT INTO resources (address,address_segment,resource_type,kind,vendor,product,variant,version,version_variant,parent,owner,status) VALUES ('{}','{}','{}',{},{},{},{},{},{},'{}','{}','Pending')", params.address, params.address_segment, params.resource_type, opt(&params.kind), opt(&params.vendor), opt(&params.product), opt(&params.variant), opt(&params.version), opt(&params.version_variant), params.parent, params.owner.to_string());
        trans.execute(statement.as_str()).await?;

        for (_, property_mod) in registration.properties.iter() {
            match property_mod {
                PropertyMod::Set { key, value, lock } => {
                    let lock: usize = match lock {
                        true => 1,
                        false => 0,
                    };
                    let statement = format!("INSERT INTO properties (resource_id,key,value,lock) VALUES ((SELECT id FROM resources WHERE parent='{}' AND address_segment='{}'),'{}','{}',{})", params.parent, params.address_segment, key.to_string(), value.to_string(), lock);
                    trans.execute(statement.as_str()).await?;
                }
                PropertyMod::UnSet(key) => {
                    let statement = format!("DELETE FROM properties WHERE resource_id=(SELECT id FROM resources WHERE parent='{}' AND address_segment='{}') AND key='{}' AND lock=false", params.parent, params.address_segment, key.to_string());
                    trans.execute(statement.as_str()).await?;
                }
            }
        }
        trans.commit().await?;
        Ok(())
    }

    pub async fn assign(&self, address: &Address, host: &StarKey) -> Result<(), Error> {
        let parent = address
            .parent()
            .ok_or("expecting parent since we have already established the segments are >= 2")?;
        let address_segment = address
            .last_segment()
            .ok_or("expecting a last_segment since we know segments are >= 2")?;
        let statement = format!(
            "UPDATE resources SET star='{}' WHERE parent='{}' AND address_segment='{}'",
            host.to_string(),
            parent.to_string(),
            address_segment.to_string()
        );
        let mut conn = self.pool.acquire().await?;
        let mut trans = conn.begin().await?;
        trans.execute(statement.as_str()).await?;
        trans.commit().await?;
        Ok(())
    }

    async fn set_status(&self, address: &Address, status: &Status) -> Result<(), Error> {
        let parent = address
            .parent()
            .ok_or("resource must have a parent")?
            .to_string();
        let address_segment = address
            .last_segment()
            .ok_or("resource must have a last segment")?
            .to_string();
        let status = status.to_string();
        let statement = format!(
            "UPDATE resources SET status='{}' WHERE parent='{}' AND address_segment='{}'",
            status.to_string(),
            parent,
            address_segment
        );
        let mut conn = self.pool.acquire().await?;
        let mut trans = conn.begin().await?;
        trans.execute(statement.as_str()).await?;
        trans.commit().await?;
        Ok(())
    }

    async fn set_properties(
        &self,
        address: &Address,
        properties: &SetProperties,
    ) -> Result<(), Error> {
        let mut conn = self.pool.acquire().await?;
        let mut trans = conn.begin().await?;
        let parent = address
            .parent()
            .ok_or("resource must have a parent")?
            .to_string();
        let address_segment = address
            .last_segment()
            .ok_or("resource must have a last segment")?
            .to_string();

        for (_, property_mod) in properties.iter() {
            match property_mod {
                PropertyMod::Set { key, value, lock } => {
                    let lock = match *lock {
                        true => 1,
                        false => 0,
                    };

                    let statement = format!("INSERT INTO properties (resource_id,key,value,lock) VALUES ((SELECT id FROM resources WHERE parent='{}' AND address_segment='{}'),'{}' ,'{}','{}') ON CONFLICT(resource_id,key) DO UPDATE SET value='{}' WHERE lock=false", parent, address_segment, key.to_string(), value.to_string(), value.to_string(), lock);
                    trans.execute(statement.as_str()).await?;
                }
                PropertyMod::UnSet(key) => {
                    let statement = format!("DELETE FROM properties WHERE resource_id=(SELECT id FROM resources WHERE parent='{}' AND address_segment='{}') AND key='{}' AND lock=false", parent, address_segment, key.to_string());
                    trans.execute(statement.as_str()).await?;
                }
            }
        }
        trans.commit().await?;
        Ok(())
    }

    async fn sequence(&self, address: &Address) -> Result<u64, Error> {
        struct Sequence(u64);

        impl sqlx::FromRow<'_, PgRow> for Sequence {
            fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
                let v: i32 = row.get(0);
                Ok(Self(v as u64))
            }
        }

        let mut conn = self.pool.acquire().await?;
        let mut trans = conn.begin().await?;
        let parent = address
            .parent()
            .ok_or("expecting parent since we have already established the segments are >= 2")?;
        let address_segment = address
            .last_segment()
            .ok_or("expecting a last_segment since we know segments are >= 2")?;
        let statement = format!(
            "UPDATE resources SET sequence=sequence+1 WHERE parent='{}' AND address_segment='{}'",
            parent.to_string(),
            address_segment.to_string()
        );

        trans.execute(statement.as_str()).await?;
        let sequence = sqlx::query_as::<Postgres, Sequence>(
            "SELECT DISTINCT sequence FROM resources WHERE parent=$1 AND address_segment=$2",
        )
        .bind(parent.to_string())
        .bind(address_segment.to_string())
        .fetch_one(&mut trans)
        .await?;
        trans.commit().await?;

        Ok(sequence.0)
    }

    pub async fn locate(&self, address: &Address) -> Result<ResourceRecord, Error> {
        let mut conn = self.pool.acquire().await?;
        let parent = address.parent().ok_or("expected a parent")?;
        let address_segment = address
            .last_segment()
            .ok_or("expected last address_segment")?
            .to_string();

        let mut record = sqlx::query_as::<Postgres, ResourceRecord>(
            "SELECT DISTINCT * FROM resources as r WHERE parent=$1 AND address_segment=$2",
        )
        .bind(parent.to_string())
        .bind(address_segment.clone())
        .fetch_one(&mut conn)
        .await?;
        let properties = sqlx::query_as::<Postgres,LocalProperty>("SELECT key,value,lock FROM properties WHERE resource_id=(SELECT id FROM resources WHERE parent=$1 AND address_segment=$2)").bind(parent.to_string()).bind(address_segment).fetch_all(& mut conn).await?;
        let mut map = HashMap::new();
        for p in properties {
            map.insert(p.key.clone(), p.into());
        }
        record.stub.properties = map;

        Ok(record)
    }

    pub async fn query(&self, address: &Address, query: &Query) -> Result<QueryResult, Error> {
        let mut kind_path = AddressKindPath::new(address.route.clone(), vec![]);
        let route = address.route.clone();

        let mut segments = vec![];
        for segment in &address.segments {
            segments.push(segment.clone());
            let address = Address {
                route: route.clone(),
                segments: segments.clone(),
            };
            let record = self.locate(&address).await?;
            let kind_segment = AddressKindSegment {
                address_segment: record
                    .stub
                    .address
                    .last_segment()
                    .ok_or("expected at least one segment")?,
                kind: record.stub.kind,
            };
            kind_path = kind_path.push(kind_segment);
        }
        return Ok(QueryResult::AddressKindPath(kind_path));
    }

    #[async_recursion]
    pub async fn select(&self, select: &Select) -> Result<PrimitiveList, Error> {
        let address = select.pattern.query_root();

        let address_kind_path = self
            .query(&address, &Query::AddressKindPath)
            .await?
            .try_into()?;

        let sub_select_hops = select.pattern.sub_select_hops();
        let sub_select =
            select
                .clone()
                .sub_select(address.clone(), sub_select_hops, address_kind_path);
        let list = self.sub_select(&sub_select).await?;
        let list = sub_select.into_payload.to_primitive(list)?;

        Ok(list)
    }

    #[async_recursion]
    async fn sub_select(&self, sub_select: &SubSelect) -> Result<Vec<ResourceStub>, Error> {
        // build a 'matching so far' query.  Here we will find every child that matches the subselect
        // these matches are used to then query children for additional matches if there are more hops.
        // all of these matches will be filtered to see if they match the ENTIRE select before returning results.
        let mut params: Vec<String> = vec![];
        let mut where_clause = String::new();
        let mut index = 1;
        where_clause.push_str("parent=$1");
        params.push(sub_select.address.to_string());

        if let Option::Some(hop) = sub_select.hops.first() {
            match &hop.segment {
                SegmentPattern::Exact(exact) => {
                    index = index + 1;
                    where_clause.push_str(format!(" AND address_segment=${}", index).as_str());
                    match exact {
                        ExactSegment::Address(address) => {
                            params.push(address.to_string());
                        }
                        ExactSegment::Version(version) => {
                            params.push(version.to_string());
                        }
                    }
                }
                _ => {}
            }

            match &hop.tks.resource_type {
                ResourceTypePattern::Any => {}
                ResourceTypePattern::Exact(resource_type) => {
                    index = index + 1;
                    where_clause.push_str(format!(" AND resource_type=${}", index).as_str());
                    params.push(resource_type.to_string());
                }
            }

            match &hop.tks.kind {
                KindPattern::Any => {}
                KindPattern::Exact(kind) => match &kind.kind {
                    None => {}
                    Some(sub) => {
                        index = index + 1;
                        where_clause.push_str(format!(" AND kind=${}", index).as_str());
                        params.push(sub.clone());
                    }
                },
            }

            match &hop.tks.specific {
                ValuePattern::Any => {}
                ValuePattern::None => {}
                ValuePattern::Pattern(specific) => {
                    match &specific.vendor {
                        VendorPattern::Any => {}
                        VendorPattern::Exact(vendor) => {
                            index = index + 1;
                            where_clause.push_str(format!(" AND vendor=${}", index).as_str());
                            params.push(vendor.clone());
                        }
                    }
                    match &specific.product {
                        ProductPattern::Any => {}
                        ProductPattern::Exact(product) => {
                            index = index + 1;
                            where_clause.push_str(format!(" AND product=${}", index).as_str());
                            params.push(product.clone());
                        }
                    }
                    match &specific.variant {
                        VariantPattern::Any => {}
                        VariantPattern::Exact(variant) => {
                            index = index + 1;
                            where_clause.push_str(format!(" AND variant=${}", index).as_str());
                            params.push(variant.clone());
                        }
                    }
                }
            }
        }

        let matching_so_far_statement = format!(
            "SELECT DISTINCT * FROM resources as r WHERE {}",
            where_clause
        );

        let mut query =
            sqlx::query_as::<Postgres, ResourceRecord>(matching_so_far_statement.as_str());
        for param in params {
            query = query.bind(param);
        }

        let mut conn = self.pool.acquire().await?;
        let mut matching_so_far = query.fetch_all(&mut conn).await?;
        let mut matching_so_far: Vec<ResourceStub> =
            matching_so_far.into_iter().map(|r| r.into()).collect();

        let mut child_stub_matches = vec![];

        // if we have more hops we need to see if there are matching children
        if !sub_select.hops.is_empty() {
            let mut hops = sub_select.hops.clone();
            let hop = hops.first().unwrap();
            match hop.segment {
                SegmentPattern::Recursive => {}
                _ => {
                    hops.remove(0);
                }
            }

            for stub in &matching_so_far {
                if let Option::Some(last_segment) = stub.address.last_segment() {
                    let address = sub_select.address.push_segment(last_segment.clone());
                    let address_tks_path = sub_select.address_kind_path.push(AddressKindSegment {
                        address_segment: last_segment,
                        kind: stub.kind.clone(),
                    });
                    let sub_select = sub_select.clone().sub_select(
                        address.clone(),
                        hops.clone(),
                        address_tks_path,
                    );
                    let more_stubs = self.sub_select(&sub_select).await?;
                    for stub in more_stubs.into_iter() {
                        child_stub_matches.push(stub);
                    }
                }
            }

            // the records matched the present hop (which we needed for deeper searches) however
            // they may not or may not match the ENTIRE select pattern therefore they must be filtered
            matching_so_far.retain(|stub| {
                let address_tks_path = sub_select.address_kind_path.push(AddressKindSegment {
                    address_segment: stub
                        .address
                        .last_segment()
                        .expect("expecting at least one segment"),
                    kind: stub.kind.clone(),
                });
                sub_select.pattern.matches(&address_tks_path)
            });

            matching_so_far.append(&mut child_stub_matches);
        }

        let stubs: Vec<ResourceStub> = matching_so_far
            .into_iter()
            .map(|record| record.into())
            .collect();

        Ok(stubs)
    }

    async fn grant(&self, access_grant: &AccessGrant) -> Result<(), Error> {

        let mut conn = self.pool.acquire().await?;
        match &access_grant.kind {
            AccessGrantKind::Super => {
                sqlx::query("INSERT INTO access_grants (kind,query_root,on_point,to_point,by_particle) VALUES ('superuser',$1,$2,$3,(SELECT id FROM resources WHERE address=$4))")
                    .bind(access_grant.on_point.query_root().to_string())
                    .bind(access_grant.on_point.clone().to_string())
                    .bind(access_grant.to_point.clone().to_string())
                    .bind(access_grant.by_particle.to_string()).execute(& mut conn).await?;
            }
            AccessGrantKind::Privilege(privilege) => {
                sqlx::query("INSERT INTO access_grants (kind,data,query_root,on_point,to_point,by_particle) VALUES ('privilege',$1,$2,$3,$4(SELECT id FROM resources WHERE address=$5))")
                    .bind(privilege.to_string() )
                    .bind(access_grant.on_point.query_root().to_string())
                    .bind(access_grant.on_point.clone().to_string())
                    .bind(access_grant.to_point.clone().to_string())
                    .bind(access_grant.by_particle.to_string()).execute(& mut conn).await?;
            }
            AccessGrantKind::PermissionsMask(mask) => {
                sqlx::query("INSERT INTO access_grants (kind,data,query_root,on_point,to_point,by_particle) VALUES ('permission-mask',$1,$2,$3,$4,(SELECT id FROM resources WHERE address=$5))")
                    .bind(mask.to_string() )
                    .bind(access_grant.on_point.query_root().to_string())
                    .bind(access_grant.on_point.clone().to_string())
                    .bind(access_grant.to_point.clone().to_string())
                    .bind(access_grant.by_particle.to_string() ).execute(& mut conn).await?;
            }
        }

        Ok(())
    }

    #[async_recursion]
    pub async fn access( &self, to: &Address, on: &Address ) -> Result<Access,Error> {

        if *HYPERUSER == *to {
            return Ok(Access::SuperUser);
        }

        let to_kind_path:AddressKindPath = self.query(&to , &Query::AddressKindPath).await?.try_into()?;
        let on_kind_path:AddressKindPath = self.query(&on , &Query::AddressKindPath).await?.try_into()?;

        let mut traversal = on.clone();
        let mut conn = self.pool.acquire().await?;
        let mut privileges = HashSet::new();
        let mut permissions = Permissions::none();
        let mut level_ands: Vec<Vec<PermissionsMask>> = vec![];
        while !traversal.segments.is_empty() {
            let mut access_grants= sqlx::query_as::<Postgres,AccessGrantWrapper>("SELECT access_grants.*,resources.address as by_particle FROM access_grants,resources WHERE access_grants.query_root=$1 AND resources.id=access_grants.by_particle").bind(traversal.to_string() ).fetch_all(& mut conn).await?;
            let mut access_grants:Vec<AccessGrant> = access_grants.into_iter().map(|a|{a.into()}).collect();
            access_grants.retain( |access_grant| access_grant.to_point.matches(&to_kind_path) && access_grant.on_point.matches(&on_kind_path));
            // check for any superusers
            for access_grant in &access_grants {
                match &access_grant.kind {
                    AccessGrantKind::Super => {
                        if let Access::SuperUser = self.access(&access_grant.by_particle, &on).await? {
                            return Ok(Access::SuperUser)
                        }
                    }
                    AccessGrantKind::Privilege(privilege) => {
                        privileges.insert(privilege.clone());
                    }
                    AccessGrantKind::PermissionsMask(mask) => {
                        if let PermissionsMaskKind::Or = mask.kind {
                            permissions.or(&mask.permissions);
                        }
                    }
                }
            }
            access_grants.retain( |a| {
               if let AccessGrantKind::PermissionsMask(mask) = &a.kind {
                   if let PermissionsMaskKind::And = mask.kind {
                       return true;
                   }
               }
               false
            });
            let ands : Vec<PermissionsMask>= access_grants.into_iter().map( |a| {
                if let AccessGrantKind::PermissionsMask(mask) = a.kind {
                    return mask;
                }
                panic!("expected a mask")
            }).collect();
            // save for later when we traverse back down
            level_ands.push(ands );

            // now reduce the segments of the traversal
            traversal.segments.pop();
        }

        level_ands.reverse();
        for level in level_ands {
            for mask in level {
                permissions.and(&mask.permissions);
            }
        }

        let access = EnumeratedAccess {
           privileges,
           permissions
        };

        let access = Access::Enumerated(access);

        Ok(access)
    }
}

fn opt(opt: &Option<String>) -> String {
    match opt {
        None => "null".to_string(),
        Some(value) => {
            format!("'{}'", value)
        }
    }
}

struct LocalProperty {
    pub key: String,
    pub value: String,
    pub locked: bool,
}

impl Into<Property> for LocalProperty {
    fn into(self) -> Property {
        Property {
            key: self.key,
            value: self.value,
            locked: self.locked,
        }
    }
}

impl sqlx::FromRow<'_, PgRow> for LocalProperty {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        let key = row.get("key");
        let value = row.get("value");
        let locked = row.get("lock");
        Ok(LocalProperty { key, value, locked })
    }
}

pub struct AccessGrantWrapper(AccessGrant);

impl Into<AccessGrant> for AccessGrantWrapper {
    fn into(self) -> AccessGrant {
        self.0
    }
}

impl sqlx::FromRow<'_, PgRow> for AccessGrantWrapper{
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {


        fn wrap(row: &PgRow) -> Result<AccessGrantWrapper, Error> {
            let kind: &str = row.get("kind");
            let kind = match kind {
                "superuser" => {
                    AccessGrantKind::Super
                },
                "privilege" => {
                    let priviledge: String = row.get("kind");
                    AccessGrantKind::Privilege(priviledge)
                }
                "permission-mask" => {
                    let mask: &str = row.get("data");
                    let mask = PermissionsMask::from_str(mask)?;
                    AccessGrantKind::PermissionsMask(mask)
                }
                what => {
                    panic!(format!("don't know how to handle access grant kind {}", what))
                }
            };

            let on_point: &str = row.get("on_point");
            let to_point: &str = row.get("to_point");
            let by_particle: &str = row.get("by_particle");

           let access = AccessGrant {
                kind,
                on_point: AddressKindPattern::from_str(on_point)?,
                to_point: AddressKindPattern::from_str(to_point)?,
                by_particle: Address::from_str(by_particle)?,
            };
            Ok(AccessGrantWrapper(access))
        }

        match wrap(row) {
            Ok(record) => Ok(record),
            Err(err) => {
                error!("{}", err.to_string());
                Err(sqlx::error::Error::Decode(err.into()))
            }
        }
    }
}

impl sqlx::FromRow<'_, PgRow> for ResourceRecord {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        fn wrap(row: &PgRow) -> Result<ResourceRecord, Error> {
            let parent: String = row.get("parent");
            let address_segment: String = row.get("address_segment");
            let resource_type: String = row.get("resource_type");
            let kind: Option<String> = row.get("kind");
            let vendor: Option<String> = row.get("vendor");
            let product: Option<String> = row.get("product");
            let variant: Option<String> = row.get("variant");
            let version: Option<String> = row.get("version");
            let version_variant: Option<String> = row.get("version_variant");
            let star: Option<String> = row.get("star");
            let status: String = row.get("status");

            let address = Address::from_str(parent.as_str())?;
            let address = address.push(address_segment)?;
            let resource_type = ResourceType::from_str(resource_type.as_str())?;

            let specific = if let Option::Some(vendor) = vendor {
                if let Option::Some(product) = product {
                    if let Option::Some(variant) = variant {
                        if let Option::Some(version) = version {
                            let version = if let Option::Some(version_variant) = version_variant {
                                let version = format!("{}-{}", version, version_variant);
                                Version::from_str(version.as_str())?
                            } else {
                                Version::from_str(version.as_str())?
                            };

                            Option::Some(Specific {
                                vendor,
                                product,
                                variant,
                                version,
                            })
                        } else {
                            Option::None
                        }
                    } else {
                        Option::None
                    }
                } else {
                    Option::None
                }
            } else {
                Option::None
            };

            let kind = Kind::from(resource_type, kind, specific)?;
            let location = match star {
                Some(star) => ResourceLocation::Star(StarKey::from_str(star.as_str())?),
                None => ResourceLocation::Unassigned,
            };
            let status = Status::from_str(status.as_str())?;

            let stub = ResourceStub {
                address,
                kind: kind.into(),
                properties: Default::default(), // not implemented yet...
                status,
            };

            let record = ResourceRecord {
                stub: stub,
                location,
            };

            Ok(record)
        }

        match wrap(row) {
            Ok(record) => Ok(record),
            Err(err) => {
                error!("{}", err.to_string());
                Err(sqlx::error::Error::Decode("resource record".into()))
            }
        }
    }

}

#[cfg(test)]
pub mod test {
    use crate::error::Error;
    use crate::registry::Registry;
    use crate::resource::{Kind, UserBaseKind};
    use crate::star::core::resource::registry::Registration;
    use crate::star::StarKey;
    use mesh_portal::version::latest::entity::request::query::Query;
    use mesh_portal::version::latest::entity::request::select::{Select, SelectIntoPayload};
    use mesh_portal::version::latest::id::Address;
    use mesh_portal::version::latest::pattern::{AddressKindPath, AddressKindPattern};
    use mesh_portal::version::latest::payload::Primitive;
    use mesh_portal::version::latest::resource::Status;
    use mesh_portal_versions::version::v0_0_1::entity::request::select::SelectKind;
    use std::convert::TryInto;
    use std::str::FromStr;
    use mesh_portal_versions::version::v0_0_1::security::{Access, AccessGrant, AccessGrantKind};

    #[tokio::test]
    pub async fn test_nuke() -> Result<(), Error> {
        let registry = Registry::new().await?;
        registry.nuke().await?;
        Ok(())
    }

    #[tokio::test]
    pub async fn test_create() -> Result<(), Error> {
        let registry = Registry::new().await?;
        registry.nuke().await?;

        let address = Address::from_str("localhost")?;
        let hyperuser = Address::from_str("hyperspace:users:hyperuser")?;
        let registration = Registration {
            address: address.clone(),
            kind: Kind::Space,
            registry: Default::default(),
            properties: Default::default(),
            owner: hyperuser.clone()
        };
        registry.register(&registration).await?;

        let address = Address::from_str("localhost:mechtron")?;
        let registration = Registration {
            address: address.clone(),
            kind: Kind::Mechtron,
            registry: Default::default(),
            properties: Default::default(),
            owner: hyperuser
        };
        registry.register(&registration).await?;

        let star = StarKey::central();
        registry.assign(&address, &star).await?;
        registry.set_status(&address, &Status::Ready).await?;
        registry.sequence(&address).await?;
        let record = registry.locate(&address).await?;

        let result = registry.query(&address, &Query::AddressKindPath).await?;
        let kind_path: AddressKindPath = result.try_into()?;

        let pattern = AddressKindPattern::from_str("**")?;
        let select = Select {
            pattern,
            properties: Default::default(),
            into_payload: SelectIntoPayload::Addresses,
            kind: SelectKind::Initial,
        };

        let addresses = registry.select(&select).await?;

        assert_eq!(addresses.len(), 2);

        Ok(())
    }

    #[tokio::test]
    pub async fn test_access() -> Result<(), Error> {
        let registry = Registry::new().await?;
        registry.nuke().await?;

        let hyperuser = Address::from_str("hyperspace:users:hyperuser")?;
        let superuser = Address::from_str("localhost:users:superuser")?;
        let scott = Address::from_str("localhost:app:users:scott")?;
        let app = Address::from_str("localhost:app")?;

        let registration = Registration {
            address: hyperuser.clone(),
            kind: Kind::Space,
            registry: Default::default(),
            properties: Default::default(),
            owner: hyperuser.clone()
        };
        registry.register(&registration).await?;

        let address = Address::from_str("localhost")?;
        let registration = Registration {
            address: address.clone(),
            kind: Kind::Space,
            registry: Default::default(),
            properties: Default::default(),
            owner: hyperuser.clone()
        };
        registry.register(&registration).await?;

        let address = Address::from_str("localhost:users")?;
        let registration = Registration {
            address: address.clone(),
            kind: Kind::UserBase(UserBaseKind::Keycloak),
            registry: Default::default(),
            properties: Default::default(),
            owner: hyperuser.clone()
        };
        registry.register(&registration).await?;

        let registration = Registration {
            address: superuser.clone(),
            kind: Kind::UserBase(UserBaseKind::Keycloak),
            registry: Default::default(),
            properties: Default::default(),
            owner: hyperuser.clone()
        };
        registry.register(&registration).await?;


        let registration = Registration {
            address: app.clone(),
            kind: Kind::User,
            registry: Default::default(),
            properties: Default::default(),
            owner: superuser.clone(),
        };
        registry.register(&registration).await?;

        let address = Address::from_str("localhost:app:users")?;
        let registration = Registration {
            address: address.clone(),
            kind: Kind::UserBase(UserBaseKind::Keycloak),
            registry: Default::default(),
            properties: Default::default(),
            owner: app.clone()
        };
        registry.register(&registration).await?;


        let registration = Registration {
            address: scott.clone(),
            kind: Kind::User,
            registry: Default::default(),
            properties: Default::default(),
            owner: app.clone()
        };
        registry.register(&registration).await?;

        let address = Address::from_str("localhost:app:mechtron")?;
        let registration = Registration {
            address: address.clone(),
            kind: Kind::Mechtron,
            registry: Default::default(),
            properties: Default::default(),
            owner: app.clone()
        };
        registry.register(&registration).await?;


        let grant= AccessGrant{
            kind: AccessGrantKind::Super,
            on_point: AddressKindPattern::from_str("localhost:**")?,
            to_point: superuser.clone().try_into()?,
            by_particle: hyperuser.clone()
        };
        registry.grant(&grant).await?;

        let grant= AccessGrant{
            kind: AccessGrantKind::Super,
            on_point: AddressKindPattern::from_str("localhost:app:**")?,
            to_point: app.clone().try_into()?,
            by_particle: superuser.clone()
        };

        registry.grant(&grant).await?;


        let access = registry.access(&hyperuser,&superuser).await?;
        assert_eq!(access.is_super(), true);

        let access = registry.access(&superuser,&app ).await?;
        assert_eq!(access.is_super(), true);

        let access = registry.access(&app,&scott).await?;
        assert_eq!(access.is_super(), true);

        let access = registry.access(&scott,&app ).await?;
        assert_eq!(access.is_super(), false);

        Ok(())



    }
}
