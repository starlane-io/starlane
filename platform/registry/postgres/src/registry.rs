use async_trait::async_trait;
use serde_derive::{Deserialize, Serialize};
use sqlx::pool::PoolConnection;
use sqlx::postgres::{PgPoolOptions, PgRow};
use sqlx::Pool;
use sqlx::{Acquire, Executor, Postgres, Row, Transaction};
use starlane_hyperspace::registry::err::RegErr;
use starlane_hyperspace::registry::{Registration, RegistryApi};
use starlane_macros::push_loc;
use starlane_platform_for_postgres::database::PostgresDatabaseHandle;
use starlane_platform_for_postgres::service::{DbKey, PostgresService, PostgresServiceHandle};
/// embedded postgres for local development environments is slated to be removed in favor of
/// Postgres provided by `DockerDesktopFoundation`
// pub mod embed;
use starlane_space::command::common::{PropertyMod, SetProperties};
use starlane_space::command::direct::create::Strategy;
use starlane_space::command::direct::delete::Delete;
use starlane_space::command::direct::get::{Get, GetOp};
use starlane_space::command::direct::query::{Query, QueryResult};
use starlane_space::command::direct::select::{Select, SelectIntoSubstance, SelectKind, SubSelect};
use starlane_space::command::direct::set::Set;
use starlane_space::err::SpaceErr;
use starlane_space::hyper::{ParticleLocation, ParticleRecord};
use starlane_space::kind::{BaseKind, Kind, KindParts, Specific};
use starlane_space::loc::{StarKey, ToBaseKind, Version};
use starlane_space::log::Logger;
use starlane_space::parse::util::{parse_errs, result};
use starlane_space::parse::{CamelCase, Domain, SkewerCase};
use starlane_space::particle::{Details, Properties, Property, Status, Stub};
use starlane_space::point::Point;
use starlane_space::security::{
    Access, AccessGrant, AccessGrantKind, EnumeratedAccess, IndexedAccessGrant, Permissions,
    PermissionsMask, PermissionsMaskKind, Privilege, Privileges,
};
use starlane_space::selector::specific::{
    ProductSelector, ProviderSelector, VariantSelector, VendorSelector,
};
use starlane_space::selector::{
    ExactPointSeg, KindBaseSelector, PointHierarchy, PointKindSeg, PointSegSelector, Selector,
    SubKindSelector,
};
use starlane_space::status::Handle;
use starlane_space::substance::{Substance, SubstanceList, SubstanceMap};
use starlane_space::util::ValuePattern;
use starlane_space::HYPERUSER;
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::Arc;

pub struct PostgresRegistry {
    logger: Logger,
    handle: PostgresDatabaseHandle,
}

impl PostgresRegistry {
    pub async fn new(handle: PostgresDatabaseHandle, logger: Logger) -> Result<Self, RegErr> {
        let logger = push_loc!((logger, Point::global_registry()));

        let registry = Self {
            handle,
            logger: logger.clone(),
        };

        match registry.setup().await {
            Ok(_) => {}
            Err(err) => {
                let message = err.to_string();
                logger.error(format!("database setup failed {} ", message));
                return Err(err);
            }
        }

        Ok(registry)
    }

    async fn setup(&self) -> Result<(), RegErr> {
        //        let database= format!("CREATE DATABASE IF NOT EXISTS {}", REGISTRY_DATABASE );

        /// reset mode of 'none' will not let the db be deleted
        let mode = r#"CREATE TYPE reset_mode_enum AS ENUM ('None', 'Scorch');
                            CREATE TABLE reset_mode (mode reset_mode_enum DEFAULT 'none' NOT NULL UNIQUE);
w                           INSERT INTO reset_mode VALUES ('None');"#;

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
         star TEXT,
         host TEXT,
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

        let mut conn = self.handle.acquire().await?;
        let mut transaction = conn.begin().await?;
        transaction.execute(particles).await?;
        transaction.execute(access_grants).await?;
        /*
        transaction.execute(labels).await?;
        transaction.execute(tags).await?;
         */
        transaction.execute(mode).await?;
        transaction.execute(properties).await?;
        transaction.execute(point_index).await?;
        transaction.execute(point_segment_parent_index).await?;
        transaction.execute(access_grants_index).await?;
        transaction.commit().await?;

        Ok(())
    }
}

#[async_trait]
impl RegistryApi for PostgresRegistry {
    async fn scorch<'a>(&'a self) -> Result<(), RegErr> {
        self.logger.info("scorching database!");
        let mut conn = self.handle.acquire().await?;

        let mut trans = conn.begin().await?;

        struct CanScorch(bool);

        impl CanScorch {
            fn can(&self) -> bool {
                self.0
            }
        }

        impl sqlx::FromRow<'_, PgRow> for CanScorch {
            fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
                let v: i64 = row.get(0);
                let v = v != 0;
                Ok(Self(v))
            }
        }

        let scorch = sqlx::query_as::<Postgres, CanScorch>(
            "SELECT count(*) FROM reset_mode WHERE mode=('Scorch')",
        )
        .fetch_one(&mut *trans)
        .await?;

        if !scorch.can() {
            let err = "database has scorch guard enabled.  To change this: 'INSERT INTO reset_mode VALUES ('Scorch')'";
            self.logger.error(err);
            Result::Err(RegErr::NoScorch)?;
        }

        trans.execute("DROP TABLE particles CASCADE").await?;
        trans.execute("DROP TABLE access_grants CASCADE").await?;
        trans.execute("DROP TABLE properties CASCADE").await?;
        trans.commit().await?;
        self.setup().await?;
        Ok(())
    }

    async fn register<'a>(&'a self, registration: &'a Registration) -> Result<(), RegErr> {
        /*
        async fn check<'a>( registration: &Registration,  trans:&mut Transaction<Postgres>, ) -> Result<(),Erroror> {
            let params = RegistryParams::from_registration(registration)?;
            let count:u64 = sqlx::query_as("SELECT count(*) as count from particles WHERE parent=? AND point_segment=?").bind(params.parent).bind(params.point_segment).fetch_one(trans).await?;
            if count > 0 {
                Err(Erroror::Dupe)
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

        let mut conn = self.handle.acquire().await?;
        let mut trans = conn.begin().await?;
        let params = RegistryParams::from_registration(registration)?;

        let count = sqlx::query_as::<Postgres, Count>(
            "SELECT count(*) as count from particles WHERE point=$1",
        )
        .bind(params.point.clone())
        .fetch_one(&mut *trans)
        .await?;

        if count.0 > 0 {
            // returning ok on Override for now which is the expected behavior but not the desired
            // result.... will revisit this and properly do an update when the time comes -- Scott
            trans.rollback().await?;
            if registration.strategy == Strategy::Ensure
                || registration.strategy == Strategy::Override
            {
                return Ok(());
            } else {
                return Err(RegErr::dupe());
            }
        }

        let statement = format!("INSERT INTO particles (point,point_segment,base,sub,provider,vendor,product,variant,version,version_variant,parent,owner,status) VALUES ('{}','{}','{}',{},{},{},{},{},{},{},'{}','{}','Pending')", params.point, params.point_segment, params.base, opt(&params.sub), opt(&params.provider), opt(&params.vendor), opt(&params.product), opt(&params.variant), opt(&params.version), opt(&params.version_variant), params.parent, params.owner.to_string());
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
        Ok(())
    }

    async fn assign_star<'a>(&'a self, point: &'a Point, star: &'a Point) -> Result<(), RegErr> {
        let parent = point.parent().ok_or(RegErr::expected_parent(point))?;
        let point_segment = point.last_segment().ok_or("expecting a last_segment")?;

        let statement = "UPDATE particles SET star=$1 WHERE parent=$2 AND point_segment=$3";

        let mut conn = self.handle.acquire().await?;
        let mut trans = conn.begin().await?;

        trans
            .execute(
                sqlx::query(statement)
                    .bind(star.to_string())
                    .bind(parent.to_string())
                    .bind(point_segment.to_string()),
            )
            .await?;

        trans.commit().await?;
        Ok(())
    }

    async fn assign_host<'a>(&'a self, point: &'a Point, host: &'a Point) -> Result<(), RegErr> {
        let parent = point.parent().ok_or(RegErr::expected_parent(point))?;
        let point_segment = point.last_segment().ok_or("expecting a last_segment")?;

        let statement = "UPDATE particles SET host=$1, WHERE parent=$2 AND point_segment=$3";

        let mut conn = self.handle.acquire().await?;
        let mut trans = conn.begin().await?;

        trans
            .execute(
                sqlx::query(statement)
                    .bind(host.to_string())
                    .bind(parent.to_string())
                    .bind(point_segment.to_string()),
            )
            .await?;

        trans.commit().await?;
        Ok(())
    }

    async fn set_status<'a>(&'a self, point: &'a Point, status: &'a Status) -> Result<(), RegErr> {
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
        let mut conn = self.handle.acquire().await?;
        let mut trans = conn.begin().await?;
        trans.execute(statement.as_str()).await?;
        trans.commit().await?;
        Ok(())
    }

    async fn set_properties<'a>(
        &'a self,
        point: &'a Point,
        properties: &'a SetProperties,
    ) -> Result<(), RegErr> {
        let mut conn = self.handle.acquire().await?;
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

    async fn sequence<'a>(&'a self, point: &'a Point) -> Result<u64, RegErr> {
        struct Sequence(u64);

        impl sqlx::FromRow<'_, PgRow> for Sequence {
            fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
                let v: i32 = row.get(0);
                Ok(Self(v as u64))
            }
        }

        let mut conn = self.handle.acquire().await?;
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
        .fetch_one(&mut *trans)
        .await?;
        trans.commit().await?;

        Ok(sequence.0)
    }

    async fn get_properties<'a>(&'a self, point: &'a Point) -> Result<Properties, RegErr> {
        let parent = point.parent().ok_or("expected a parent")?;
        let point_segment = point
            .last_segment()
            .ok_or("expected last point_segment")?
            .to_string();

        let mut conn = self.handle.acquire().await?;
        let properties = sqlx::query_as::<Postgres, LocalProperty>("SELECT key,value,lock FROM properties WHERE resource_id=(SELECT id FROM particles WHERE parent=$1 AND point_segment=$2)").bind(parent.to_string()).bind(point_segment).fetch_all(&mut *conn).await?;
        let mut map = HashMap::new();
        for p in properties {
            map.insert(p.key.clone(), p.into());
        }
        Ok(map)
    }

    async fn record<'a>(&'a self, point: &'a Point) -> Result<ParticleRecord, RegErr> {
        if point.is_local_root() {
            return Ok(ParticleRecord::root());
        }

        let mut conn = self.handle.acquire().await?;
        let parent = point.parent().ok_or("expected a parent")?;
        let point_segment = point
            .last_segment()
            .ok_or("expected last point_segment")?
            .to_string();

        let mut record = sqlx::query_as::<Postgres, PostgresParticleRecord>(
            "SELECT DISTINCT * FROM particles as r WHERE parent=$1 AND point_segment=$2",
        )
        .bind(parent.to_string())
        .bind(point_segment.clone())
        .fetch_one(&mut *conn)
        .await?;
        let mut record: ParticleRecord = record.into();
        let properties = sqlx::query_as::<Postgres, LocalProperty>("SELECT key,value,lock FROM properties WHERE resource_id=(SELECT id FROM particles WHERE parent=$1 AND point_segment=$2)").bind(parent.to_string()).bind(point_segment).fetch_all(&mut *conn).await?;
        let mut map = HashMap::new();
        for p in properties {
            map.insert(p.key.clone(), p.into());
        }
        record.details.properties = map;

        Ok(record)
    }

    async fn query<'a>(
        &'a self,
        point: &'a Point,
        query: &'a Query,
    ) -> Result<QueryResult, RegErr> {
        let mut kind_path = PointHierarchy::new(point.route.clone(), vec![]);
        let route = point.route.clone();

        let mut segments = vec![];
        for segment in &point.segments {
            segments.push(segment.clone());
            let point = Point {
                route: route.clone(),
                segments: segments.clone(),
            };
            let record = self.record(&point).await?;
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
        return Ok(QueryResult::PointHierarchy(kind_path));
    }

    async fn delete<'a>(&'a self, delete: &'a Delete) -> Result<SubstanceList, RegErr> {
        let mut select = delete.clone().into();
        let list = self.select(&mut select).await?;
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

            let mut conn = self.handle.acquire().await?;
            let statement = format!("DELETE FROM particles WHERE point IN [{}]", points);
            sqlx::query(statement.as_str()).execute(&mut *conn).await?;
        }

        Ok(list)
    }

    //    #[async_recursion]
    async fn sub_select<'a>(&'a self, sub_select: &'a SubSelect) -> Result<Vec<Stub>, RegErr> {
        // build a 'matching so far' query.  Here we will find every child that matches the subselect
        // these matches are used to then query children for additional matches if there are more hops.
        // all of these matches will be filtered to see if they match the ENTIRE select before returning results.
        let mut params: Vec<String> = vec![];
        let mut where_clause = String::new();
        let mut index = 1;
        where_clause.push_str("parent=$1");
        params.push(sub_select.point.to_string());

        if let Option::Some(hop) = sub_select.hops.first() {
            let x = &hop.kind_selector;
            match &hop.segment_selector {
                PointSegSelector::Exact(exact) => {
                    index = index + 1;
                    where_clause.push_str(format!(" AND point_segment=${}", index).as_str());
                    match exact {
                        ExactPointSeg::PointSeg(point) => {
                            params.push(point.to_string());
                        }
                        ExactPointSeg::Version(version) => {
                            params.push(version.to_string());
                        }
                    }
                }
                _ => {}
            }

            match &hop.kind_selector.base {
                KindBaseSelector::Always => {}
                KindBaseSelector::Exact(kind) => {
                    index = index + 1;
                    where_clause.push_str(format!(" AND base=${}", index).as_str());
                    params.push(kind.to_string());
                }
                KindBaseSelector::Never => {}
            }

            match &hop.kind_selector.base {
                KindBaseSelector::Always => {}
                KindBaseSelector::Exact(kind) => match &hop.kind_selector.sub {
                    SubKindSelector::Always => {}
                    SubKindSelector::Exact(sub) => {
                        index = index + 1;
                        where_clause.push_str(format!(" AND sub=${}", index).as_str());
                        params.push(sub.to_string());
                    }
                    SubKindSelector::None => {}
                    SubKindSelector::Never => {}
                },
                KindBaseSelector::Never => {}
            }

            match &hop.kind_selector.specific {
                ValuePattern::Always => {}
                ValuePattern::Never => {}
                ValuePattern::Pattern(specific) => {
                    match &specific.provider {
                        ProviderSelector::Always => {}
                        ProviderSelector::Exact(provider) => {
                            index = index + 1;
                            where_clause.push_str(format!(" AND provider=${}", index).as_str());
                            params.push(provider.to_string());
                        }
                    }
                    match &specific.vendor {
                        VendorSelector::Always => {}
                        VendorSelector::Exact(vendor) => {
                            index = index + 1;
                            where_clause.push_str(format!(" AND vendor=${}", index).as_str());
                            params.push(vendor.to_string());
                        }
                    }
                    match &specific.product {
                        ProductSelector::Always => {}
                        ProductSelector::Exact(product) => {
                            index = index + 1;
                            where_clause.push_str(format!(" AND product=${}", index).as_str());
                            params.push(product.to_string());
                        }
                    }
                    match &specific.variant {
                        VariantSelector::Always => {}
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
            sqlx::query_as::<Postgres, PostgresParticleRecord>(matching_so_far_statement.as_str());
        for param in params {
            query = query.bind(param);
        }

        let mut conn = self.handle.acquire().await?;
        let mut matching_so_far = query.fetch_all(&mut *conn).await?;

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
                sub_select.pattern.matches_found(&point_tks_path)
            });

            matching_so_far.append(&mut child_stub_matches);
        }

        let stubs: Vec<Stub> = matching_so_far
            .into_iter()
            .map(|record| record.into())
            .collect();

        Ok(stubs)
    }

    async fn grant<'a>(&'a self, access_grant: &'a AccessGrant) -> Result<(), RegErr> {
        let mut conn = self.handle.acquire().await?;
        match &access_grant.kind {
            AccessGrantKind::Super => {
                sqlx::query("INSERT INTO access_grants (kind,query_root,on_point,to_point,by_particle) VALUES ('super',$1,$2,$3,(SELECT id FROM particles WHERE point=$4))")
                    .bind(access_grant.on_point.query_root().to_string())
                    .bind(access_grant.on_point.clone().to_string())
                    .bind(access_grant.to_point.clone().to_string())
                    .bind(access_grant.by_particle.to_string()).execute(&mut *conn).await?;
            }
            AccessGrantKind::Privilege(privilege) => {
                sqlx::query("INSERT INTO access_grants (kind,data,query_root,on_point,to_point,by_particle) VALUES ('priv',$1,$2,$3,$4,(SELECT id FROM particles WHERE point=$5))")
                    .bind(privilege.to_string())
                    .bind(access_grant.on_point.query_root().to_string())
                    .bind(access_grant.on_point.clone().to_string())
                    .bind(access_grant.to_point.clone().to_string())
                    .bind(access_grant.by_particle.to_string()).execute(&mut *conn).await?;
            }
            AccessGrantKind::PermissionsMask(mask) => {
                sqlx::query("INSERT INTO access_grants (kind,data,query_root,on_point,to_point,by_particle) VALUES ('perm',$1,$2,$3,$4,(SELECT id FROM particles WHERE point=$5))")
                    .bind(mask.to_string())
                    .bind(access_grant.on_point.query_root().to_string())
                    .bind(access_grant.on_point.clone().to_string())
                    .bind(access_grant.to_point.clone().to_string())
                    .bind(access_grant.by_particle.to_string()).execute(&mut *conn).await?;
            }
        }

        Ok(())
    }

    //    #[async_recursion]
    async fn access<'a>(&'a self, to: &'a Point, on: &'a Point) -> Result<Access, RegErr> {
        let mut conn = self.handle.acquire().await?;

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
        .fetch_one(&mut *conn)
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

        let to_kind_path: PointHierarchy =
            self.query(&to, &Query::PointHierarchy).await?.try_into()?;
        let on_kind_path: PointHierarchy =
            self.query(&on, &Query::PointHierarchy).await?.try_into()?;

        let mut traversal = on.clone();
        let mut privileges = Privileges::none();
        let mut permissions = Permissions::none();
        let mut level_ands: Vec<Vec<PermissionsMask>> = vec![];
        loop {
            let mut access_grants = sqlx::query_as::<Postgres, WrappedIndexedAccessGrant>("SELECT access_grants.*,particles.point as by_particle FROM access_grants,particles WHERE access_grants.query_root=$1 AND particles.id=access_grants.by_particle").bind(traversal.to_string()).fetch_all(&mut *conn).await?;
            let mut access_grants: Vec<AccessGrant> = access_grants
                .into_iter()
                .map(|a| a.into())
                .map(|a: IndexedAccessGrant| a.into())
                .collect();
            access_grants.retain(|access_grant| {
                access_grant.to_point.matches_found(&to_kind_path)
                    && access_grant.on_point.matches_found(&on_kind_path)
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

    async fn chown<'a>(
        &'a self,
        on: &'a Selector,
        owner: &'a Point,
        by: &'a Point,
    ) -> Result<(), RegErr> {
        let mut select = Select {
            pattern: on.clone(),
            properties: Default::default(),
            into_substance: SelectIntoSubstance::Points,
            kind: SelectKind::Initial,
        };

        let selection = self.select(&mut select).await?;
        let mut conn = self.handle.acquire().await?;
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
                .execute(&mut *trans)
                .await?;
        }
        trans.commit().await?;
        Ok(())
    }

    async fn list_access<'a>(
        &'a self,
        to: &'a Option<&'a Point>,
        on: &'a Selector,
    ) -> Result<Vec<IndexedAccessGrant>, RegErr> {
        let mut select = Select {
            pattern: on.clone(),
            properties: Default::default(),
            into_substance: SelectIntoSubstance::Points,
            kind: SelectKind::Initial,
        };

        let to: Option<PointHierarchy> = match to {
            None => None,
            //Some(to) => Some(self.query(*to, &Query::PointHierarchy).await?.try_into()?),
            Some(to) => Some(
                self.query(*to, &Query::PointHierarchy)
                    .await?
                    .try_into()
                    .expect("convert point to ..."),
            ),
        };

        let selection = self.select(&mut select).await?;
        let mut all_access_grants = HashMap::new();
        let mut conn = self.handle.acquire().await?;
        for on in selection.list {
            let on: Point = (*on).try_into()?;
            let access_grants = sqlx::query_as::<Postgres, WrappedIndexedAccessGrant>("SELECT access_grants.*,particles.point as by_particle FROM access_grants,particles WHERE access_grants.query_root=$1 AND particles.id=access_grants.by_particle").bind(on.to_string()).fetch_all(&mut *conn).await?;
            let mut access_grants: Vec<IndexedAccessGrant> =
                access_grants.into_iter().map(|a| a.into()).collect();

            access_grants.retain(|a| match to.as_ref() {
                None => true,
                Some(to) => a.to_point.matches_found(to),
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

    async fn remove_access<'a>(&'a self, id: i32, to: &'a Point) -> Result<(), RegErr> {
        let mut conn = self.handle.acquire().await?;
        let access_grant: IndexedAccessGrant = sqlx::query_as::<Postgres, WrappedIndexedAccessGrant>("SELECT access_grants.*,particles.point as by_particle FROM access_grants,particles WHERE access_grants.id=$1 AND particles.id=access_grants.by_particle").bind(id).fetch_one(&mut *conn).await?.into();
        let access = self.access(to, &access_grant.by_particle).await?;
        if access.has_full() {
            let mut trans = conn.begin().await?;
            sqlx::query("DELETE FROM access_grants WHERE id=$1")
                .bind(id)
                .execute(&mut *trans)
                .await?;
            trans.commit().await?;
            Ok(())
        } else {
            Err(RegErr::Msg(format!("'{}' could not revoked grant {} because it does not have full access (super or owner) on {}", to.to_string(), id, access_grant.by_particle.to_string()).to_string()))
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

pub struct WrappedIndexedAccessGrant {
    grant: IndexedAccessGrant,
}

impl Unpin for WrappedIndexedAccessGrant {}

impl Into<IndexedAccessGrant> for WrappedIndexedAccessGrant {
    fn into(self) -> IndexedAccessGrant {
        self.grant
    }
}

impl sqlx::FromRow<'_, PgRow> for WrappedIndexedAccessGrant {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        fn wrap(row: &PgRow) -> Result<IndexedAccessGrant, RegErr> {
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
                    panic!("don't know how to handle access grant kind {}", what)
                }
            };

            let on_point: &str = row.get("on_point");
            let to_point: &str = row.get("to_point");
            let by_particle: &str = row.get("by_particle");

            let access_grant = AccessGrant {
                kind,
                on_point: Selector::from_str(on_point)?,
                to_point: Selector::from_str(to_point)?,
                by_particle: Point::from_str(by_particle)?,
            };
            Ok(IndexedAccessGrant { id, access_grant })
        }

        match wrap(row) {
            Ok(record) => Ok(WrappedIndexedAccessGrant { grant: record }),
            Err(err) => Err(sqlx::error::Error::PoolClosed),
        }
    }
}

struct PostgresParticleRecord {
    pub details: Details,
    pub location: ParticleLocation,
}

impl Unpin for PostgresParticleRecord {}

impl Into<ParticleRecord> for PostgresParticleRecord {
    fn into(self) -> ParticleRecord {
        ParticleRecord {
            details: self.details,
            location: self.location,
        }
    }
}

impl sqlx::FromRow<'_, PgRow> for PostgresParticleRecord {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        fn wrap(row: &PgRow) -> Result<PostgresParticleRecord, RegErr> {
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
            let star: Option<String> = row.get("star");
            let host: Option<String> = row.get("host");
            let status: String = row.get("status");

            let point = Point::from_str(parent.as_str())?;
            let point = point.push(point_segment)?;
            let base = parse_errs(BaseKind::from_str(base.as_str()))?;

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

            let star = match star {
                None => None,
                Some(p) => Some(Point::from_str(p.as_str())?),
            };

            let host = match host {
                None => None,
                Some(p) => Some(Point::from_str(p.as_str())?),
            };

            let location = ParticleLocation { star, host };

            let status = parse_errs(Status::from_str(status.as_str()))?;

            let stub = Stub {
                point,
                kind,
                status,
            };

            let details = Details {
                stub,
                properties: Default::default(), // not implemented yet...
            };

            let record = PostgresParticleRecord { details, location };

            Ok(record)
        }

        match wrap(row) {
            Ok(rtn) => Ok(rtn),
            Err(err) => Err(sqlx::Error::RowNotFound),
        }
    }
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
    pub fn from_registration(registration: &Registration) -> Result<Self, RegErr> {
        let point_segment = match registration.point.segments.last() {
            None => "".to_string(),
            Some(segment) => segment.to_string(),
        };
        let parent = match registration.point.parent() {
            None => "".to_string(),
            Some(parent) => parent.to_string(),
        };
        let specific_str = match registration.kind.sub().specific() {
            None => "None".to_string(),
            Some(specific) => specific.to_string(),
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

impl PostgresRegistry {
    pub async fn set(&self, set: &Set) -> Result<(), RegErr> {
        self.set_properties(&set.point, &set.properties).await
    }

    pub async fn get(&self, get: &Get) -> Result<Substance, RegErr> {
        match &get.op {
            GetOp::State => {
                return Err(RegErr::NoGetOpStateOperations);
                /*
                let mut proto = ProtoStarMessage::new();
                proto.to(ProtoStarMessageTo::Resource(get.point.clone()));
                proto.payload = StarMessageSubstance::ResourceHost(ResourceHostAction::GetState(get.point.clone()));
                if let Ok(Reply::Substance(payload)) = self.skel.messaging_api
                    .star_exchange(proto, ReplyKind::Substance, "get state from driver")
                    .await {
                    Ok(payload)
                } else {
                    Err("could not get state".into())
                }
                 */
            }
            GetOp::Properties(keys) => {
                let properties = self.get_properties(&get.point).await?;
                let mut map = SubstanceMap::new();
                for (index, property) in properties.iter().enumerate() {
                    map.insert(
                        property.0.clone(),
                        Substance::Text(property.1.value.clone()),
                    );
                }

                Ok(Substance::Map(map))
            }
        }
    }

    pub async fn cmd_select<'a>(&'a self, select: &'a mut Select) -> Result<Substance, RegErr> {
        let list = Substance::List(self.select(select).await?);
        Ok(list)
    }

    pub async fn cmd_query<'a>(
        &'a self,
        to: &'a Point,
        query: &'a Query,
    ) -> Result<Substance, RegErr> {
        let result = Substance::Text(self.query(to, query).await?.to_string());
        Ok(result)
    }
}

#[cfg(all(test, feature = "postgres-tests"))]
pub mod test {
    use std::collections::HashSet;
    use std::convert::TryInto;
    use std::str::FromStr;
    use std::sync::Arc;

    use starlane::driver::DriversBuilder;
    use starlane::hyperlane::{AnonHyperAuthenticator, LocalHyperwayGateJumper};
    use starlane::machine::MachineTemplate;
    use starlane::platform::Platform;
    use starlane::reg::{Registration, Registry};
    use starlane::registry::err::RegErr;
    use starlane_platform_postgres_registry::{
        PostgresConnectInfo, PostgresPlatform, PostgresRegistry, PostgresRegistryContext,
        PostgresRegistryContextHandle,
    };
    use starlane_space::artifact::asynch::Artifacts;
    use starlane_space::command::direct::create::Strategy;
    use starlane_space::command::direct::query::Query;
    use starlane_space::command::direct::select::{Select, SelectIntoSubstance, SelectKind};
    use starlane_space::kind::{Kind, Specific, StarSub, UserBaseSubKind};
    use starlane_space::loc::{MachineName, StarKey, ToPoint};
    use starlane_space::particle::property::PropertiesConfig;
    use starlane_space::particle::Status;
    use starlane_space::point::Point;
    use starlane_space::security::{AccessGrant, AccessGrantKind, PermissionsMask, Privilege};
    use starlane_space::selector::{PointHierarchy, Selector};
    use starlane_space::HYPERUSER;

    #[derive(Clone)]
    pub struct TestPlatform {
        pub handle: PostgresRegistryContextHandle,
    }

    impl TestPlatform {
        pub async fn new() -> Result<Self, RegErr> {
            let postgres = Box::new(StarlanePostgres::new());
            let db = postgres.lookup_registry_db()?;
            let mut set = HashSet::new();
            set.insert(db.clone());
            let ctx = Arc::new(PostgresRegistryContext::new(set, postgres).await?);
            let handle = PostgresRegistryContextHandle::new(&db, ctx);
            Ok(Self { handle })
        }
    }

    /*
    impl PostgresPlatform for TestPlatform {
        fn lookup_registry_db(&self) -> Result<PostgresDbInfo, <Self as Platform>::Err> {
            Ok(PostgresDbInfo::new(
                "localhost",
                "postgres",
                "password",
                "postgres",
            ))
        }

        fn lookup_star_db(&self, star: &StarKey) -> Result<PostgresDbInfo, <Self as Platform>::Err> {
            todo!()
        }
    }
     */

    #[async_trait]
    impl Platform for TestPlatform {
        type Err = RegErr;
        type RegistryContext = PostgresRegistryContextHandle;
        type StarAuth = AnonHyperAuthenticator;
        type RemoteStarConnectionFactory = LocalHyperwayGateJumper;

        async fn global_registry(&self) -> Result<Registry, Self::Err> {
            let logger = RootLogger::default();
            let logger = logger.point(Point::global_registry());
            Ok(Arc::new(
                PostgresRegistry::new(self.handle.clone(), logger).await?,
            ))
        }

        fn star_auth(&self, star: &StarKey) -> Result<Self::StarAuth, Self::Err> {
            todo!()
        }

        fn remote_connection_factory_for_star(
            &self,
            star: &StarKey,
        ) -> Result<Self::RemoteStarConnectionFactory, Self::Err> {
            todo!()
        }

        fn machine_template(&self) -> MachineTemplate {
            todo!()
        }

        fn machine_name(&self) -> MachineName {
            todo!()
        }

        fn properties_config(&self, kind: &Kind) -> PropertiesConfig {
            todo!()
        }

        fn drivers_builder(&self, kind: &StarSub) -> DriversBuilder<Self> {
            todo!()
        }

        async fn star_registry(&self, star: &StarKey) -> Result<Registry<Self>, Self::Err> {
            todo!()
        }

        fn artifact_hub(&self) -> Artifacts {
            todo!()
        }
    }

    pub async fn registry() -> Result<Registry<TestPlatform>, RegErr> {
        TestPlatform::new().await?.global_registry().await
    }

    #[test]
    pub fn test_compile_postgres() {}

    #[tokio::test]
    pub async fn test_nuke() -> Result<(), RegErr> {
        let registry = registry().await?;
        registry.scorch().await?;
        Ok(())
    }

    #[tokio::test]
    pub async fn test_create() -> Result<(), RegErr> {
        let registry = registry().await?;
        registry.scorch().await?;

        let point = Point::from_str("localhost")?;
        let hyperuser = (*HYPERUSER).clone();
        let registration = Registration {
            point: point.clone(),
            kind: Kind::Space,
            registry: Default::default(),
            properties: Default::default(),
            owner: hyperuser.clone(),
            strategy: Strategy::Commit,
            status: Status::Unknown,
        };
        println!("pre register...");
        registry.register(&registration).await?;
        println!("post registration!");

        let point = Point::from_str("localhost:mech-old")?;
        let registration = Registration {
            point: point.clone(),
            kind: Kind::Mechtron,
            registry: Default::default(),
            properties: Default::default(),
            owner: hyperuser,
            strategy: Strategy::Commit,
            status: Status::Unknown,
        };
        registry.register(&registration).await?;
        println!("second registration...");
        registry
            .assign_star(&point, &StarKey::central().to_point())
            .await?;
        println!("assignment...");
        registry.set_status(&point, &Status::Ready).await?;
        registry.sequence(&point).await?;
        let record = registry.record(&point).await?;

        let result = registry.query(&point, &Query::PointHierarchy).await?;
        let kind_path: PointHierarchy = result.try_into()?;

        println!("selecting......");
        let pattern = Selector::from_str("**")?;
        let mut select = Select {
            pattern,
            properties: Default::default(),
            into_substance: SelectIntoSubstance::Points,
            kind: SelectKind::Initial,
        };
        println!("doing select...");
        let points = registry.select(&mut select).await?;
        println!("select success");

        assert_eq!(points.len(), 2);

        Ok(())
    }

    #[tokio::test]
    pub async fn test_access() -> Result<(), RegErr> {
        let registry = registry().await?;
        registry.scorch().await?;

        let hyperuser = (*HYPERUSER).clone();
        let superuser = Point::from_str("localhost:users:superuser").unwrap();
        let scott = Point::from_str("localhost:app:users:scott").unwrap();
        let app = Point::from_str("localhost:app").unwrap();
        let mechtron = Point::from_str("localhost:app:mech-old").unwrap();
        let localhost = Point::from_str("localhost").unwrap();

        let registration = Registration {
            point: Point::root(),
            kind: Kind::Root,
            registry: Default::default(),
            properties: Default::default(),
            owner: hyperuser.clone(),
            strategy: Strategy::Commit,
            status: Status::Unknown,
        };
        registry.register(&registration).await?;

        let registration = Registration {
            point: Point::from_str("hyper")?,
            kind: Kind::Space,
            registry: Default::default(),
            properties: Default::default(),
            owner: hyperuser.clone(),
            strategy: Strategy::Commit,
            status: Status::Unknown,
        };
        registry.register(&registration).await?;

        let userbase = Kind::UserBase(UserBaseSubKind::OAuth(Specific::from_str(
            "mechtronhost.io:keycloak.com:keycloak:community:11.0.0",
        )?));

        let registration = Registration {
            point: Point::from_str("hyperspace:users")?,
            kind: userbase.clone(),
            registry: Default::default(),
            properties: Default::default(),
            owner: hyperuser.clone(),
            strategy: Strategy::Commit,
            status: Status::Unknown,
        };
        registry.register(&registration).await?;

        let registration = Registration {
            point: hyperuser.clone(),
            kind: Kind::Space,
            registry: Default::default(),
            properties: Default::default(),
            owner: hyperuser.clone(),
            strategy: Strategy::Commit,
            status: Status::Unknown,
        };
        registry.register(&registration).await?;

        let registration = Registration {
            point: localhost.clone(),
            kind: Kind::Space,
            registry: Default::default(),
            properties: Default::default(),
            owner: hyperuser.clone(),
            strategy: Strategy::Commit,
            status: Status::Unknown,
        };
        registry.register(&registration).await?;

        let point = Point::from_str("localhost:users")?;
        let registration = Registration {
            point: point.clone(),
            kind: userbase.clone(),
            registry: Default::default(),
            properties: Default::default(),
            owner: hyperuser.clone(),
            strategy: Strategy::Commit,
            status: Status::Unknown,
        };
        registry.register(&registration).await?;

        let registration = Registration {
            point: superuser.clone(),
            kind: userbase.clone(),
            registry: Default::default(),
            properties: Default::default(),
            owner: hyperuser.clone(),
            strategy: Strategy::Commit,
            status: Status::Unknown,
        };
        registry.register(&registration).await?;

        let registration = Registration {
            point: app.clone(),
            kind: Kind::App,
            registry: Default::default(),
            properties: Default::default(),
            owner: superuser.clone(),
            strategy: Strategy::Commit,
            status: Status::Unknown,
        };
        registry.register(&registration).await?;

        let app_userbase = Point::from_str("localhost:app:users")?;
        let registration = Registration {
            point: app_userbase.clone(),
            kind: userbase.clone(),
            registry: Default::default(),
            properties: Default::default(),
            owner: app.clone(),
            strategy: Strategy::Commit,
            status: Status::Unknown,
        };
        registry.register(&registration).await?;

        let registration = Registration {
            point: scott.clone(),
            kind: Kind::User,
            registry: Default::default(),
            properties: Default::default(),
            owner: app.clone(),
            strategy: Strategy::Commit,
            status: Status::Unknown,
        };
        registry.register(&registration).await?;

        let registration = Registration {
            point: mechtron.clone(),
            kind: Kind::Mechtron,
            registry: Default::default(),
            properties: Default::default(),
            owner: app.clone(),
            strategy: Strategy::Commit,
            status: Status::Unknown,
        };
        registry.register(&registration).await?;

        let grant = AccessGrant {
            kind: AccessGrantKind::Super,
            on_point: Selector::from_str("localhost+:**")?,
            to_point: superuser
                .clone()
                .try_into()
                .map_err(|e| RegErr::msg("infallible"))?,
            by_particle: hyperuser.clone(),
        };
        println!("granting...");
        registry.grant(&grant).await?;
        println!("granted...");

        let grant = AccessGrant {
            kind: AccessGrantKind::PermissionsMask(PermissionsMask::from_str("+csd-Rwx")?),
            on_point: Selector::from_str("localhost:app+:**")?,
            to_point: Selector::from_str("localhost:app:users:**<User>")?,
            by_particle: app.clone(),
        };
        registry.grant(&grant).await?;

        let grant = AccessGrant {
            kind: AccessGrantKind::PermissionsMask(PermissionsMask::from_str("+csd-rwX")?),
            on_point: Selector::from_str("localhost:app:**<Mechtron>")?,
            to_point: Selector::from_str("localhost:app:users:**<User>")?,
            by_particle: app.clone(),
        };
        registry.grant(&grant).await?;

        let grant = AccessGrant {
            kind: AccessGrantKind::PermissionsMask(PermissionsMask::from_str("+CSD-RWX")?),
            on_point: Selector::from_str("localhost:users:superuser")?,
            to_point: scott.clone().try_into().map_err(|err| RegErr::new(e))?,
            by_particle: app.clone(),
        };
        registry.grant(&grant).await?;

        let grant = AccessGrant {
            kind: AccessGrantKind::Privilege(Privilege::Single("property:email:read".to_string())),
            on_point: Selector::from_str("localhost:app:users:**<User>")?,
            to_point: Selector::from_str("localhost:app:**<Mechtron>")?,
            by_particle: app.clone(),
        };
        registry.grant(&grant).await?;

        println!("access?");
        let access = registry.access(&hyperuser, &superuser).await?;
        println!("access...");
        println!("super?");
        assert_eq!(access.has_super(), true);
        println!("super!");

        println!("get superuser record...");
        let record = registry.record(&superuser).await?;
        println!("got superuser record...");

        let access = registry.access(&superuser, &localhost).await?;
        assert_eq!(access.has_super(), true);
        println!("one");
        let access = registry.access(&superuser, &app).await?;
        assert_eq!(access.has_super(), true);

        println!("two");
        let access = registry.access(&app, &scott).await?;
        assert_eq!(access.has_super(), false);
        println!("owner?");
        assert_eq!(access.has_owner(), true);
        println!("owner.");
        assert_eq!(access.has_full(), true);

        let access = registry.access(&scott, &superuser).await?;
        assert_eq!(access.has_super(), false);
        assert_eq!(access.has_full(), false);
        assert_eq!(access.permissions().to_string(), "csd-rwx".to_string());

        // should fail because app is not the owner of localhost:app yet...
        let access = registry.access(&scott, &app).await?;
        assert_eq!(access.has_super(), false);
        assert_eq!(access.permissions().to_string(), "csd-rwx".to_string());
        println!("chown?");
        // must have super to chagne ownership
        let app_pattern = Selector::from_str("localhost:app+:**")?;
        assert!(registry.chown(&app_pattern, &app, &scott).await.is_err());
        // this should work:
        assert!(registry.chown(&app_pattern, &app, &superuser).await.is_ok());
        println!("chown...");

        // now the previous rule should work since app now owns itself.
        let access = registry.access(&scott, &app).await?;
        assert_eq!(access.has_super(), false);
        assert_eq!(access.permissions().to_string(), "csd-Rwx".to_string());

        let access = registry.access(&scott, &superuser).await?;
        assert_eq!(access.has_super(), false);
        assert_eq!(access.permissions().to_string(), "csd-rwx".to_string());

        // mem masked OR permissions
        let access = registry.access(&scott, &mechtron).await?;
        assert_eq!(access.has_super(), false);
        assert_eq!(access.permissions().to_string(), "csd-RwX".to_string());
        println!("got here...");
        // now mem AND permissions (masking Read)
        let grant = AccessGrant {
            kind: AccessGrantKind::PermissionsMask(PermissionsMask::from_str("&csd-rwX")?),
            on_point: Selector::from_str("localhost:app:**<Mechtron>")?,
            to_point: Selector::from_str("localhost:app:users:**<User>")?,
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
        println!("and here...");
        //let selector = Selector::from_str("+:**")?;
        let selector = Selector::from_str("**")?;
        println!("selector created...");
        let access_grants = registry.list_access(&None, &selector).await?;
        println!("lising access grants...");
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
