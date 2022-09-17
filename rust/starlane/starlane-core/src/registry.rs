use crate::error::Error;
use crate::frame::{ResourceHostAction, StarMessagePayload};
use crate::message::{ProtoStarMessage, ProtoStarMessageTo, Reply, ReplyKind};

use crate::databases::lookup_registry_db;
use crate::particle::properties_config;
use cosmic_universe::command::request::delete::Delete;
use cosmic_universe::command::request::select::{SelectKind, SubSelect};
use cosmic_universe::id::{ArtifactSubKind, FileSubKind, Tks, UserBaseSubKind};
use cosmic_universe::id2::{BaseSubKind};
use cosmic_universe::parse::{CamelCase, Domain, SkewerCase};
use cosmic_universe::particle::Details;
use cosmic_universe::security::{
    Access, AccessGrant, AccessGrantKind, EnumeratedAccess, Permissions, PermissionsMask,
    PermissionsMaskKind, Privilege, Privileges,
};
use cosmic_universe::selector::SubKindSelector;
use cosmic_universe::hyper::{Location, ParticleRecord};
use futures::{FutureExt, StreamExt};
use mesh_portal::error::MsgErr;
use mesh_portal::version::latest::command::common::{PropertyMod, SetProperties, SetRegistry};
use mesh_portal::version::latest::entity::request::create::{
    Create, KindTemplate, PointSegFactory, Strategy,
};
use mesh_portal::version::latest::entity::request::get::{Get, GetOp};
use mesh_portal::version::latest::entity::request::query::{Query, QueryResult};
use mesh_portal::version::latest::entity::request::select::{Select, SelectIntoPayload};
use mesh_portal::version::latest::entity::request::set::Set;
use mesh_portal::version::latest::entity::request::{Method, Rc};
use mesh_portal::version::latest::id::{KindParts, Point, Specific, Version};
use mesh_portal::version::latest::messaging::ReqShell;
use mesh_portal::version::latest::particle::{Properties, Property, Status, Stub};
use mesh_portal::version::latest::payload::{PayloadMap, PrimitiveList, Substance};
use mesh_portal::version::latest::selector::specific::{
    ProductSelector, VariantSelector, VendorSelector,
};
use mesh_portal::version::latest::selector::{
    ExactSegment, GenericKindSelector, KindSelector, PointKindHierarchy, PointKindSeg,
    PointSegSelector, PointSelector,
};
use mesh_portal::version::latest::util::ValuePattern;
use mysql::prelude::TextQuery;
use sqlx::postgres::{PgArguments, PgPoolOptions, PgRow};
use sqlx::{Connection, Executor, Pool, Postgres, Row, Transaction};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::convert::TryInto;
use std::num::ParseIntError;
use std::ops::{Deref, Index};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::mpsc;
use cosmic_universe::id::{BaseKind, Kind};

lazy_static! {
    pub static ref HYPERUSER: Point = Point::from_str("hyperspace:users:hyperuser").expect("point");
}

pub type RegistryApi = Arc<Registry>;

pub struct Registry {
    pool: Pool<Postgres>,
}

impl Registry {
    pub async fn new() -> Result<Self, Error> {
        let db = lookup_registry_db();
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(db.to_uri().as_str())
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
        //        let database= format!("CREATE DATABASE IF NOT EXISTS {}", REGISTRY_DATABASE );

        let particles = r#"CREATE TABLE IF NOT EXISTS particles (
         id SERIAL PRIMARY KEY,
         point TEXT NOT NULL,
         point_segment TEXT NOT NULL,
         parent TEXT NOT NULL,
         base TEXT NOT NULL,
         sub TEXT,
         provider TEXT,
         vendor TEXT,
         product TEXT,
         variant TEXT,
         version TEXT,
         version_variant TEXT,
         location TEXT,
         status TEXT NOT NULL,
         sequence INTEGER DEFAULT 0,
         owner TEXT,
         UNIQUE(point),
         UNIQUE(parent,point_segment)
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
          FOREIGN KEY (by_particle) REFERENCES particles (id)
        )"#;

        let labels = r#"
       CREATE TABLE IF NOT EXISTS labels (
          id SERIAL PRIMARY KEY,
	      resource_id INTEGER NOT NULL,
	      key TEXT NOT NULL,
	      value TEXT,
          UNIQUE(key,value),
          FOREIGN KEY (resource_id) REFERENCES particles (id)
        )"#;

        /// note that a tag may reference an point NOT in this database
        /// therefore it does not have a FOREIGN KEY constraint
        let tags = r#"
       CREATE TABLE IF NOT EXISTS tags(
          id SERIAL PRIMARY KEY,
          parent TEXT NOT NULL,
          tag TEXT NOT NULL,
          point TEXT NOT NULL,
          UNIQUE(tag)
        )"#;

        let properties = r#"CREATE TABLE IF NOT EXISTS properties (
         id SERIAL PRIMARY KEY,
	     resource_id INTEGER NOT NULL,
         key TEXT NOT NULL,
         value TEXT NOT NULL,
         lock BOOLEAN NOT NULL,
         FOREIGN KEY (resource_id) REFERENCES particles (id),
         UNIQUE(resource_id,key)
        )"#;

        let point_index =
            "CREATE UNIQUE INDEX IF NOT EXISTS resource_point_index ON particles(point)";
        let point_segment_parent_index = "CREATE UNIQUE INDEX IF NOT EXISTS resource_point_segment_parent_index ON particles(parent,point_segment)";
        let access_grants_index =
            "CREATE INDEX IF NOT EXISTS query_root_index ON access_grants(query_root)";

        let mut conn = self.pool.acquire().await?;
        let mut transaction = conn.begin().await?;
        transaction.execute(particles).await?;
        transaction.execute(access_grants).await?;
        /*
        transaction.execute(labels).await?;
        transaction.execute(tags).await?;
         */
        transaction.execute(properties).await?;
        transaction.execute(point_index).await?;
        transaction.execute(point_segment_parent_index).await?;
        transaction.execute(access_grants_index).await?;
        transaction.commit().await?;

        Ok(())
    }

    async fn nuke(&self) -> Result<(), Error> {
        let mut conn = self.pool.acquire().await?;
        let mut trans = conn.begin().await?;
        trans.execute("DROP TABLE particles CASCADE").await;
        trans.execute("DROP TABLE access_grants CASCADE").await;
        trans.execute("DROP TABLE properties CASCADE").await;
        trans.commit().await?;
        self.setup().await?;
        Ok(())
    }

    pub async fn register(&self, registration: &Registration) -> Result<Details, RegError> {
        /*
        async fn check<'a>( registration: &Registration,  trans:&mut Transaction<Postgres>, ) -> Result<(),RegError> {
            let params = RegistryParams::from_registration(registration)?;
            let count:u64 = sqlx::query_as("SELECT count(*) as count from particles WHERE parent=? AND point_segment=?").bind(params.parent).bind(params.point_segment).fetch_one(trans).await?;
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
        let params = RegistryParams::from_registration(registration)?;

        let count = sqlx::query_as::<Postgres, Count>(
            "SELECT count(*) as count from particles WHERE parent=$1 AND point_segment=$2",
        )
        .bind(params.parent.to_string())
        .bind(params.point_segment.to_string())
        .fetch_one(&mut trans)
        .await?;

        if count.0 > 0 {
            return Err(RegError::Dupe);
        }

        let statement = format!("INSERT INTO particles (point,point_segment,base,kind,vendor,product,variant,version,version_variant,parent,owner,status) VALUES ('{}','{}','{}',{},{},{},{},{},{},'{}','{}','Pending')", params.point, params.point_segment, params.base, opt(&params.sub), opt(&params.vendor), opt(&params.product), opt(&params.variant), opt(&params.version), opt(&params.version_variant), params.parent, params.owner.to_string());
        trans.execute(statement.as_str()).await?;

        for (_, property_mod) in registration.properties.iter() {
            match property_mod {
                PropertyMod::Set { key, value, lock } => {
                    let lock: usize = match lock {
                        true => 1,
                        false => 0,
                    };
                    let statement = format!("INSERT INTO properties (resource_id,key,value,lock) VALUES ((SELECT id FROM particles WHERE parent='{}' AND point_segment='{}'),'{}','{}',{})", params.parent, params.point_segment, key.to_string(), value.to_string(), lock);
                    trans.execute(statement.as_str()).await?;
                }
                PropertyMod::UnSet(key) => {
                    let statement = format!("DELETE FROM properties WHERE resource_id=(SELECT id FROM particles WHERE parent='{}' AND point_segment='{}') AND key='{}' AND lock=false", params.parent, params.point_segment, key.to_string());
                    trans.execute(statement.as_str()).await?;
                }
            }
        }
        trans.commit().await?;
        Ok(Details {
            stub: Stub {
                point: registration.point.clone(),
                kind: registration.kind.clone(),
                status: Status::Pending,
            },
            properties: Default::default(),
        })
    }

    pub async fn assign(&self, point: &Point, location: &Point) -> Result<(), Error> {
        let parent = point
            .parent()
            .ok_or("expecting parent since we have already established the segments are >= 2")?;
        let point_segment = point
            .last_segment()
            .ok_or("expecting a last_segment since we know segments are >= 2")?;
        let statement = format!(
            "UPDATE particles SET location='{}' WHERE parent='{}' AND point_segment='{}'",
            location.to_string(),
            parent.to_string(),
            point_segment.to_string()
        );
        let mut conn = self.pool.acquire().await?;
        let mut trans = conn.begin().await?;
        trans.execute(statement.as_str()).await?;
        trans.commit().await?;
        Ok(())
    }

    pub async fn set_status(&self, point: &Point, status: &Status) -> Result<(), Error> {
        let parent = point
            .parent()
            .ok_or("particle must have a parent")?
            .to_string();
        let point_segment = point
            .last_segment()
            .ok_or("particle must have a last segment")?
            .to_string();
        let status = status.to_string();
        let statement = format!(
            "UPDATE particles SET status='{}' WHERE parent='{}' AND point_segment='{}'",
            status.to_string(),
            parent,
            point_segment
        );
        let mut conn = self.pool.acquire().await?;
        let mut trans = conn.begin().await?;
        trans.execute(statement.as_str()).await?;
        trans.commit().await?;
        Ok(())
    }

    pub async fn set_properties(
        &self,
        point: &Point,
        properties: &SetProperties,
    ) -> Result<(), Error> {
        let mut conn = self.pool.acquire().await?;
        let mut trans = conn.begin().await?;
        let parent = point
            .parent()
            .ok_or("particle must have a parent")?
            .to_string();
        let point_segment = point
            .last_segment()
            .ok_or("particle must have a last segment")?
            .to_string();

        for (_, property_mod) in properties.iter() {
            match property_mod {
                PropertyMod::Set { key, value, lock } => {
                    let lock = match *lock {
                        true => 1,
                        false => 0,
                    };

                    let statement = format!("INSERT INTO properties (resource_id,key,value,lock) VALUES ((SELECT id FROM particles WHERE parent='{}' AND point_segment='{}'),'{}' ,'{}','{}') ON CONFLICT(resource_id,key) DO UPDATE SET value='{}' WHERE lock=false", parent, point_segment, key.to_string(), value.to_string(), value.to_string(), lock);
                    trans.execute(statement.as_str()).await?;
                }
                PropertyMod::UnSet(key) => {
                    let statement = format!("DELETE FROM properties WHERE resource_id=(SELECT id FROM particles WHERE parent='{}' AND point_segment='{}') AND key='{}' AND lock=false", parent, point_segment, key.to_string());
                    trans.execute(statement.as_str()).await?;
                }
            }
        }
        trans.commit().await?;
        Ok(())
    }

    pub async fn sequence(&self, point: &Point) -> Result<u64, Error> {
        struct Sequence(u64);

        impl sqlx::FromRow<'_, PgRow> for Sequence {
            fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
                let v: i32 = row.get(0);
                Ok(Self(v as u64))
            }
        }

        let mut conn = self.pool.acquire().await?;
        let mut trans = conn.begin().await?;
        let parent = point
            .parent()
            .ok_or("expecting parent since we have already established the segments are >= 2")?;
        let point_segment = point
            .last_segment()
            .ok_or("expecting a last_segment since we know segments are >= 2")?;
        let statement = format!(
            "UPDATE particles SET sequence=sequence+1 WHERE parent='{}' AND point_segment='{}'",
            parent.to_string(),
            point_segment.to_string()
        );

        trans.execute(statement.as_str()).await?;
        let sequence = sqlx::query_as::<Postgres, Sequence>(
            "SELECT DISTINCT sequence FROM particles WHERE parent=$1 AND point_segment=$2",
        )
        .bind(parent.to_string())
        .bind(point_segment.to_string())
        .fetch_one(&mut trans)
        .await?;
        trans.commit().await?;

        Ok(sequence.0)
    }

    pub async fn get_properties(&self, point: &Point) -> Result<Properties, Error> {
        let parent = point.parent().ok_or("expected a parent")?;
        let point_segment = point
            .last_segment()
            .ok_or("expected last point_segment")?
            .to_string();

        let mut conn = self.pool.acquire().await?;
        let properties = sqlx::query_as::<Postgres,LocalProperty>("SELECT key,value,lock FROM properties WHERE resource_id=(SELECT id FROM particles WHERE parent=$1 AND point_segment=$2)").bind(parent.to_string()).bind(point_segment).fetch_all(& mut conn).await?;
        let mut map = HashMap::new();
        for p in properties {
            map.insert(p.key.clone(), p.into());
        }
        Ok(map)
    }

    pub async fn locate(&self, point: &Point) -> Result<ParticleRecord, Error> {
        if point.is_local_root() {
            return Ok(ParticleRecord::root());
        }

        let mut conn = self.pool.acquire().await?;
        let parent = point.parent().ok_or("expected a parent")?;
        let point_segment = point
            .last_segment()
            .ok_or("expected last point_segment")?
            .to_string();

        let mut record = sqlx::query_as::<Postgres, StarlaneParticleRecord>(
            "SELECT DISTINCT * FROM particles as r WHERE parent=$1 AND point_segment=$2",
        )
        .bind(parent.to_string())
        .bind(point_segment.clone())
        .fetch_one(&mut conn)
        .await?;
        let mut record: ParticleRecord = record.into();
        let properties = sqlx::query_as::<Postgres,LocalProperty>("SELECT key,value,lock FROM properties WHERE resource_id=(SELECT id FROM particles WHERE parent=$1 AND point_segment=$2)").bind(parent.to_string()).bind(point_segment).fetch_all(& mut conn).await?;
        let mut map = HashMap::new();
        for p in properties {
            map.insert(p.key.clone(), p.into());
        }
        record.details.properties = map;

        Ok(record)
    }

    pub async fn query(&self, point: &Point, query: &Query) -> Result<QueryResult, Error> {
        let mut kind_path = PointKindHierarchy::new(point.route.clone(), vec![]);
        let route = point.route.clone();

        let mut segments = vec![];
        for segment in &point.segments {
            segments.push(segment.clone());
            let point = Point {
                route: route.clone(),
                segments: segments.clone(),
            };
            let record = self.locate(&point).await?;
            let kind_segment = PointKindSeg {
                segment: record
                    .details
                    .stub
                    .point
                    .last_segment()
                    .ok_or("expected at least one segment")?,
                kind: record.details.stub.kind,
            };
            kind_path = kind_path.push(kind_segment);
        }
        return Ok(QueryResult::PointKindHierarchy(kind_path));
    }

    pub async fn delete(&self, delete: &Delete) -> Result<PrimitiveList, Error> {
        let select = delete.clone().into();
        let list = self.select(&select).await?;
        if !list.is_empty() {
            let mut points = String::new();
            for (index, point) in list.iter().enumerate() {
                if let Substance::Point(point) = &**point {
                    points.push_str(format!("'{}'", point.to_string()).as_str());
                    if index < list.len() - 1 {
                        points.push_str(", ");
                    }
                }
            }

            let mut conn = self.pool.acquire().await?;
            let statement = format!("DELETE FROM particles WHERE point IN [{}]", points);
            sqlx::query(statement.as_str()).execute(&mut conn).await?;
        }

        Ok(list)
    }

    #[async_recursion]
    pub async fn select(&self, select: &Select) -> Result<PrimitiveList, Error> {
        let point = select.pattern.query_root();

        let hierarchy = self
            .query(&point, &Query::PointKindHierarchy)
            .await?
            .try_into()?;

        let sub_select_hops = select.pattern.sub_select_hops();
        let sub_select = select
            .clone()
            .sub_select(point.clone(), sub_select_hops, hierarchy);
        let mut list = self.sub_select(&sub_select).await?;
        if select.pattern.matches_root() {
            list.push(Stub {
                point: Point::root(),
                kind: Kind::Root,
                status: Status::Ready,
            });
        }

        let list = sub_select.into_payload.to_primitive(list)?;

        Ok(list)
    }

    #[async_recursion]
    async fn sub_select(&self, sub_select: &SubSelect) -> Result<Vec<Stub>, Error> {
        // build a 'matching so far' query.  Here we will find every child that matches the subselect
        // these matches are used to then query children for additional matches if there are more hops.
        // all of these matches will be filtered to see if they match the ENTIRE select before returning results.
        let mut params: Vec<String> = vec![];
        let mut where_clause = String::new();
        let mut index = 1;
        where_clause.push_str("parent=$1");
        params.push(sub_select.point.to_string());

        if let Option::Some(hop) = sub_select.hops.first() {
            match &hop.segment_selector {
                PointSegSelector::Exact(exact) => {
                    index = index + 1;
                    where_clause.push_str(format!(" AND point_segment=${}", index).as_str());
                    match exact {
                        ExactSegment::PointSeg(point) => {
                            params.push(point.to_string());
                        }
                        ExactSegment::Version(version) => {
                            params.push(version.to_string());
                        }
                    }
                }
                _ => {}
            }

            match &hop.kind_selector.kind {
                GenericKindSelector::Any => {}
                GenericKindSelector::Exact(kind) => {
                    index = index + 1;
                    where_clause.push_str(format!(" AND base=${}", index).as_str());
                    params.push(kind.to_string());
                }
            }

            match &hop.kind_selector.kind {
                GenericKindSelector::Any => {}
                GenericKindSelector::Exact(kind) => match &hop.kind_selector.sub {
                    SubKindSelector::Any => {}
                    SubKindSelector::Exact(sub) => match sub {
                        None => {}
                        Some(sub) => {
                            index = index + 1;
                            where_clause.push_str(format!(" AND sub=${}", index).as_str());
                            params.push(sub.to_string());
                        }
                    },
                },
            }

            match &hop.kind_selector.specific {
                ValuePattern::Any => {}
                ValuePattern::None => {}
                ValuePattern::Pattern(specific) => {
                    match &specific.provider {
                        VendorSelector::Any => {}
                        VendorSelector::Exact(provider) => {
                            index = index + 1;
                            where_clause.push_str(format!(" AND provider=${}", index).as_str());
                            params.push(provider.to_string());
                        }
                    }
                    match &specific.vendor {
                        VendorSelector::Any => {}
                        VendorSelector::Exact(vendor) => {
                            index = index + 1;
                            where_clause.push_str(format!(" AND vendor=${}", index).as_str());
                            params.push(vendor.to_string());
                        }
                    }
                    match &specific.product {
                        ProductSelector::Any => {}
                        ProductSelector::Exact(product) => {
                            index = index + 1;
                            where_clause.push_str(format!(" AND product=${}", index).as_str());
                            params.push(product.to_string());
                        }
                    }
                    match &specific.variant {
                        VariantSelector::Any => {}
                        VariantSelector::Exact(variant) => {
                            index = index + 1;
                            where_clause.push_str(format!(" AND variant=${}", index).as_str());
                            params.push(variant.to_string());
                        }
                    }
                }
            }
        }

        let matching_so_far_statement = format!(
            "SELECT DISTINCT * FROM particles as r WHERE {}",
            where_clause
        );

        let mut query =
            sqlx::query_as::<Postgres, StarlaneParticleRecord>(matching_so_far_statement.as_str());
        for param in params {
            query = query.bind(param);
        }

        let mut conn = self.pool.acquire().await?;
        let mut matching_so_far = query.fetch_all(&mut conn).await?;

        let mut matching_so_far: Vec<ParticleRecord> =
            matching_so_far.into_iter().map(|m| m.into()).collect();
        let mut matching_so_far: Vec<Stub> =
            matching_so_far.into_iter().map(|r| r.into()).collect();

        let mut child_stub_matches = vec![];

        // if we have more hops we need to see if there are matching children
        if !sub_select.hops.is_empty() {
            let mut hops = sub_select.hops.clone();
            let hop = hops.first().unwrap();
            match hop.segment_selector {
                PointSegSelector::Recursive => {}
                _ => {
                    hops.remove(0);
                }
            }

            for stub in &matching_so_far {
                if let Option::Some(last_segment) = stub.point.last_segment() {
                    let point = sub_select.point.push_segment(last_segment.clone())?;
                    let point_tks_path = sub_select.hierarchy.push(PointKindSeg {
                        segment: last_segment,
                        kind: stub.kind.clone(),
                    });
                    let sub_select =
                        sub_select
                            .clone()
                            .sub_select(point.clone(), hops.clone(), point_tks_path);
                    let more_stubs = self.sub_select(&sub_select).await?;
                    for stub in more_stubs.into_iter() {
                        child_stub_matches.push(stub);
                    }
                }
            }

            // the records matched the present hop (which we needed for deeper searches) however
            // they may not or may not match the ENTIRE select pattern therefore they must be filtered
            matching_so_far.retain(|stub| {
                let point_tks_path = sub_select.hierarchy.push(PointKindSeg {
                    segment: stub
                        .point
                        .last_segment()
                        .expect("expecting at least one segment"),
                    kind: stub.kind.clone(),
                });
                sub_select.pattern.matches(&point_tks_path)
            });

            matching_so_far.append(&mut child_stub_matches);
        }

        let stubs: Vec<Stub> = matching_so_far
            .into_iter()
            .map(|record| record.into())
            .collect();

        Ok(stubs)
    }

    async fn grant(&self, access_grant: &AccessGrant) -> Result<(), Error> {
        let mut conn = self.pool.acquire().await?;
        match &access_grant.kind {
            AccessGrantKind::Super => {
                sqlx::query("INSERT INTO access_grants (kind,query_root,on_point,to_point,by_particle) VALUES ('super',$1,$2,$3,(SELECT id FROM particles WHERE point=$4))")
                        .bind(access_grant.on_point.query_root().to_string())
                        .bind(access_grant.on_point.clone().to_string())
                        .bind(access_grant.to_point.clone().to_string())
                        .bind(access_grant.by_particle.to_string()).execute(& mut conn).await?;
            }
            AccessGrantKind::Privilege(privilege) => {
                sqlx::query("INSERT INTO access_grants (kind,data,query_root,on_point,to_point,by_particle) VALUES ('priv',$1,$2,$3,$4,(SELECT id FROM particles WHERE point=$5))")
                        .bind(privilege.to_string() )
                        .bind(access_grant.on_point.query_root().to_string())
                        .bind(access_grant.on_point.clone().to_string())
                        .bind(access_grant.to_point.clone().to_string())
                        .bind(access_grant.by_particle.to_string()).execute(& mut conn).await?;
            }
            AccessGrantKind::PermissionsMask(mask) => {
                sqlx::query("INSERT INTO access_grants (kind,data,query_root,on_point,to_point,by_particle) VALUES ('perm',$1,$2,$3,$4,(SELECT id FROM particles WHERE point=$5))")
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
    pub async fn access(&self, to: &Point, on: &Point) -> Result<Access, Error> {
        let mut conn = self.pool.acquire().await?;

        struct Owner(bool);

        impl sqlx::FromRow<'_, PgRow> for Owner {
            fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
                Ok(Self(row.get(0)))
            }
        }

        //if 'to' owns 'on' then grant Owner access
        let has_owner = sqlx::query_as::<Postgres, Owner>(
            "SELECT count(*) > 0 as owner FROM particles WHERE point=$1 AND owner=$2",
        )
        .bind(on.to_string())
        .bind(to.to_string())
        .fetch_one(&mut conn)
        .await?
        .0;

        if *HYPERUSER == *to {
            if has_owner {
                return Ok(Access::Super);
            } else {
                return Ok(Access::SuperOwner);
            }
        }

        if *to == *on && has_owner {
            return Ok(Access::Owner);
        }

        let to_kind_path: PointKindHierarchy = self
            .query(&to, &Query::PointKindHierarchy)
            .await?
            .try_into()?;
        let on_kind_path: PointKindHierarchy = self
            .query(&on, &Query::PointKindHierarchy)
            .await?
            .try_into()?;

        let mut traversal = on.clone();
        let mut privileges = Privileges::none();
        let mut permissions = Permissions::none();
        let mut level_ands: Vec<Vec<PermissionsMask>> = vec![];
        loop {
            let mut access_grants= sqlx::query_as::<Postgres, WrappedIndexedAccessGrant>("SELECT access_grants.*,particles.point as by_particle FROM access_grants,particles WHERE access_grants.query_root=$1 AND particles.id=access_grants.by_particle").bind(traversal.to_string() ).fetch_all(& mut conn).await?;
            let mut access_grants: Vec<AccessGrant> = access_grants
                .into_iter()
                .map(|a| a.into())
                .map(|a: IndexedAccessGrant| a.into())
                .collect();
            access_grants.retain(|access_grant| {
                access_grant.to_point.matches(&to_kind_path)
                    && access_grant.on_point.matches(&on_kind_path)
            });
            // check for any superusers
            for access_grant in &access_grants {
                let by_access = self.access(&access_grant.by_particle, &on).await?;
                match &access_grant.kind {
                    AccessGrantKind::Super => {
                        if by_access.has_super() {
                            if has_owner {
                                return Ok(Access::SuperOwner);
                            } else {
                                return Ok(Access::Super);
                            }
                        }
                    }
                    AccessGrantKind::Privilege(privilege) => {
                        if by_access.has_full() {
                            privileges = privileges | privilege;
                        }
                    }
                    AccessGrantKind::PermissionsMask(mask) => {
                        if by_access.has_full() {
                            if let PermissionsMaskKind::Or = mask.kind {
                                permissions.or(&mask.permissions);
                            }
                        }
                    }
                }
            }
            access_grants.retain(|a| {
                if let AccessGrantKind::PermissionsMask(mask) = &a.kind {
                    if let PermissionsMaskKind::And = mask.kind {
                        return true;
                    }
                }
                false
            });
            let ands: Vec<PermissionsMask> = access_grants
                .into_iter()
                .map(|a| {
                    if let AccessGrantKind::PermissionsMask(mask) = a.kind {
                        return mask;
                    }
                    panic!("expected a mask")
                })
                .collect();
            // save for later when we traverse back down
            level_ands.push(ands);

            // now reduce the segments of the traversal or break if it's root
            if traversal.is_root() {
                break;
            } else {
                traversal.segments.pop();
            }
        }

        if has_owner {
            return Ok(Access::Owner);
        }

        level_ands.reverse();
        for level in level_ands {
            for mask in level {
                permissions.and(&mask.permissions);
            }
        }

        let access = EnumeratedAccess {
            privileges,
            permissions,
        };

        let access = Access::Enumerated(access);

        Ok(access)
    }

    pub async fn chown(&self, on: &PointSelector, owner: &Point, by: &Point) -> Result<(), Error> {
        let select = Select {
            pattern: on.clone(),
            properties: Default::default(),
            into_payload: SelectIntoPayload::Points,
            kind: SelectKind::Initial,
        };

        let selection = self.select(&select).await?;
        let mut conn = self.pool.acquire().await?;
        let mut trans = conn.begin().await?;
        for on in selection.list {
            let on = (*on).try_into()?;
            let access = self.access(by, &on).await?;

            if !access.has_super() {
                return Err("only a super can change owners".into());
            }

            sqlx::query("UPDATE particles SET owner=$1 WHERE point=$2")
                .bind(owner.to_string())
                .bind(on.to_string())
                .execute(&mut trans)
                .await?;
        }
        trans.commit().await?;
        Ok(())
    }

    pub async fn list_access(
        &self,
        to: &Option<&Point>,
        on: &PointSelector,
    ) -> Result<Vec<IndexedAccessGrant>, Error> {
        let select = Select {
            pattern: on.clone(),
            properties: Default::default(),
            into_payload: SelectIntoPayload::Points,
            kind: SelectKind::Initial,
        };

        let to = match to.as_ref() {
            None => None,
            Some(to) => Some(
                self.query(to, &Query::PointKindHierarchy)
                    .await?
                    .try_into()?,
            ),
        };

        let selection = self.select(&select).await?;
        let mut all_access_grants = HashMap::new();
        let mut conn = self.pool.acquire().await?;
        for on in selection.list {
            let on: Point = (*on).try_into()?;
            let access_grants= sqlx::query_as::<Postgres, WrappedIndexedAccessGrant>("SELECT access_grants.*,particles.point as by_particle FROM access_grants,particles WHERE access_grants.query_root=$1 AND particles.id=access_grants.by_particle").bind(on.to_string() ).fetch_all(& mut conn).await?;
            let mut access_grants: Vec<IndexedAccessGrant> =
                access_grants.into_iter().map(|a| a.into()).collect();

            access_grants.retain(|a| match to.as_ref() {
                None => true,
                Some(to) => a.to_point.matches(to),
            });
            for access_grant in access_grants {
                all_access_grants.insert(access_grant.id.clone(), access_grant);
            }
        }

        let mut all_access_grants: Vec<IndexedAccessGrant> = all_access_grants
            .values()
            .into_iter()
            .map(|a| a.clone())
            .collect();

        all_access_grants.sort();

        Ok(all_access_grants)
    }

    pub async fn remove_access(&self, id: i32, to: &Point) -> Result<(), Error> {
        let mut conn = self.pool.acquire().await?;
        let access_grant: IndexedAccessGrant = sqlx::query_as::<Postgres, WrappedIndexedAccessGrant>("SELECT access_grants.*,particles.point as by_particle FROM access_grants,particles WHERE access_grants.id=$1 AND particles.id=access_grants.by_particle").bind(id ).fetch_one(& mut conn).await?.into();
        let access = self.access(to, &access_grant.by_particle).await?;
        if access.has_full() {
            let mut trans = conn.begin().await?;
            sqlx::query("DELETE FROM access_grants WHERE id=$1")
                .bind(id)
                .execute(&mut trans)
                .await?;
            trans.commit().await?;
            Ok(())
        } else {
            Err(format!("'{}' could not revoked grant {} because it does not have full access (super or owner) on {}", to.to_string(), id, access_grant.by_particle.to_string() ).into())
        }
    }
}

fn opt<S: ToString>(opt: &Option<S>) -> String {
    match opt {
        None => "null".to_string(),
        Some(value) => {
            format!("'{}'", value.to_string())
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

pub struct WrappedIndexedAccessGrant(IndexedAccessGrant);

impl Into<IndexedAccessGrant> for WrappedIndexedAccessGrant {
    fn into(self) -> IndexedAccessGrant {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct IndexedAccessGrant {
    pub id: i32,
    pub access_grant: AccessGrant,
}

impl Eq for IndexedAccessGrant {}

impl PartialEq<Self> for IndexedAccessGrant {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Ord for IndexedAccessGrant {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.id < other.id {
            Ordering::Greater
        } else if self.id < other.id {
            Ordering::Less
        } else {
            Ordering::Equal
        }
    }
}

impl PartialOrd<Self> for IndexedAccessGrant {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.id < other.id {
            Some(Ordering::Greater)
        } else if self.id < other.id {
            Some(Ordering::Less)
        } else {
            Some(Ordering::Equal)
        }
    }
}

impl Deref for IndexedAccessGrant {
    type Target = AccessGrant;

    fn deref(&self) -> &Self::Target {
        &self.access_grant
    }
}

impl Into<AccessGrant> for IndexedAccessGrant {
    fn into(self) -> AccessGrant {
        self.access_grant
    }
}

impl sqlx::FromRow<'_, PgRow> for WrappedIndexedAccessGrant {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        fn wrap(row: &PgRow) -> Result<IndexedAccessGrant, Error> {
            let id: i32 = row.get("id");
            let kind: &str = row.get("kind");
            let kind = match kind {
                "super" => AccessGrantKind::Super,
                "priv" => {
                    let privilege: String = row.get("data");
                    AccessGrantKind::Privilege(Privilege::from_str(privilege.as_str())?)
                }
                "perm" => {
                    let mask: &str = row.get("data");
                    let mask = PermissionsMask::from_str(mask)?;
                    AccessGrantKind::PermissionsMask(mask)
                }
                what => {
                    panic!(format!(
                        "don't know how to handle access grant kind {}",
                        what
                    ))
                }
            };

            let on_point: &str = row.get("on_point");
            let to_point: &str = row.get("to_point");
            let by_particle: &str = row.get("by_particle");

            let access_grant = AccessGrant {
                kind,
                on_point: PointSelector::from_str(on_point)?,
                to_point: PointSelector::from_str(to_point)?,
                by_particle: Point::from_str(by_particle)?,
            };
            Ok(IndexedAccessGrant { id, access_grant })
        }

        match wrap(row) {
            Ok(record) => Ok(WrappedIndexedAccessGrant(record)),
            Err(err) => {
                error!("{}", err.to_string());
                Err(sqlx::error::Error::Decode(err.into()))
            }
        }
    }
}

struct StarlaneParticleRecord {
    pub details: Details,
    pub location: Location,
}

impl Into<ParticleRecord> for StarlaneParticleRecord {
    fn into(self) -> ParticleRecord {
        ParticleRecord {
            details: self.details,
            location: self.location,
        }
    }
}

impl sqlx::FromRow<'_, PgRow> for StarlaneParticleRecord {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        fn wrap(row: &PgRow) -> Result<StarlaneParticleRecord, Error> {
            let parent: String = row.get("parent");
            let point_segment: String = row.get("point_segment");
            let base: String = row.get("base");
            let sub: Option<CamelCase> = match row.get("sub") {
                Some(sub) => {
                    let sub: String = sub;
                    Some(CamelCase::from_str(sub.as_str())?)
                }
                None => None,
            };

            let provider: Option<String> = row.get("provider");
            let vendor: Option<String> = row.get("vendor");
            let product: Option<String> = row.get("product");
            let variant: Option<String> = row.get("variant");
            let version: Option<String> = row.get("version");
            let version_variant: Option<String> = row.get("version_variant");
            let location: Option<String> = row.get("location");
            let status: String = row.get("status");

            let point = Point::from_str(parent.as_str())?;
            let point = point.push(point_segment)?;
            let base = BaseKind::from_str(base.as_str())?;

            let specific = if let Option::Some(provider) = provider {
                if let Option::Some(vendor) = vendor {
                    if let Option::Some(product) = product {
                        if let Option::Some(variant) = variant {
                            if let Option::Some(version) = version {
                                let version = if let Option::Some(version_variant) = version_variant
                                {
                                    let version = format!("{}-{}", version, version_variant);
                                    Version::from_str(version.as_str())?
                                } else {
                                    Version::from_str(version.as_str())?
                                };

                                let provider = Domain::from_str(provider.as_str())?;
                                let vendor = Domain::from_str(vendor.as_str())?;
                                let product = SkewerCase::from_str(product.as_str())?;
                                let variant = SkewerCase::from_str(variant.as_str())?;

                                Some(Specific {
                                    provider,
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
                }
            } else {
                Option::None
            };

            let kind = KindParts::new(base, sub, specific);
            let kind: Kind = kind.try_into()?;

            let location = match location {
                Some(location) => Location::Somewhere(Point::from_str(location.as_str())?),
                None => Location::Nowhere,
            };
            let status = Status::from_str(status.as_str())?;

            let stub = Stub {
                point,
                kind,
                status,
            };

            let details = Details {
                stub,
                properties: Default::default(), // not implemented yet...
            };

            let record = StarlaneParticleRecord { details, location };

            Ok(record)
        }

        match wrap(row) {
            Ok(record) => Ok(record),
            Err(err) => {
                error!("{}", err.to_string());
                Err(sqlx::error::Error::Decode("particle record".into()))
            }
        }
    }
}

#[cfg(test)]
pub mod test {
    use crate::error::Error;
    use crate::particle::Kind;
    use crate::registry::{Registration, Registry};
    use cosmic_universe::command::request::select::SelectKind;
    use cosmic_universe::entity::request::select::SelectKind;
    use cosmic_universe::id::ToPoint;
    use cosmic_universe::id::UserBaseSubKind;
    use cosmic_universe::security::{
        Access, AccessGrant, AccessGrantKind, Permissions, PermissionsMask, PermissionsMaskKind,
        Privilege,
    };
    use mesh_portal::version::latest::entity::request::query::Query;
    use mesh_portal::version::latest::entity::request::select::{Select, SelectIntoPayload};
    use mesh_portal::version::latest::id::Point;
    use mesh_portal::version::latest::particle::Status;
    use mesh_portal::version::latest::payload::Primitive;
    use mesh_portal::version::latest::selector::{PointKindHierarchy, PointSelector};
    use std::convert::TryInto;
    use std::str::FromStr;
    use cosmic_universe::id::Kind;

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

        let point = Point::from_str("localhost")?;
        let hyperuser = Point::from_str("hyperspace:users:hyperuser")?;
        let registration = Registration {
            point: point.clone(),
            kind: Kind::Space,
            registry: Default::default(),
            properties: Default::default(),
            owner: hyperuser.clone(),
        };
        registry.register(&registration).await?;

        let point = Point::from_str("localhost:mechtron")?;
        let registration = Registration {
            point: point.clone(),
            kind: Kind::Mechtron,
            registry: Default::default(),
            properties: Default::default(),
            owner: hyperuser,
        };
        registry.register(&registration).await?;

        let location = StarKey::central().to_point();
        registry.assign(&point, &location).await?;
        registry.set_status(&point, &Status::Ready).await?;
        registry.sequence(&point).await?;
        let record = registry.locate(&point).await?;

        let result = registry.query(&point, &Query::PointKindHierarchy).await?;
        let kind_path: PointKindHierarchy = result.try_into()?;

        let pattern = PointSelector::from_str("**")?;
        let select = Select {
            pattern,
            properties: Default::default(),
            into_payload: SelectIntoPayload::Points,
            kind: SelectKind::Initial,
        };

        let points = registry.select(&select).await?;

        assert_eq!(points.len(), 2);

        Ok(())
    }

    #[tokio::test]
    pub async fn test_access() -> Result<(), Error> {
        let registry = Registry::new().await?;
        registry.nuke().await?;

        let hyperuser = Point::from_str("hyperspace:users:hyperuser")?;
        let superuser = Point::from_str("localhost:users:superuser")?;
        let scott = Point::from_str("localhost:app:users:scott")?;
        let app = Point::from_str("localhost:app")?;
        let mechtron = Point::from_str("localhost:app:mechtron")?;
        let localhost = Point::from_str("localhost")?;

        let registration = Registration {
            point: Point::root(),
            kind: Kind::Root,
            registry: Default::default(),
            properties: Default::default(),
            owner: hyperuser.clone(),
        };
        registry.register(&registration).await?;

        let registration = Registration {
            point: Point::from_str("hyperspace")?,
            kind: Kind::Space,
            registry: Default::default(),
            properties: Default::default(),
            owner: hyperuser.clone(),
        };
        registry.register(&registration).await?;

        let registration = Registration {
            point: Point::from_str("hyperspace:users")?,
            kind: Kind::UserBase(UserBaseSubKind::Keycloak),
            registry: Default::default(),
            properties: Default::default(),
            owner: hyperuser.clone(),
        };
        registry.register(&registration).await?;

        let registration = Registration {
            point: hyperuser.clone(),
            kind: Kind::Space,
            registry: Default::default(),
            properties: Default::default(),
            owner: hyperuser.clone(),
        };
        registry.register(&registration).await?;

        let registration = Registration {
            point: localhost.clone(),
            kind: Kind::Space,
            registry: Default::default(),
            properties: Default::default(),
            owner: hyperuser.clone(),
        };
        registry.register(&registration).await?;

        let point = Point::from_str("localhost:users")?;
        let registration = Registration {
            point: point.clone(),
            kind: Kind::UserBase(UserBaseSubKind::Keycloak),
            registry: Default::default(),
            properties: Default::default(),
            owner: hyperuser.clone(),
        };
        registry.register(&registration).await?;

        let registration = Registration {
            point: superuser.clone(),
            kind: Kind::UserBase(UserBaseSubKind::Keycloak),
            registry: Default::default(),
            properties: Default::default(),
            owner: hyperuser.clone(),
        };
        registry.register(&registration).await?;

        let registration = Registration {
            point: app.clone(),
            kind: Kind::App,
            registry: Default::default(),
            properties: Default::default(),
            owner: superuser.clone(),
        };
        registry.register(&registration).await?;

        let app_userbase = Point::from_str("localhost:app:users")?;
        let registration = Registration {
            point: app_userbase.clone(),
            kind: Kind::UserBase(UserBaseSubKind::Keycloak),
            registry: Default::default(),
            properties: Default::default(),
            owner: app.clone(),
        };
        registry.register(&registration).await?;

        let registration = Registration {
            point: scott.clone(),
            kind: Kind::User,
            registry: Default::default(),
            properties: Default::default(),
            owner: app.clone(),
        };
        registry.register(&registration).await?;

        let registration = Registration {
            point: mechtron.clone(),
            kind: Kind::Mechtron,
            registry: Default::default(),
            properties: Default::default(),
            owner: app.clone(),
        };
        registry.register(&registration).await?;

        let grant = AccessGrant {
            kind: AccessGrantKind::Super,
            on_point: PointSelector::from_str("localhost+:**")?,
            to_point: superuser.clone().try_into()?,
            by_particle: hyperuser.clone(),
        };
        registry.grant(&grant).await?;

        let grant = AccessGrant {
            kind: AccessGrantKind::PermissionsMask(PermissionsMask::from_str("+csd-Rwx")?),
            on_point: PointSelector::from_str("localhost:app+:**")?,
            to_point: PointSelector::from_str("localhost:app:users:**<User>")?,
            by_particle: app.clone(),
        };
        registry.grant(&grant).await?;

        let grant = AccessGrant {
            kind: AccessGrantKind::PermissionsMask(PermissionsMask::from_str("+csd-rwX")?),
            on_point: PointSelector::from_str("localhost:app:**<Mechtron>")?,
            to_point: PointSelector::from_str("localhost:app:users:**<User>")?,
            by_particle: app.clone(),
        };
        registry.grant(&grant).await?;

        let grant = AccessGrant {
            kind: AccessGrantKind::PermissionsMask(PermissionsMask::from_str("+CSD-RWX")?),
            on_point: PointSelector::from_str("localhost:users:superuser")?,
            to_point: scott.clone().try_into()?,
            by_particle: app.clone(),
        };
        registry.grant(&grant).await?;

        let grant = AccessGrant {
            kind: AccessGrantKind::Privilege(Privilege::Single("property:email:read".to_string())),
            on_point: PointSelector::from_str("localhost:app:users:**<User>")?,
            to_point: PointSelector::from_str("localhost:app:**<Mechtron>")?,
            by_particle: app.clone(),
        };
        registry.grant(&grant).await?;

        let access = registry.access(&hyperuser, &superuser).await?;
        assert_eq!(access.has_super(), true);

        let access = registry.access(&superuser, &localhost).await?;
        assert_eq!(access.has_super(), true);

        let access = registry.access(&superuser, &app).await?;
        assert_eq!(access.has_super(), true);

        let access = registry.access(&app, &scott).await?;
        assert_eq!(access.has_super(), false);
        assert_eq!(access.has_owner(), true);
        assert_eq!(access.has_full(), true);

        let access = registry.access(&scott, &superuser).await?;
        assert_eq!(access.has_super(), false);
        assert_eq!(access.has_full(), false);
        assert_eq!(access.permissions().to_string(), "csd-rwx".to_string());

        // should fail because app is not the owner of localhost:app yet...
        let access = registry.access(&scott, &app).await?;
        assert_eq!(access.has_super(), false);
        assert_eq!(access.permissions().to_string(), "csd-rwx".to_string());

        // must have super to chagne ownership
        let app_pattern = PointSelector::from_str("localhost:app+:**")?;
        assert!(registry.chown(&app_pattern, &app, &scott).await.is_err());
        // this should work:
        assert!(registry.chown(&app_pattern, &app, &superuser).await.is_ok());

        // now the previous rule should work since app now owns itself.
        let access = registry.access(&scott, &app).await?;
        assert_eq!(access.has_super(), false);
        assert_eq!(access.permissions().to_string(), "csd-Rwx".to_string());

        let access = registry.access(&scott, &superuser).await?;
        assert_eq!(access.has_super(), false);
        assert_eq!(access.permissions().to_string(), "csd-rwx".to_string());

        // test masked OR permissions
        let access = registry.access(&scott, &mechtron).await?;
        assert_eq!(access.has_super(), false);
        assert_eq!(access.permissions().to_string(), "csd-RwX".to_string());

        // now test AND permissions (masking Read)
        let grant = AccessGrant {
            kind: AccessGrantKind::PermissionsMask(PermissionsMask::from_str("&csd-rwX")?),
            on_point: PointSelector::from_str("localhost:app:**<Mechtron>")?,
            to_point: PointSelector::from_str("localhost:app:users:**<User>")?,
            by_particle: app.clone(),
        };
        registry.grant(&grant).await?;

        let access = registry.access(&scott, &mechtron).await?;
        assert_eq!(access.has_super(), false);
        assert_eq!(access.permissions().to_string(), "csd-rwX".to_string());

        let access = registry.access(&mechtron, &scott).await?;
        assert_eq!(access.has_super(), false);
        assert_eq!(access.permissions().to_string(), "csd-rwx".to_string());
        assert!(access.check_privilege("property:email:read").is_ok());

        let access_grants = registry
            .list_access(&None, &PointSelector::from_str("+:**")?)
            .await?;

        println!(
            "{: <4}{:<6}{:<20}{:<40}{:<40}{:<40}",
            "id", "grant", "data", "on", "to", "by"
        );
        for access_grant in &access_grants {
            println!(
                "{: <4}{:<6}{:<20}{:<40}{:<40}{:<40}",
                access_grant.id,
                access_grant.access_grant.kind.to_string(),
                match &access_grant.kind {
                    AccessGrantKind::Super => "".to_string(),
                    AccessGrantKind::Privilege(prv) => prv.to_string(),
                    AccessGrantKind::PermissionsMask(perm) => perm.to_string(),
                },
                access_grant.access_grant.on_point.to_string(),
                access_grant.to_point.to_string(),
                access_grant.by_particle.to_string()
            );
            //            registry.remove_access(access_grant.id, &app ).await?;
        }

        Ok(())
    }
}

pub enum RegError {
    Dupe,
    Error(Error),
}

impl ToString for RegError {
    fn to_string(&self) -> String {
        match self {
            RegError::Dupe => "Dupe".to_string(),
            RegError::Error(error) => error.to_string(),
        }
    }
}
impl From<sqlx::Error> for RegError {
    fn from(e: sqlx::Error) -> Self {
        RegError::Error(e.into())
    }
}

impl From<tokio::sync::oneshot::error::RecvError> for RegError {
    fn from(e: tokio::sync::oneshot::error::RecvError) -> Self {
        RegError::Error(Error::from_internal(format!("{}", e.to_string())))
    }
}
impl From<Error> for RegError {
    fn from(e: Error) -> Self {
        RegError::Error(e)
    }
}

impl<T> From<mpsc::error::SendError<T>> for RegError {
    fn from(e: mpsc::error::SendError<T>) -> Self {
        RegError::Error(e.into())
    }
}

impl From<&str> for RegError {
    fn from(e: &str) -> Self {
        RegError::Error(e.into())
    }
}

impl From<RegError> for Error {
    fn from(r: RegError) -> Self {
        Error::new(r.to_string().as_str())
    }
}

#[derive(Clone)]
pub struct Registration {
    pub point: Point,
    pub kind: Kind,
    pub registry: SetRegistry,
    pub properties: SetProperties,
    pub owner: Point,
}

pub struct RegistryParams {
    pub point: String,
    pub point_segment: String,
    pub base: String,
    pub sub: Option<String>,
    pub provider: Option<Domain>,
    pub vendor: Option<Domain>,
    pub product: Option<SkewerCase>,
    pub variant: Option<SkewerCase>,
    pub version: Option<String>,
    pub version_variant: Option<String>,
    pub parent: String,
    pub owner: Point,
}

impl RegistryParams {
    pub fn from_registration(registration: &Registration) -> Result<Self, Error> {
        let point_segment = match registration.point.segments.last() {
            None => "".to_string(),
            Some(segment) => segment.to_string(),
        };
        let parent = match registration.point.parent() {
            None => "".to_string(),
            Some(parent) => parent.to_string(),
        };

        let base = registration.kind.to_base().to_string();
        let sub = registration.kind.sub();

        let provider = match &registration.kind.specific() {
            None => Option::None,
            Some(specific) => Option::Some(specific.provider.clone()),
        };

        let vendor = match &registration.kind.specific() {
            None => Option::None,
            Some(specific) => Option::Some(specific.vendor.clone()),
        };

        let product = match &registration.kind.specific() {
            None => Option::None,
            Some(specific) => Option::Some(specific.product.clone()),
        };

        let variant = match &registration.kind.specific() {
            None => Option::None,
            Some(specific) => Option::Some(specific.variant.clone()),
        };

        let version = match &registration.kind.specific() {
            None => Option::None,
            Some(specific) => {
                let version = &specific.version;
                Option::Some(format!(
                    "{}.{}.{}",
                    version.major, version.minor, version.patch
                ))
            }
        };

        let version_variant = match &registration.kind.specific() {
            None => Option::None,
            Some(specific) => {
                let version = &specific.version;
                if !version.pre.is_empty() {
                    Option::Some(version.pre.to_string())
                } else {
                    Option::None
                }
            }
        };

        Ok(RegistryParams {
            point: registration.point.to_string(),
            point_segment: point_segment,
            parent,
            base,
            sub: sub.into(),
            provider,
            vendor,
            product,
            variant,
            version,
            version_variant,
            owner: registration.owner.clone(),
        })
    }
}

pub fn match_kind(template: &KindTemplate) -> Result<Kind, Error> {
    let base: BaseKind = BaseKind::from_str(template.base.to_string().as_str())?;
    Ok(match base {
        BaseKind::Root => Kind::Root,
        BaseKind::Space => Kind::Space,
        BaseKind::Base => match &template.sub {
            None => {
                return Err("kind must be set for Base".into());
            }
            Some(sub_kind) => {
                let kind = BaseSubKind::from_str(sub_kind.as_str())?;
                if template.specific.is_some() {
                    return Err("BaseKind cannot have a Specific".into());
                }
                return Ok(Kind::Base(kind));
            }
        },
        BaseKind::User => Kind::User,
        BaseKind::App => Kind::App,
        BaseKind::Mechtron => Kind::Mechtron,
        BaseKind::FileSystem => Kind::FileSystem,
        BaseKind::File => match &template.sub {
            None => return Err("expected kind for File".into()),
            Some(kind) => {
                let file_kind = FileSubKind::from_str(kind.as_str())?;
                return Ok(Kind::File(file_kind));
            }
        },
        BaseKind::Database => {
            unimplemented!("need to write a SpecificPattern matcher...")
        }
        BaseKind::BundleSeries => Kind::BundleSeries,
        BaseKind::Bundle => Kind::Bundle,
        BaseKind::Artifact => match &template.sub {
            None => {
                return Err("expected Sub for Artirtact".into());
            }
            Some(sub) => {
                let artifact_kind = ArtifactSubKind::from_str(sub.as_str())?;
                return Ok(Kind::Artifact(artifact_kind));
            }
        },
        BaseKind::Control => Kind::Control,
        BaseKind::UserBase => match &template.sub {
            None => {
                return Err("SubKind must be set for UserBase<?>".into());
            }
            Some(sub) => {
                let specific =
                    Specific::from_str("starlane.io:redhat.com:keycloak:community:18.0.0")?;
                let sub = UserBaseSubKind::OAuth(specific);
                Kind::UserBase(sub)
            }
        },
    })
}

impl Registry {
    pub async fn set(&self, set: &Set) -> Result<(), Error> {
        self.set_properties(&set.point, &set.properties).await
    }

    pub async fn get(&self, get: &Get) -> Result<Substance, Error> {
        match &get.op {
            GetOp::State => {
                return Err("Registry does not handle GetOp::State operations".into());
                /*
                let mut proto = ProtoStarMessage::new();
                proto.to(ProtoStarMessageTo::Resource(get.point.clone()));
                proto.payload = StarMessagePayload::ResourceHost(ResourceHostAction::GetState(get.point.clone()));
                if let Ok(Reply::Payload(payload)) = self.skel.messaging_api
                    .star_exchange(proto, ReplyKind::Payload, "get state from driver")
                    .await {
                    Ok(payload)
                } else {
                    Err("could not get state".into())
                }
                 */
            }
            GetOp::Properties(keys) => {
                println!("GET PROPERTIES for {}", get.point.to_string());
                let properties = self.get_properties(&get.point).await?;
                let mut map = PayloadMap::new();
                for (index, property) in properties.iter().enumerate() {
                    println!("\tprop{}", property.0.clone());
                    map.insert(
                        property.0.clone(),
                        Substance::Text(property.1.value.clone()),
                    );
                }

                Ok(Substance::Map(map))
            }
        }
    }

    pub async fn create(&self, create: &Create) -> Result<Details, Error> {
        let child_kind = match_kind(&create.template.kind)?;
        let stub = match &create.template.point.child_segment_template {
            PointSegFactory::Exact(child_segment) => {
                let point = create.template.point.parent.push(child_segment.clone());
                match &point {
                    Ok(_) => {}
                    Err(err) => {
                        eprintln!("RC CREATE error: {}", err.to_string());
                    }
                }
                let point = point?;

                let properties =
                    properties_config(&child_kind).fill_create_defaults(&create.properties)?;
                properties_config(&child_kind).check_create(&properties)?;

                let registration = Registration {
                    point: point.clone(),
                    kind: child_kind.clone(),
                    registry: create.registry.clone(),
                    properties,
                    owner: Point::root(),
                };
                println!("creating {}", point.to_string());
                let mut result = self.register(&registration).await;

                // if strategy is ensure then a dupe is GOOD!
                if create.strategy == Strategy::Ensure {
                    if let Err(RegError::Dupe) = result {
                        result = Ok(self.locate(&point).await?.details);
                    }
                }

                println!("result {}? {}", point.to_string(), result.is_ok());
                result?
            }
            PointSegFactory::Pattern(pattern) => {
                if !pattern.contains("%") {
                    return Err("AddressSegmentTemplate::Pattern must have at least one '%' char for substitution".into());
                }
                loop {
                    let index = self.sequence(&create.template.point.parent).await?;
                    let child_segment = pattern.replace("%", index.to_string().as_str());
                    let point = create.template.point.parent.push(child_segment.clone())?;
                    let registration = Registration {
                        point: point.clone(),
                        kind: child_kind.clone(),
                        registry: create.registry.clone(),
                        properties: create.properties.clone(),
                        owner: Point::root(),
                    };

                    match self.register(&registration).await {
                        Ok(stub) => return Ok(stub),
                        Err(RegError::Dupe) => {
                            // continue loop
                        }
                        Err(RegError::Error(error)) => {
                            return Err(error);
                        }
                    }
                }
            }
        };
        Ok(stub)
    }

    pub async fn cmd_select(&self, select: &Select) -> Result<Substance, Error> {
        let list = Substance::List(self.select(select).await?);
        Ok(list)
    }

    pub async fn cmd_query(&self, to: &Point, query: &Query) -> Result<Substance, Error> {
        let result = Substance::Text(self.query(to, query).await?.to_string());
        Ok(result)
    }
}
