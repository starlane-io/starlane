use std::collections::{HashMap, HashSet};
use std::convert::{TryFrom, TryInto};
use std::fmt;
use std::fmt::{Debug, Formatter};
use std::fs::DirBuilder;
use std::hash::Hash;
use std::iter::FromIterator;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use rusqlite::types::{ToSqlOutput, Value, ValueRef};
use rusqlite::{params, params_from_iter, Connection, Row, ToSql, Transaction};
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot::Receiver;
use tokio::sync::{mpsc, oneshot};

use crate::error::Error;
use crate::fail::Fail;
use crate::file_access::FileAccess;
use crate::logger::{elog, LogInfo, StaticLogInfo};
use crate::mesh::serde::id::Address;
use crate::mesh::serde::payload::Payload;
use crate::mesh::serde::resource::{Archetype, ResourceStub};
use crate::message::{MessageExpect, ProtoStarMessage, ReplyKind};
use crate::names::Name;

use crate::resources::message::{ProtoMessage, MessageFrom};
use crate::star::shell::pledge::{ResourceHostSelector, StarConscript};
use crate::star::{ResourceRegistryBacking, StarInfo, StarKey, StarSkel};
use crate::starlane::api::StarlaneApi;
use crate::util::AsyncHashMap;
use crate::{error, logger, util};
use mesh_portal_serde::version::latest::id::Specific;
use std::collections::hash_map::RandomState;
use tracing_futures::WithSubscriber;
use mesh_portal_serde::version::v0_0_1::generic::entity::request::ReqEntity;
use crate::mesh::serde::entity::request::Rc;
use crate::mesh::serde::payload::{RcCommand, Primitive, PayloadMap};
use crate::mesh::serde::fail;
use crate::mesh::serde::resource::command::create::{Create, Strategy};
use crate::mesh::serde::resource::command::create::AddressSegmentTemplate;
use crate::mesh::serde::resource::command::update::Update;
use crate::frame::{ResourceHostAction, StarMessagePayload};
use crate::mesh::serde::resource::command::common::{SetRegistry, SetProperties};
use crate::mesh::serde::resource::command::select::Select;
use crate::mesh::serde::pattern::AddressKindPattern;
use mesh_portal_serde::version::v0_0_1::pattern::SegmentPattern;

pub mod artifact;
pub mod config;
pub mod file;
pub mod file_system;
pub mod user;

//static RESOURCE_QUERY_FIELDS: &str = "r.key,r.address,r.kind,r.specific,r.owner,r.config,r.host,r.gathering";
static RESOURCE_QUERY_FIELDS: &str = "r.key,r.address,r.kind,r.specific,r.owner,r.config,r.host";

impl ToSql for Name {
    fn to_sql(&self) -> Result<ToSqlOutput<'_>, rusqlite::Error> {
        Ok(ToSqlOutput::Owned(Value::Text(self.to())))
    }
}

pub struct ResourceRegistryAction {
    pub tx: oneshot::Sender<ResourceRegistryResult>,
    pub command: RegistryCall,
}

impl ResourceRegistryAction {
    pub fn new(
        command: RegistryCall,
    ) -> (Self, oneshot::Receiver<ResourceRegistryResult>) {
        let (tx, rx) = oneshot::channel();
        (
            ResourceRegistryAction {
                tx: tx,
                command: command,
            },
            rx,
        )
    }
}

pub enum RegistryCall {
    Close,
    Clear,
    //Accepts(HashSet<ResourceType>),
    Register{registration:Registration, tx: oneshot::Sender<Result<(),Fail>>},
    Select{ select: Select, tx: oneshot::Sender<Result<Vec<ResourceStub>,Fail>>},
    SetLocation(ResourceRecord),
    Locate(Address),
    Update(ResourceRegistryPropertyAssignment),
}

pub enum ResourceRegistryResult {
    Ok,
    Error(String),
    Resource(Option<ResourceRecord>),
    Resources(Vec<ResourceRecord>),
    Address(Address),
    Reservation(RegistryReservation),
    Key(Address),
    Unique(u64),
    NotFound,
    NotAccepted,
}

impl ToString for ResourceRegistryResult {
    fn to_string(&self) -> String {
        match self {
            ResourceRegistryResult::Ok => "Ok".to_string(),
            ResourceRegistryResult::Error(err) => format!("Error({})", err),
            ResourceRegistryResult::Resource(_) => "Resource".to_string(),
            ResourceRegistryResult::Resources(_) => "Resources".to_string(),
            ResourceRegistryResult::Address(_) => "Address".to_string(),
            ResourceRegistryResult::Reservation(_) => "Reservation".to_string(),
            ResourceRegistryResult::Key(_) => "Key".to_string(),
            ResourceRegistryResult::Unique(_) => "Unique".to_string(),
            ResourceRegistryResult::NotFound => "NotFound".to_string(),
            ResourceRegistryResult::NotAccepted => "NotAccepted".to_string(),
        }
    }
}

type Blob = Vec<u8>;

struct RegistryParams {
    address: String,
    resource_type: String,
    kind: Option<String>,
    vendor: Option<String>,
    product: Option<String>,
    variant: Option<String>,
    version: Option<String>,
    version_pre: Option<String>,
    parent: String,
}

impl RegistryParams {
    pub fn from_registration(registration: &Registration ) -> Result<Self, Error> {

        let address = registration.address.to_string();
        let parent = registration.parent.to_string();
        let resource_type = registration.kind.resource_type().to_string();
        let kind = registration.kind.sub_string();

        let vendor = match &registration.kind.specific() {
            None => Option::None,
            Some(specific) => Option::Some(specific.vendor.clone()),
        };

        let product= match &registration.kind.specific() {
            None => Option::None,
            Some(specific) => Option::Some(specific.product.clone()),
        };

        let variant = match &registration.kind.specific() {
            None => Option::None,
            Some(specific) => Option::Some(specific.variant.clone()),
        };

        let version = match &registration.kind.specific() {
            None => Option::None,
            Some(specific) =>  {
                let version = &specific.version;
                Option::Some(format!( "{}.{}.{}", version.major, version.minor, version.patch ))
            }
        };

        let version_pre = match &registration.kind.specific() {
            None => Option::None,
            Some(specific) =>  {
                let version = &specific.version;
                if version.is_prerelease() {
                    let mut pre = String::new();
                    for (i, x) in version.pre.iter().enumerate() {
                        if i != 0 {
                            result.push_str(".");
                        }
                        result.push_str(format!("{}", x).as_ref());
                    }
                    Option::Some(pre)
                } else {
                    Option::None
                }
            }
        };

        Ok(RegistryParams {
            address,
            parent,
            resource_type,
            kind,
            vendor,
            product,
            variant,
            version,
            version_pre
        })
    }
}

/*
pub struct Registry {
    pub conn: Connection,
    pub tx: mpsc::Sender<ResourceRegistryAction>,
    pub rx: mpsc::Receiver<ResourceRegistryAction>,
    star_info: StarInfo,
}

impl Registry {
    pub async fn new(star_info: StarInfo, path: String) -> mpsc::Sender<ResourceRegistryAction> {
        let (tx, rx) = mpsc::channel(8 * 1024);
        let tx_clone = tx.clone();

        // ensure that path directory exists
        let mut dir_builder = DirBuilder::new();
        dir_builder.recursive(true);
        if let Result::Err(_) = dir_builder.create(path.clone()) {
            eprintln!("FATAL: could not create star data directory: {}", path);
            return tx;
        }
        tokio::spawn(async move {
            //let conn = Connection::open(format!("{}/resource_registry.sqlite",path));
            let conn = Connection::open_in_memory();
            if conn.is_ok() {
                let mut db = Registry {
                    conn: conn.unwrap(),
                    tx: tx_clone,
                    rx: rx,
                    star_info: star_info,
                };
                db.run().await.unwrap();
            } else {
                let log_info = StaticLogInfo::new(
                    "ResourceRegistry".to_string(),
                    star_info.log_kind().to_string(),
                    star_info.key.to_string(),
                );
                eprintln!("connection ERROR!");
                logger::elog(
                    &log_info,
                    &star_info,
                    "new()",
                    format!(
                        "ERROR: could not create SqLite connection to database: '{}'",
                        conn.err().unwrap().to_string(),
                    )
                    .as_str(),
                );
            }
        });
        tx
    }

    async fn run(&mut self) -> Result<(), Error> {
        match self.setup() {
            Ok(_) => {}
            Err(err) => {
                eprintln!("error setting up db: {}", err);
                return Err(err);
            }
        };

        while let Option::Some(request) = self.rx.recv().await {
            if let RegistryCall::Close = request.command {
                break;
            }
            match self.process(request.command) {
                Ok(ok) => {
                    request.tx.send(ok);
                }
                Err(err) => {
                    eprintln!("{}", err);
                    request
                        .tx
                        .send(ResourceRegistryResult::Error(err.to_string()));
                }
            }
        }

        Ok(())
    }

    fn process(
        &mut self,
        command: RegistryCall,
    ) -> Result<ResourceRegistryResult, Error> {
        match command {
            RegistryCall::Close => Ok(ResourceRegistryResult::Ok),
            RegistryCall::Clear => {
                let trans = self.conn.transaction()?;
                trans.execute("DELETE FROM labels", [])?;
                trans.execute("DELETE FROM tags", [])?;
                trans.execute("DELETE FROM properties", [])?;
                trans.execute("DELETE FROM resources", [])?;
                trans.commit()?;

                Ok(ResourceRegistryResult::Ok)
            }

            RegistryCall::Register{ registration, tx } => {

                fn register( registration: Registration, trans: Transaction ) -> Result<(),Fail> {
                    let params = RegistryParams::from_registration(&registration)?;
                    trans.execute("INSERT INTO resources (address,resource_type,kind,vendor,product,variant,version,version_pre,parent) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)", params![params.address,params.resource_type,params.kind,params.vendor,params.product,params.variant,params.version,params.version_pre,params.parent])?;

                    fn properties( prefix: &str, properties: SetProperties, trans: &Transaction ) -> Result<(),Fail> {
                        for (key, payload) in properties.iter() {
                            match payload {
                                Payload::Empty => {}
                                Payload::Primitive(primitive) => {
                                    match primitive {
                                        Primitive::Text(text) => {
                                            trans.execute("INSERT INTO properties (parent,address_segment,key,value) VALUES (?1,?2,?3)", params![params.parent,parzms.address_segment,key.to_string(),text.to_string()])?;
                                        }
                                        Primitive::Address(address) => {
                                            trans.execute("INSERT INTO properties (parent,address_segment,address,key,value) VALUES (?1,?2,?3)", params![params.parent,params.address_segment,key.to_string(),address.to_string()])?;
                                        }
                                        found => {
                                            return Err(Fail::Fail(fail::Fail::Resource(fail::resource::Fail::Create(fail::resource::Create::InvalidProperty { expected: "Text|Address|PayloadMap".to_string(), found: found.primitive_type().to_string() }))));
                                        }
                                    }
                                }
                                Payload::Map(map) => {
                                    let prefix = if prefix.len() == 0 {
                                        key.clone()
                                    } else {
                                        format!("{}.{}",prefix,key)
                                    };
                                    properties( prefix.as_str(), map, &trans)?;
                                }
                                found => {
                                    return Err(Fail::Fail(fail::Fail::Resource(fail::resource::Fail::Create(fail::resource::Create::InvalidProperty { expected: "Text|Address|PayloadMap".to_string(), found: found.payload_type().to_string() }))));
                                }
                            }
                        }
                        Ok(())
                    }

                    properties( "", registration.properties, &trans )?;

                    trans.commit()?;
                    Ok(())
                }
                let trans = self.conn.transaction()?;
                match register( registration, trans ) {
                    Ok(_) => {
                        tx.send(Ok(()));
                    }
                    Err(err) => {
                        tx.send(Err(Fail::Starlane(StarlaneFailure::Error(err.to_string()))));
                        return Err(err.into())
                    }
                }

                Ok(ResourceRegistryResult::Ok)
            }
            RegistryCall::Select{select,tx} => {


                fn address_pattern( address: AddressKindPattern ) -> String {
                    let mut rtn = String::new();
                    for (index, hop) in address.hops.iter().enumerate() {
                       match hop.segment {
                           SegmentPattern::Any => {
                               rtn.push_str("%");
                               if index != address.hops.len() {
                                   rtn.push_str(":");
                               }
                           }
                           SegmentPattern::Recursive => {}
                           SegmentPattern::Exact(_) => {}
                       }
                    }
                    rtn
                }



                let mut params: Vec<FieldSelectionSql> = vec![];
                let mut where_clause = String::new();

                for (index, field) in Vec::from_iter(selector.fields.clone())
                    .iter()
                    .map(|x| x.clone())
                    .enumerate()
                {
                    if index != 0 {
                        where_clause.push_str(" AND ");
                    }

                    let f = match field {
                        FieldSelection::Type(_) => {
                            format!("r.resource_type=?{}", index + 1)
                        }
                        FieldSelection::Kind(_) => {
                            format!("r.kind=?{}", index + 1)
                        }
                        FieldSelection::Specific(_) => {
                            format!("r.specific=?{}", index + 1)
                        }
                        FieldSelection::Parent(_) => {
                            format!("r.parent=?{}", index + 1)
                        }
                    };
                    where_clause.push_str(f.as_str());
                    params.push(field.into());
                }

                /*
                if !params.is_empty() {
                    where_clause.push_str(" AND ");
                }

                where_clause.push_str(" key IS NOT NULL");

                 */

                let mut statement = match &selector.meta {
                    MetaSelector::None => {
                        format!(
                            "SELECT DISTINCT {} FROM resources as r WHERE {}",
                            RESOURCE_QUERY_FIELDS, where_clause
                        )
                    }
                    MetaSelector::Label(label_selector) => {
                        let mut labels = String::new();
                        for (_index, label_selection) in
                            Vec::from_iter(label_selector.labels.clone())
                                .iter()
                                .map(|x| x.clone())
                                .enumerate()
                        {
                            if let LabelSelection::Exact(label) = label_selection {
                                labels.push_str(format!(" AND {} IN (SELECT labels.resource_key FROM labels WHERE labels.name='{}' AND labels.value='{}')", RESOURCE_QUERY_FIELDS, label.name, label.value).as_str())
                            }
                        }

                        format!(
                            "SELECT DISTINCT {} FROM resources as r WHERE {} {}",
                            RESOURCE_QUERY_FIELDS, where_clause, labels
                        )
                    }
                    MetaSelector::Name(name) => {
                        if where_clause.is_empty() {
                            format!(
                                "SELECT DISTINCT {} FROM names as r WHERE r.name='{}'",
                                RESOURCE_QUERY_FIELDS, name
                            )
                        } else {
                            format!(
                                "SELECT DISTINCT {} FROM names as r WHERE {} AND r.name='{}'",
                                RESOURCE_QUERY_FIELDS, where_clause, name
                            )
                        }
                    }
                };

                // in case this search was for EVERYTHING
                if selector.is_empty() {
                    statement = format!(
                        "SELECT DISTINCT {} FROM resources as r",
                        RESOURCE_QUERY_FIELDS
                    )
                    .to_string();
                }

                let mut statement = self.conn.prepare(statement.as_str())?;
                let mut rows = statement.query(params_from_iter(params.iter()))?;

                let mut resources = vec![];
                while let Option::Some(row) = rows.next()? {
                    resources.push(Self::process_resource_row_catch(row)?);
                }

                Ok(ResourceRegistryResult::Resources(resources))
            }
            RegistryCall::SetLocation(location_record) => {
                let key = location_record.stub.key.bin()?;
                let host = location_record.location.host.bin()?;
                let trans = self.conn.transaction()?;
                trans.execute(
                    "UPDATE resources SET host=?1 WHERE key=?3",
                    params![host, key],
                )?;
                trans.commit()?;
                Ok(ResourceRegistryResult::Ok)
            }
            RegistryCall::Locate(address) => {
                if address.is_root() {
                    return Ok(ResourceRegistryResult::Resource(Option::Some(
                        ResourceRecord::root(),
                    )));
                }

                let address = address.to_string();
                let statement = format!(
                    "SELECT {} FROM resources as r WHERE address=?1",
                    RESOURCE_QUERY_FIELDS
                );
                let mut statement = self.conn.prepare(statement.as_str())?;
                let result = statement.query_row(params![address], |row| {
                    let record = Self::process_resource_row_catch(row)?;
                    println!(
                        "return record: {} with config {}",
                        record.stub.address.to_string(),
                        record.stub.archetype.config.to_string()
                    );
                    Ok(record)
                });

                match result {
                    Ok(record) => Ok(ResourceRegistryResult::Resource(Option::Some(record))),
                    Err(rusqlite::Error::QueryReturnedNoRows) => {
                        Ok(ResourceRegistryResult::Resource(Option::None))
                    }
                    Err(err) => match err {
                        rusqlite::Error::QueryReturnedNoRows => {
                            Ok(ResourceRegistryResult::Resource(Option::None))
                        }
                        err => {
                            eprintln!("for {} SQL ERROR: {}", address.to_string(), err.to_string());
                            Err(err.into())
                        }
                    },
                }
            }

            RegistryCall::Reserve(request) => {
                let trans = self.conn.transaction()?;
                trans.execute("DELETE FROM names WHERE key IS NULL AND datetime(reservation_timestamp) < datetime('now')", [] )?;

                let params = RegistryParams::new(
                    request.archetype.clone(),
                    Option::Some(request.parent.clone()),
                    Option::None,
                )?;
                if request.info.is_some() {
                    let params = RegistryParams::from_archetype(
                        request.archetype.clone(),
                        Option::Some(request.parent.clone()),
                    )?;
                    Self::process_names(
                        &trans,
                        &request.info.as_ref().cloned().unwrap().names,
                        &params,
                    )?;
                }
                trans.commit()?;
                let (tx, rx) = oneshot::channel();
                let reservation = RegistryReservation::new(tx);
                let action_tx = self.tx.clone();
                let info = request.info.clone();
                tokio::spawn(async move {
                    let result = rx.await;
                    if let Result::Ok((record, result_tx)) = result {
                        let mut params = params;
                        let key = match record.stub.key.bin() {
                            Ok(key) => Option::Some(key),
                            Err(_) => Option::None,
                        };

                        params.key = key;
                        params.address = Option::Some(record.stub.address.to_string());
                        let registration = ResourceRegistration::new(record.clone(), info);
                        let (action, rx) = ResourceRegistryAction::new(
                            RegistryCall::Commit(registration),
                        );
                        action_tx.send(action).await;
                        rx.await;
                        result_tx.send(Ok(()));
                    } else if let Result::Err(error) = result {
                        error!(
                            "ERROR: reservation failed to commit due to RecvErr: '{}'",
                            error.to_string()
                        );
                    } else {
                        error!("ERROR: reservation failed to commit.");
                    }
                });
                Ok(ResourceRegistryResult::Reservation(reservation))
            }
            RegistryCall::Update(assignment) => {

                unimplemented!()
            }
        }
    }

    fn process_resource_row_catch(row: &Row) -> Result<ResourceRecord, Error> {
        match Self::process_resource_row(row) {
            Ok(ok) => Ok(ok),
            Err(error) => {
                eprintln!("process_resource_rows: {}", error);
                Err(error)
            }
        }
    }

    fn process_resource_row(row: &Row) -> Result<ResourceRecord, Error> {

        let address: String = row.get(1)?;
        let address = Address::from_str(address.as_str())?;

        let kind: String = row.get(2)?;
        let kind = Kind::from_str(kind.as_str())?;

        let specific = if let ValueRef::Null = row.get_ref(3)? {
            Option::None
        } else {
            let specific: String = row.get(3)?;
            let specific = Specific::from_str(specific.as_str())?;
            Option::Some(specific)
        };

        let config = if let ValueRef::Null = row.get_ref(5)? {
            ConfigSrc::None
        } else {
            let config: String = row.get(5)?;
            let config = ConfigSrc::from_str(config.as_str())?;
            config
        };

        let host: Vec<u8> = row.get(6)?;
        let host = StarKey::from_bin(host)?;

        let stub = ResourceStub {
            address: address,
            archetype: Archetype{
                kind: kind,
                config_src: config
            },
        };

        let record = ResourceRecord {
            stub: stub,
            location: ResourceLocation { host: host },
        };

        Ok(record)
    }

    pub fn setup(&mut self) -> Result<(), Error> {


        let resources = r#"CREATE TABLE IF NOT EXISTS resources (
         address TEXT PRIMARY KEY,
         parent TEXT NOT NULL,
         resource_type TEXT NOT NULL,
         kind TEXT NOT NULL,
         vendor TEXT,
         product TEXT,
         variant TEXT,
         version TEXT,
         version_variant TEXT,
         host TEXT
        )"#;

        let labels = r#"
       CREATE TABLE IF NOT EXISTS labels (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
	      address TEXT NOT NULL,
	      key TEXT NOT NULL,
	      value TEXT,
          UNIQUE(key,value),
          FOREIGN KEY (address) REFERENCES resources (address)
        )"#;

        /// note that a tag may reference an address NOT in this database
        /// therefore it does not have a FOREIGN KEY constraint
        let tags = r#"
       CREATE TABLE IF NOT EXISTS tags(
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          parent TEXT NOT NULL,
          tag TEXT NOT NULL,
          address TEXT NOT NULL,
          UNIQUE(tag)
        )"#;


        let properties = r#"CREATE TABLE IF NOT EXISTS properties (
         id INTEGER PRIMARY KEY AUTOINCREMENT,
         address TEXT NOT NULL,
         key TEXT NOT NULL,
         value TEXT NOT NULL,
         FOREIGN KEY (address) REFERENCES resources (address),
         UNIQUE(address,key)
        )"#;

        let address_index = "CREATE UNIQUE INDEX resource_address_index ON resources(address)";

        let transaction = self.conn.transaction()?;
        transaction.execute(labels, [])?;
        transaction.execute(tags, [])?;
        transaction.execute(resources, [])?;
        transaction.execute(properties, [])?;
        transaction.execute(address_index, [])?;
        transaction.commit()?;

        Ok(())
    }
}


impl LogInfo for Registry {
    fn log_identifier(&self) -> String {
        self.star_info.log_identifier()
    }

    fn log_kind(&self) -> String {
        self.star_info.log_kind()
    }

    fn log_object(&self) -> String {
        "Registry".to_string()
    }
}

 */

#[async_trait]
pub trait HostedResource: Send + Sync {
    fn key(&self) -> Address;
}

#[derive(Clone)]
pub struct HostedResourceStore {
    map: AsyncHashMap<Address, Arc<LocalHostedResource>>,
}

impl HostedResourceStore {
    pub async fn new() -> Self {
        HostedResourceStore {
            map: AsyncHashMap::new(),
        }
    }

    pub async fn store(&self, resource: Arc<LocalHostedResource>) -> Result<(), Error> {
        self.map.put(resource.resource.key.clone(), resource).await
    }

    pub async fn get(&self, key: Address) -> Result<Option<Arc<LocalHostedResource>>, Error> {
        self.map.get(key).await
    }

    pub async fn remove(
        &self,
        key: Address,
    ) -> Result<Option<Arc<LocalHostedResource>>, Error> {
        self.map.remove(key).await
    }

    pub async fn contains(&self, key: &Address) -> Result<bool, Error> {
        self.map.contains(key.clone()).await
    }
}

#[derive(Clone)]
pub struct RemoteHostedResource {
    key: Address,
    star_host: StarKey,
    local_skel: StarSkel,
}

pub struct LocalHostedResource {
    //    pub manager: Arc<dyn ResourceManager>,
    pub unique_src: Box<dyn UniqueSrc>,
    pub resource: ResourceStub,
}
impl HostedResource for LocalHostedResource {
    fn key(&self) -> Address {
        self.resource.key.clone()
    }
}

#[async_trait]
pub trait ResourceManager: Send + Sync {
    async fn create(
        &self,
        create: ResourceCreate,
    ) -> oneshot::Receiver<Result<ResourceRecord, Fail>>;
}

pub struct RemoteResourceManager {
    pub key: Address,
}

impl RemoteResourceManager {
    pub fn new(key: Address) -> Self {
        RemoteResourceManager { key: key }
    }
}

#[async_trait]
impl ResourceManager for RemoteResourceManager {
    async fn create(&self, _create: ResourceCreate) -> Receiver<Result<ResourceRecord, Fail>> {
        unimplemented!();
    }
}

#[derive(Clone)]
pub struct ParentCore {
    pub skel: StarSkel,
    pub stub: ResourceStub,
    pub selector: ResourceHostSelector,
    pub child_registry: Arc<dyn ResourceRegistryBacking>,
}

impl Debug for ParentCore {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ParentCore")
            .field(&self.skel)
            .field(&self.stub)
            .finish()
    }
}

pub struct Parent {
    pub core: ParentCore,
}

impl Parent {
    #[instrument]
    async fn create_child(
        core: ParentCore,
        create: Create,
        tx: oneshot::Sender<Result<ResourceStub, Fail>>,
    ) {
        let parent = match create
            .parent
            .clone()
            .key_or("expected create.parent to already be a key")
        {
            Ok(key) => key,
            Err(error) => {
                tx.send(Err(Fail::from(error)));
                return;
            }
        };

        if let Ok(reservation) = core
            .child_registry
            .reserve(Registration {
                parent: parent,
                archetype: create.archetype.clone(),
                info: create.registry_info.clone(),
            })
            .await
        {
            let rx =
                ResourceCreationChamber::new(core.stub.clone(), create.clone(), core.skel.clone())
                    .await;

            tokio::spawn(async move {
                match Self::process_action(core.clone(), create.clone(), reservation, rx).await {
                    Ok(resource) => {
                        tx.send(Ok(resource.into()));
                    }
                    Err(fail) => {
                        error!("Failed to create child: FAIL: {}", fail.to_string());
                        tx.send(Err(fail.into()));
                    }
                }
            });
        } else {
            error!("ERROR: reservation failed.");

            tx.send(Err("RESERVATION FAILED!".into()));
        }
    }

    async fn process_action(
        core: ParentCore,
        create: Create,
        reservation: RegistryReservation,
        rx: oneshot::Receiver<Result<ResourceAction<AssignResourceStateSrc>, Fail>>,
    ) -> Result<ResourceRecord, Error> {
        let action = rx.await??;

        match action {
            ResourceAction::Assign(assign) => {
                let host = core
                    .selector
                    .select(create.archetype.kind.resource_type())
                    .await?;
                let record = ResourceRecord::new(assign.stub.clone(), host.star_key());
                /// need to make this so that reservation is already committed with status set to Pending
                /// at this exact point status is updated to Assigning
                /// if Assigning succeeds then the host may put it through an Initializing status (if this AssignKind is Create vs. Move)
                /// once Status is Ready the resource can receive & process requests
                host.assign(assign.clone().try_into()?).await?;
/*               let (commit_tx, _commit_rx) = oneshot::channel();
                reservation.commit(record.clone(), commit_tx)?;
                host.init(assign.stub.address).await?;

 */
                Ok(record)
            }
            ResourceAction::Update(resource) => {
                /*
                // save resource state...
                let mut proto = ProtoMessage::new();

                let update = Update{
                    address: resource.address.clone(),
                    properties: PayloadMap::default()
                };

                proto.entity(ReqEntity::Rc(Rc::new(RcCommand::Update(Box::new(update)), resource.state_src() )));
                proto.to(resource.address.clone());
                proto.from(MessageFrom::Address(core.stub.address.clone()));

                let reply = core
                    .skel
                    .messaging_api
                    .exchange(
                        proto.try_into()?,
                        ReplyKind::Empty,
                        "updating the state of a record ",
                    )
                    .await;
                match reply {
                    Ok(reply) => {
                        let record = core
                            .skel
                            .resource_locator_api
                            .locate(resource.address )
                            .await;
                        record
                    }
                    Err(err) => Err(err.into()),
                }


                 */
                //               reservation.cancel();
            }
            ResourceAction::None => {
                // do nothing
            }
        }
    }

    /*
    if let Ok(Ok(assign)) = rx.await {
    if let Ok(mut host) = core.selector.select(create.archetype.kind.resource_type()).await
    {
    let record = ResourceRecord::new(assign.stub.clone(), host.star_key());
    match host.assign(assign).await
    {
    Ok(_) => {}
    Err(err) => {
    eprintln!("host assign failed.");
    return;
    }
    }
    let (commit_tx, commit_rx) = oneshot::channel();
    match reservation.commit(record.clone(), commit_tx) {
    Ok(_) => {
    if let Ok(Ok(_)) = commit_rx.await {
    tx.send(Ok(record));
    } else {
    elog( &core, &record.stub, "create_child()", "commit failed" );
    tx.send(Err("commit failed".into()));
    }
    }
    Err(err) => {
    elog( &core, &record.stub, "create_child()", format!("ERROR: commit failed '{}'",err.to_string()).as_str() );
    tx.send(Err("commit failed".into()));
    }
    }
    } else {
    elog( &core, &assign.stub, "create_child()", "ERROR: could not select a host" );
    tx.send(Err("could not select a host".into()));
    }
    }

     */

    /*
    async fn process_create(core: ChildResourceManagerCore, create: ResourceCreate ) -> Result<ResourceRecord,Fail>{



        if !create.archetype.kind.resource_type().parent().matches(Option::Some(&core.key.resource_type())) {
            return Err(Fail::WrongParentResourceType {
                expected: HashSet::from_iter(core.key.resource_type().parent().types()),
                received: Option::Some(create.parent.resource_type())
            });
        };

        create.validate()?;

        let reservation = core.registry.reserve(ResourceNamesReservationRequest{
            parent: create.parent.clone(),
            archetype: create.archetype.clone(),
            info: create.registry_info } ).await?;

        let key = match create.key {
            KeyCreationSrc::None => {
                Address::new(core.key.clone(), ResourceId::new(&create.archetype.kind.resource_type(), core.id_seq.next() ) )?
            }
            KeyCreationSrc::Key(key) => {
                if key.parent() != Option::Some(core.key.clone()){
                    return Err("parent keys do not match".into());
                }
                key
            }
        };

        let address = match create.address{
            AddressCreationSrc::None => {
                let address = format!("{}:{}", core.address.to_parts_string(), key.generate_address_tail()? );
                create.archetype.kind.resource_type().address_structure().from_str(address.as_str())?
            }
            AddressCreationSrc::Append(tail) => {
                create.archetype.kind.resource_type().append_address(core.address.clone(), tail )?
            }
            AddressCreationSrc::Space(space_name) => {
                if core.key.resource_type() != ResourceType::Nothing{
                    return Err(format!("Space creation can only be used at top level (Nothing) not by {}",core.key.resource_type().to_string()).into());
                }
                ResourceAddress::for_space(space_name.as_str())?
            }
        };

        let stub = ResourceStub {
            key: key,
            address: address.clone(),
            archetype: create.archetype.clone(),
            owner: None
        };


        let assign = ResourceAssign {
            stub: stub.clone(),
            state_src: create.src.clone(),
        };

        let mut host = core.selector.select(create.archetype.kind.resource_type() ).await?;
        host.assign(assign).await?;
        let record = ResourceRecord::new( stub, host.star_key() );
        let (tx,rx) = oneshot::channel();
        reservation.commit( record.clone(), tx )?;

        Ok(record)
    }

     */
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourceAction<S> {
    None,
    Assign(ResourceAssign<S>),
    Update(Resource)
}

impl LogInfo for ParentCore {
    fn log_identifier(&self) -> String {
        self.skel.info.log_identifier()
    }

    fn log_kind(&self) -> String {
        self.skel.info.log_kind()
    }

    fn log_object(&self) -> String {
        "Parent".to_string()
    }
}

#[async_trait]
impl ResourceManager for Parent {
    async fn create(
        &self,
        create: Create,
    ) -> oneshot::Receiver<Result<ResourceStub, Fail>> {
        let (tx, rx) = oneshot::channel();

        let core = self.core.clone();
        tokio::spawn(async move {
            Parent::create_child(core, create, tx).await;
        });
        rx
    }
}





pub struct ResourceCreationChamber {
    parent: ResourceStub,
    create: Create,
    skel: StarSkel,
    tx: oneshot::Sender<Result<ResourceAction<AssignResourceStateSrc>, Fail>>,
}

impl ResourceCreationChamber {
    pub async fn new(
        parent: ResourceStub,
        create: Create,
        skel: StarSkel,
    ) -> oneshot::Receiver<Result<ResourceAction<AssignResourceStateSrc>, Fail>> {
        let (tx, rx) = oneshot::channel();
        let chamber = ResourceCreationChamber {
            parent: parent,
            create: create,
            skel: skel,
            tx: tx,
        };
        chamber.run().await;
        rx
    }

    async fn run(self) {
        tokio::spawn(async move {
           async fn create( chamber: &ResourceCreationChamber) -> Result<ResourceAction<AssignResourceStateSrc>,Fail> {

               let address = match &chamber.create.address_template.child_segment_template {
                   AddressSegmentTemplate::Exact(segment) => {
                       chamber.create.address_template.parent.push(segment.clone())?
                   }
               };

               let record = chamber
                   .skel
                   .resource_locator_api
                   .locate(address.clone().into())
                   .await;

               match record {
                   Ok(record) => {
                       match chamber.create.strategy {
                           Strategy::Create => {

                               let fail = Fail::Fail(fail::Fail::Resource(fail::resource::Fail::Create(fail::resource::Create::AddressAlreadyInUse(address.to_string()))));
                               return Err(fail);
                           }
                           Strategy::Ensure => {
                               // we've proven it's here, now we can go home
                               return Ok(ResourceAction::None);
                           }
                           Strategy::CreateOrUpdate => {
                               if record.stub.archetype != chamber.create.archetype {
                                   let fail = Fail::Fail(fail::Fail::Resource(fail::resource::Fail::Create(fail::resource::Create::CannotUpdateArchetype)));
                                   return Err(fail);
                               }
                               match &chamber.create.state_src {
                                   AssignResourceStateSrc::Stateless => {
                                       // nothing left to do...
                                       return Ok(ResourceAction::None);
                                   }
                                   AssignResourceStateSrc::Direct(state) => {
                                       let resource = Resource::new(
                                           record.stub.address,
                                           record.stub.archetype,
                                           state.clone(),
                                       );
                                       return Ok(ResourceAction::Update(resource));
                                   }
                               }
                           }
                       }
                   }
                   Err(_) => {
                       // maybe this should be Option since using an error to signal not found
                       // might get confused with error for actual failure
                       let assign = ResourceAssign::new(AssignKind::Create, stub, chamber.create.state_src.clone() );
                       return Ok(ResourceAction::Assign(assign));
                   }
               }
           }

           let result = create(&self).await;
           self.tx.send(result);

        });
    }

    async fn finalize_create(
        &self,
        key: Address,
        address: Address,
    ) -> Result<ResourceAction<AssignResourceStateSrc>, Fail> {
        let stub = ResourceStub {
            address: address.clone(),
            archetype: self.create.archetype.clone(),
        };

        let assign = ResourceAssign {
            kind: AssignKind::Create,
            stub: stub,
            state_src: self.create.state_src.clone(),
        };
        Ok(ResourceAction::Assign(assign))
    }
}

#[async_trait]
pub trait ResourceHost: Send + Sync {
    fn star_key(&self) -> StarKey;
    async fn assign(&self, assign: ResourceAssign<AssignResourceStateSrc>) -> Result<(), Error>;

    async fn init(&self, key: Address) -> Result<(), Error>;
}

pub struct Registration {
    pub address: Address,
    pub kind: Kind,
    pub parent: Address,
    pub registry: SetRegistry,
    pub properties: SetProperties
}

pub struct RegistryReservation {
    tx: Option<oneshot::Sender<(ResourceRecord, oneshot::Sender<Result<(), Fail>>)>>,
}

impl RegistryReservation {
    pub fn commit(
        self,
        record: ResourceRecord,
        result_tx: oneshot::Sender<Result<(), Fail>>,
    ) -> Result<(), Fail> {
        if let Option::Some(tx) = self.tx {
            tx.send((record, result_tx))
                .or(Err(Fail::Error("could not send to tx".to_string())));
        }
        Ok(())
    }

    pub fn new(tx: oneshot::Sender<(ResourceRecord, oneshot::Sender<Result<(), Fail>>)>) -> Self {
        Self {
            tx: Option::Some(tx),
        }
    }

    pub fn empty() -> Self {
        RegistryReservation { tx: Option::None }
    }
}

pub struct FieldSelectionSql {
    selection: FieldSelection,
}

impl From<FieldSelection> for FieldSelectionSql {
    fn from(selection: FieldSelection) -> Self {
        Self { selection }
    }
}

impl ToSql for FieldSelectionSql {
    fn to_sql(&self) -> Result<ToSqlOutput<'_>, rusqlite::Error> {
        match self.to_sql_error() {
            Ok(ok) => Ok(ok),
            Err(err) => {
                eprintln!("{}", err.to_string());
                Err(rusqlite::Error::InvalidQuery)
            }
        }
    }
}

impl FieldSelectionSql {
    fn to_sql_error(&self) -> Result<ToSqlOutput<'_>, error::Error> {
        match &self.selection {
            FieldSelection::Identifier(id) => Ok(ToSqlOutput::Owned(Value::Blob(id.clone().key_or("(Identifier) selection fields must be turned into Addresss before they can be used by the ResourceRegistry")?.bin()?))),
            FieldSelection::Type(resource_type) => {
                Ok(ToSqlOutput::Owned(Value::Text(resource_type.to_string())))
            }
            FieldSelection::Kind(kind) => Ok(ToSqlOutput::Owned(Value::Text(kind.to_string()))),
            FieldSelection::Specific(specific) => {
                Ok(ToSqlOutput::Owned(Value::Text(specific.to_string())))
            }
            FieldSelection::Owner(owner) => {
                Ok(ToSqlOutput::Owned(Value::Blob(owner.clone().bin()?)))
            }
            FieldSelection::Parent(parent_id) => Ok(ToSqlOutput::Owned(Value::Blob(parent_id.clone().key_or("(Parent) selection fields must be turned into Addresss before they can be used by the ResourceRegistry")?.bin()?))),
        }
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRegistration {
    pub resource: ResourceRecord,
    pub info: Option<ResourceRegistryInfo>,
}

impl ResourceRegistration {
    pub fn new(resource: ResourceRecord, info: Option<ResourceRegistryInfo>) -> Self {
        ResourceRegistration {
            resource: resource,
            info: info,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLocationAffinity {
    pub star: StarKey,
}

impl From<ResourceRecord> for Address {
    fn from(record: ResourceRecord) -> Self {
        record.stub.key
    }
}

pub enum ResourceManagerKey {
    Central,
    Key(Address),
}

/*
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct ResourceAddress {
    resource_type: ResourceType,
    parts: Vec<ResourceAddressPart>,
}

impl ResourceAddress {
    pub fn path(&self) -> Result<Path, Error> {
        if self.parts.len() == 0 {
            Path::new("/")
        } else if let ResourceAddressPart::Path(path) = self.parts.last().unwrap() {
            Ok(path.clone())
        } else {
            Path::new(base64::encode(self.parts.last().unwrap().to_string().as_bytes()).as_str())
        }
    }

    pub fn last(&self) -> Option<ResourceAddressPart> {
        self.parts.last().cloned()
    }
}


impl ResourceSelectorId for ResourceAddress {}


impl ResourceAddress {
    pub fn root() -> Self {
        Self {
            resource_type: ResourceType::Root,
            parts: vec![],
        }
    }

    pub fn last_to_string(&self) -> Result<String, Error> {
        Ok(self.parts.last().ok_or("couldn't find last")?.to_string())
    }

    pub fn parent(&self) -> Option<ResourceAddress> {
        match &self.resource_type {
            ResourceType::Root => Option::None,
            ResourceType::FileSystem => match self.parts.last().unwrap().is_wildcard() {
                true => self.chop(1, ResourceType::App),
                false => self.chop(2, ResourceType::SubSpace),
            },
            ResourceType::Database => match self.parts.last().unwrap().is_wildcard() {
                true => self.chop(1, ResourceType::App),
                false => self.chop(2, ResourceType::SubSpace),
            },
            ResourceType::Space => Option::Some(Self::root()),
            ResourceType::SubSpace => self.chop(1, ResourceType::Space),
            ResourceType::App => self.chop(1, ResourceType::SubSpace),
            ResourceType::Actor => self.chop(1, ResourceType::Actor),
            ResourceType::User => self.chop(1, ResourceType::User),
            ResourceType::File => self.chop(1, ResourceType::FileSystem),
            ResourceType::Domain => self.chop(1, ResourceType::SubSpace),
            ResourceType::UrlPathPattern => self.chop(1, ResourceType::Space),
            ResourceType::Proxy => self.chop(1, ResourceType::SubSpace),
            ResourceType::ArtifactBundle => self.chop(2, ResourceType::SubSpace),
            ResourceType::Artifact => self.chop(1, ResourceType::ArtifactBundle),
        }
    }

    fn chop(&self, indices: usize, resource_type: ResourceType) -> Option<Self> {
        let mut parts = self.parts.clone();
        for i in 0..indices {
            if !parts.is_empty() {
                parts.pop();
            }
        }
        Option::Some(Self {
            resource_type: resource_type,
            parts: parts,
        })
    }

    pub fn parent_resource_type(&self) -> Option<ResourceType> {
        match self.resource_type {
            ResourceType::Root => Option::None,
            ResourceType::Space => Option::Some(ResourceType::Root),
            ResourceType::SubSpace => Option::Some(ResourceType::Space),
            ResourceType::App => Option::Some(ResourceType::SubSpace),
            ResourceType::Actor => Option::Some(ResourceType::App),
            ResourceType::User => Option::Some(ResourceType::Space),
            ResourceType::FileSystem => match self.parts.last().unwrap().is_wildcard() {
                true => Option::Some(ResourceType::App),
                false => Option::Some(ResourceType::SubSpace),
            },
            ResourceType::File => Option::Some(ResourceType::FileSystem),
            ResourceType::Domain => Option::Some(ResourceType::Space),
            ResourceType::UrlPathPattern => Option::Some(ResourceType::Domain),
            ResourceType::Proxy => Option::Some(ResourceType::Space),
            ResourceType::ArtifactBundle => Option::Some(ResourceType::SubSpace),
            ResourceType::Artifact => Option::Some(ResourceType::ArtifactBundle),
            ResourceType::Database => match self.parts.last().unwrap().is_wildcard() {
                true => Option::Some(ResourceType::App),
                false => Option::Some(ResourceType::SubSpace),
            },
        }
    }
    /*
    pub fn from_filename(value: &str) -> Result<Self,Error>{
        let split = value.split("_");
    }

    pub fn to_filename(&self) -> String {
        let mut rtn = String::new();
        for (index,part) in self.parts.iter().enumerate() {
            if index != 0 {
                rtn.push_str("_" );
            }
            let part = match part {
                ResourceAddressPart::Wildcard => {
                    "~"
                }
                ResourceAddressPart::SkewerCase(skewer) => {
                    skewer.to_string()
                }
                ResourceAddressPart::Domain(domain) => {
                    domain.to_string()
                }
                ResourceAddressPart::Base64Encoded(base) => {
                    base.to_string()
                }
                ResourceAddressPart::Path(path) => {
                    path.to_relative().replace("/", "+")
                }
                ResourceAddressPart::Version(version) => {
                    version.to_string()
                }
                ResourceAddressPart::Email(email) => {
                    email.to_string()
                }
                ResourceAddressPart::Url(url) => {
                    url.replace("/", "+")
                }
                ResourceAddressPart::UrlPathPattern(pattern) => {
                    let result = Base64Encoded::encoded(pattern.to_string());
                    if result.is_ok() {
                        result.unwrap().encoded
                    }
                    else{
                        "+++"
                    }
                }
            };
            rtn.push_str(part);
        }
        rtn
    }

     */

    pub fn for_space(string: &str) -> Result<Self, Error> {
        ResourceType::Space.address_structure().from_str(string)
    }

    pub fn for_sub_space(string: &str) -> Result<Self, Error> {
        ResourceType::SubSpace.address_structure().from_str(string)
    }

    pub fn for_app(string: &str) -> Result<Self, Error> {
        ResourceType::App.address_structure().from_str(string)
    }

    pub fn for_actor(string: &str) -> Result<Self, Error> {
        ResourceType::Actor.address_structure().from_str(string)
    }

    pub fn for_filesystem(string: &str) -> Result<Self, Error> {
        ResourceType::FileSystem
            .address_structure()
            .from_str(string)
    }

    pub fn for_file(string: &str) -> Result<Self, Error> {
        ResourceType::File.address_structure().from_str(string)
    }

    pub fn for_user(string: &str) -> Result<Self, Error> {
        ResourceType::User.address_structure().from_str(string)
    }

    pub fn test_address(key: &Address) -> Result<Self, Error> {
        let mut parts = vec![];

        let mut mark = Option::Some(key.clone());
        while let Option::Some(key) = mark {
            match &key {
                Address::Root => {
                    // do nothing
                }
                Address::Space(space) => {
                    parts.push(ResourceAddressPart::SkewerCase(SkewerCase::new(
                        format!("space-{}", space.id()).as_str(),
                    )?));
                }
                Address::SubSpace(sub_space) => {
                    parts.push(ResourceAddressPart::SkewerCase(SkewerCase::new(
                        format!("sub-{}", sub_space.id).as_str(),
                    )?));
                }
                Address::App(app) => {
                    parts.push(app.address_part()?);
                }
                Address::Actor(actor) => {
                    parts.push(actor.address_part()?);
                }
                Address::User(user) => {
                    parts.push(ResourceAddressPart::SkewerCase(SkewerCase::new(
                        format!("user-{}", user.id).as_str(),
                    )?));
                }
                Address::File(file) => {
                    parts.push(ResourceAddressPart::Path(Path::new(
                        format!("/files/{}", file.id).as_str(),
                    )?));
                }
                Address::FileSystem(filesystem) => match filesystem {
                    FileSystemKey::App(app) => {
                        parts.push(ResourceAddressPart::SkewerCase(SkewerCase::new(
                            format!("filesystem-{}", app.id).as_str(),
                        )?));
                    }
                    FileSystemKey::SubSpace(sub_space) => {
                        parts.push(ResourceAddressPart::SkewerCase(SkewerCase::new(
                            format!("filesystem-{}", sub_space.id).as_str(),
                        )?));
                        parts.push(ResourceAddressPart::Wildcard);
                    }
                },
                Address::Domain(domain) => {
                    parts.push(ResourceAddressPart::Domain(DomainCase::new(
                        format!("domain-{}", domain.id).as_str(),
                    )?));
                }
                Address::UrlPathPattern(pattern) => {
                    parts.push(ResourceAddressPart::UrlPathPattern(format!(
                        "url-path-pattern-{}",
                        pattern.id
                    )));
                }
                Address::Proxy(proxy) => {
                    parts.push(ResourceAddressPart::SkewerCase(SkewerCase::from_str(
                        format!("proxy-{}", proxy.id).as_str(),
                    )?));
                }
                Address::ArtifactBundle(bundle) => {
                    parts.push(ResourceAddressPart::SkewerCase(SkewerCase::from_str(
                        format!("artifact-bundle-{}", bundle.id).as_str(),
                    )?));
                    parts.push(ResourceAddressPart::Version(Version::from_str("1.0.0")?));
                }
                Address::Artifact(artifact) => {
                    parts.push(ResourceAddressPart::Path(Path::new(
                        format!("/artifacts/{}", artifact.id).as_str(),
                    )?));
                }
                Address::Database(_) => {
                    unimplemented!()
                }
            }

            mark = key.parent();
        }
        Ok(ResourceAddress::from_parts(&key.resource_type(), parts)?)
    }

    pub fn resource_type(&self) -> ResourceType {
        self.resource_type.clone()
    }

    pub fn space(&self) -> Result<ResourceAddress, Error> {
        Ok(SPACE_ADDRESS_STRUCT.from_str(
            self.parts
                .get(0)
                .ok_or("expected space")?
                .to_string()
                .as_str(),
        )?)
    }

    pub fn sub_space(&self) -> Result<ResourceAddress, Error> {
        if self.resource_type == ResourceType::Space {
            Err("Space ResourceAddress does not have a SubSpace".into())
        } else {
            Ok(SUB_SPACE_ADDRESS_STRUCT.from_str(
                format!(
                    "{}:{}",
                    self.parts.get(0).ok_or("expected space")?.to_string(),
                    self.parts.get(1).ok_or("expected sub_space")?.to_string()
                )
                .as_str(),
            )?)
        }
    }
    pub fn from_parent(
        resource_type: &ResourceType,
        parent: Option<&ResourceAddress>,
        part: ResourceAddressPart,
    ) -> Result<ResourceAddress, Error> {
        if !resource_type.parent().matches_address(parent) {
            return Err(format!(
                "resource type parent is wrong: expected: {}",
                resource_type.parent().to_string()
            )
            .into());
        }

        let mut parts = vec![];
        if let Option::Some(parent) = parent {
            parts.append(&mut parent.parts.clone());
        }
        parts.push(part);

        Self::from_parts(resource_type, parts)
    }

    pub fn from_parts(
        resource_type: &ResourceType,
        mut parts: Vec<ResourceAddressPart>,
    ) -> Result<ResourceAddress, Error> {
        for (index, part_struct) in resource_type.address_structure().parts.iter().enumerate() {
            let part = parts.get(index).ok_or("missing part")?;
            if !part_struct.kind.matches(part) {
                return Err(format!("part does not match {}", part_struct.kind.to_string()).into());
            }
        }

        Ok(ResourceAddress {
            parts: parts,
            resource_type: resource_type.clone(),
        })
    }

    pub fn part_to_string(&self, name: &str) -> Result<String, Error> {
        for (index, part_struct) in self
            .resource_type
            .address_structure()
            .parts
            .iter()
            .enumerate()
        {
            if part_struct.name == name.to_string() {
                let part = self.parts.get(index).ok_or(format!(
                    "missing part index {} for part name {}",
                    index, name
                ))?;
                return Ok(part.to_string());
            }
        }

        Err(format!("could not find part with name {}", name).into())
    }

    pub fn to_parts_string(&self) -> String {
        let mut rtn = String::new();

        for (index, part) in self.parts.iter().enumerate() {
            if index != 0 {
                rtn.push_str(RESOURCE_ADDRESS_DELIM)
            }
            rtn.push_str(part.to_string().as_str());
        }
        rtn
    }
}

impl ToString for ResourceAddress {
    fn to_string(&self) -> String {
        if self.resource_type == ResourceType::Root {
            return "<Root>".to_string();
        } else {
            let mut rtn = self.to_parts_string();

            rtn.push_str("::");
            rtn.push_str("<");
            rtn.push_str(self.resource_type.to_string().as_str());
            rtn.push_str(">");

            rtn
        }
    }
}

impl FromStr for ResourceAddress {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.trim() == "<Root>" {
            return Ok(ResourceAddress {
                parts: vec![],
                resource_type: ResourceType::Root,
            });
        }

        let mut split = s.split("::<");
        let address_structure = split
            .next()
            .ok_or("missing address structure at beginning i.e: 'space::sub_space::<SubSpace>")?;
        let mut resource_type_gen = split
            .next()
            .ok_or("missing resource type at end i.e.: 'space::sub_space::<SubSpace>")?
            .to_string();

        // chop off the generics i.e. <Space> remove '<' and '>'
        if resource_type_gen.len() < 1 {
            return Err(
                format!("not a valid resource type generic '{}'", resource_type_gen).into(),
            );
        }
        //        resource_type_gen.remove(0);
        resource_type_gen.remove(resource_type_gen.len() - 1);

        let resource_type = ResourceType::from_str(resource_type_gen.as_str())?;
        let resource_address = resource_type
            .address_structure()
            .from_str(address_structure)?;

        Ok(resource_address)
    }
}

*/
#[derive(Clone, Serialize, Deserialize)]
pub struct ResourceBinding {
    pub key: Address,
    pub address: ResourceAddress,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLocation {
    pub host: StarKey,
}

impl ResourceLocation {
    pub fn new(star: StarKey) -> Self {
        Self { host: star }
    }
    pub fn root() -> Self {
        Self {
            host: StarKey::central(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct DisplayValue {
    string: String,
}

impl DisplayValue {
    pub fn new(string: &str) -> Result<Self, Error> {
        if string.is_empty() {
            return Err("cannot be empty".into());
        }

        Ok(DisplayValue {
            string: string.to_string(),
        })
    }
}

impl ToString for DisplayValue {
    fn to_string(&self) -> String {
        self.string.clone()
    }
}

impl FromStr for DisplayValue {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(DisplayValue::new(s)?)
    }
}

#[derive(Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum ResourceSliceStatus {
    Unknown,
    Preparing,
    Waiting,
    Ready,
}

impl ToString for ResourceSliceStatus {
    fn to_string(&self) -> String {
        match self {
            ResourceSliceStatus::Unknown => "Unknown".to_string(),
            ResourceSliceStatus::Preparing => "Preparing".to_string(),
            ResourceSliceStatus::Waiting => "Waiting".to_string(),
            ResourceSliceStatus::Ready => "Ready".to_string(),
        }
    }
}

impl FromStr for ResourceSliceStatus {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Unknown" => Ok(Self::Unknown),
            "Preparing" => Ok(Self::Preparing),
            "Waiting" => Ok(Self::Waiting),
            "Ready" => Ok(Self::Ready),
            what => Err(format!("not recognized: {}", what).into()),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ResourceSliceAssign {
    key: Address,
    archetype: ResourceArchetype,
}

/*j
impl TryInto<ResourceAssign<DataSet<BinSrc>>> for ResourceAssign<AssignResourceStateSrc<DataSet<BinSrc>>> {
    type Error = Error;

    fn try_into(self) -> Result<ResourceAssign<DataSet<BinSrc>>, Self::Error> {
        Ok(ResourceAssign {
            stub: self.stub,
            state_src: self.state_src.try_into()?,
        })
    }
}

 */

pub struct RemoteResourceHost {
    pub skel: StarSkel,
    pub handle: StarConscript,
}

#[async_trait]
impl ResourceHost for RemoteResourceHost {
    fn star_key(&self) -> StarKey {
        self.handle.key.clone()
    }

    async fn assign(&self, assign: ResourceAssign<AssignResourceStateSrc>) -> Result<(), Error> {
        if !self
            .handle
            .kind
            .hosted()
            .contains(&assign.stub.key.resource_type())
        {
            return Err(Fail::WrongResourceType {
                expected: self.handle.kind.hosted().clone(),
                received: assign.stub.key.resource_type().clone(),
            }
            .into());
        }

        let mut proto = ProtoStarMessage::new();
        proto.to = self.handle.key.clone().into();
        proto.payload =
            StarMessagePayload::ResourceHost(ResourceHostAction::Assign(assign.try_into()?));

        self.skel
            .messaging_api
            .exchange(
                proto,
                ReplyKind::Empty,
                "RemoteResourceHost: assign resource to host",
            )
            .await?;

        Ok(())
    }

    async fn init(&self, key: Address) -> Result<(), Error> {
        let mut proto = ProtoStarMessage::new();
        proto.to = self.handle.key.clone().into();
        proto.payload = StarMessagePayload::ResourceHost(ResourceHostAction::Init(key));

        self.skel
            .messaging_api
            .exchange(
                proto,
                ReplyKind::Empty,
                "RemoteResourceHost: create resource on host",
            )
            .await?;

        Ok(())
    }
}

pub trait ResourceSelectorId:
    Debug
    + Clone
    + Serialize
    + for<'de> Deserialize<'de>
    + Eq
    + PartialEq
    + Hash
    + Into<Address>
    + Sized
{
}

#[async_trait]
pub trait UniqueSrc: Send + Sync {
    async fn next(&self, resource_type: &ResourceType) -> Result<ResourceId, Error>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRecord {
    pub stub: ResourceStub,
    pub location: ResourceLocation,
}

impl ResourceRecord {
    pub fn new(stub: ResourceStub, host: StarKey) -> Self {
        ResourceRecord {
            stub: stub,
            location: ResourceLocation::new(host),
        }
    }

    pub fn root() -> Self {
        Self {
            stub: ResourceStub::root(),
            location: ResourceLocation::root(),
        }
    }
}

impl Into<ResourceStub> for ResourceRecord {
    fn into(self) -> ResourceStub {
        self.stub
    }
}

#[derive(
    Clone,
    Serialize,
    Deserialize,
    Eq,
    PartialEq,
    Hash,
    strum_macros::Display,
    strum_macros::EnumString,
)]
pub enum ResourceType {
    Root,
    Space,
    Base,
    User,
    App,
    Mechtron,
    FileSystem,
    File,
    Database,
    Authenticator,
    ArtifactBundleSeries,
    ArtifactBundle,
    Artifact,
    Proxy,
    Credentials,
}

#[derive(
    Clone,
    Serialize,
    Deserialize,
    Eq,
    PartialEq,
    Hash,
    strum_macros::Display,
    strum_macros::EnumString,
)]
pub enum Kind {
    Root,
    Space,
    Base(BaseKind),
    User,
    App,
    Mechtron,
    FileSystem,
    File(FileKind),
    Database(DatabaseKind),
    Authenticator,
    ArtifactBundleSeries,
    ArtifactBundle,
    Artifact(ArtifactKind),
    Proxy,
    Credentials,
}

impl Kind {
    pub fn resource_type(&self) -> ResourceType {
        match self {
            Kind::Root => ResourceType::Root,
            Kind::Space => ResourceType::Space,
            Kind::Base(_) => ResourceType::Base,
            Kind::User => ResourceType::User,
            Kind::App => ResourceType::App,
            Kind::Mechtron => ResourceType::Mechtron,
            Kind::FileSystem => ResourceType::FileSystem,
            Kind::File(_) => ResourceType::File,
            Kind::Database(_) => ResourceType::Database,
            Kind::Authenticator => ResourceType::Authenticator,
            Kind::ArtifactBundleSeries => ResourceType::ArtifactBundleSeries,
            Kind::ArtifactBundle => ResourceType::ArtifactBundle,
            Kind::Artifact(_) => ResourceType::Artifact,
            Kind::Proxy => ResourceType::Proxy,
            Kind::Credentials => ResourceType::Credentials,
        }
    }

    pub fn sub_string(&self) -> Option<String> {
        match self {
            Self::Base(base) =>  {
                Option::Some(base.to_string())
            }
            Self::File( file ) => {
                Option::Some(file.to_string())
            }
            Self::Database( db) => {
                Option::Some(db.to_string())
            }
            Self::Artifact( artifact) => {
                Option::Some(artifact.to_string())
            }
            _ => {
                Option::None
            }
        }
    }

    pub fn specific(&self) -> Option<Specific> {
        match self {
            Self::Database(kind) => kind.specific(),
            _ => Option::None,
        }
    }

    pub fn from( resource_type: ResourceType, kind: Option<String>, specific: Option<Specific> ) -> Result<Self,Error> {
        Ok(match resource_type {
            ResourceType::Root => {Self::Root}
            ResourceType::Space => {Self::Space}
            ResourceType::Base => {
                let kind = kind.ok_or("expected sub kind".into() )?;
                Self::Base(BaseKind::from_str(kind)?)
            }
            ResourceType::User => { Self::User}
            ResourceType::App => {Self::App}
            ResourceType::Mechtron => {Self::Mechtron}
            ResourceType::FileSystem => {Self::FileSystem}
            ResourceType::File => {Self::File}
            ResourceType::Database => {
                let kind = kind.ok_or("expected sub kind".into() )?;
                if "Relational" != kind.as_str() {
                    return Err(format!("DatabaseKind is not recognized found: {}",kind).into());
                }
                let specific = specific.ok_or("expected specific".into() )?;
                Self::Database(DatabaseKind::Relational(specific))
            }
            ResourceType::Authenticator => {Self::Authenticator}
            ResourceType::ArtifactBundleSeries => {Self::ArtifactBundleSeries}
            ResourceType::ArtifactBundle => {Self::ArtifactBundle}
            ResourceType::Artifact => {
                let kind = kind.ok_or("expected sub kind".into() )?;
                Self::Artifact(ArtifactKind::from_str(kind)?)
            }
            ResourceType::Proxy => {Self::Proxy}
            ResourceType::Credentials => {Self::Credentials}
        })
    }
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Hash,
    Serialize,
    Deserialize,
    strum_macros::Display,
)]
pub enum DatabaseKind {
    Relational(Specific),
}

impl DatabaseKind {
    pub fn specific(&self) -> Option<Specific> {
        match self {
            Self::Relational(specific) => Option::Some(specific.clone()),
            _ => Option::None,
        }
    }
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Hash,
    Serialize,
    Deserialize,
    strum_macros::Display,
    strum_macros::EnumString,
)]
pub enum BaseKind {
    User,
    App,
    Mechtron,
    Database,
    Any,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Hash,
    Serialize,
    Deserialize,
    strum_macros::Display,
    strum_macros::EnumString,
)]
pub enum FileKind {
    File,
    Dir,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Hash,
    Serialize,
    Deserialize,
    strum_macros::Display,
    strum_macros::EnumString,
)]
pub enum ArtifactKind {
    Raw,
    AppConfig,
    MechtronConfig,
    BindConfig,
    Wasm,
    HttpRouter,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub address: Address,
    pub archetype: ResourceArchetype,
    pub state: Payload,
}

impl Resource {
    pub fn new(address: Address, archetype: ResourceArchetype, state: Payload) -> Resource {
        Resource {
            address: address,
            state: state_src,
            archetype: archetype,
        }
    }

    pub fn address(&self) -> Address {
        self.address.clone()
    }

    pub fn resource_type(&self) -> ResourceType {
        self.key.resource_type()
    }

    pub fn state_src(&self) -> Payload {
        self.state.clone()
    }
}

/// can have other options like to Initialize the state data
#[derive(Debug, Clone, Serialize, Deserialize, strum_macros::Display)]
pub enum AssignResourceStateSrc {
    Stateless,
    Direct(Payload),
}


pub enum AssignKind {
    Create,
    // eventually we will have Move as well as Create
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAssign<S> {
    pub kind: AssignKind,
    pub stub: ResourceStub,
    pub state_src: S,
}


impl<S> ResourceAssign<S> {

    pub fn new( kind: AssignKind, stub: ResourceStub, state_src: S ) -> Self {
        Self {
            kind,
            stub,
            state_src
        }
    }

    pub fn archetype(&self) -> Archetype {
        self.stub.archetype.clone()
    }
}
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct ResourceKindParts {
    pub resource_type: String,
    pub kind: Option<String>,
    pub specific: Option<Specific>,
}

impl ToString for ResourceKindParts {
    fn to_string(&self) -> String {
        if self.specific.is_some() && self.kind.is_some() {
            format!(
                "<{}<{}<{}>>>",
                self.resource_type,
                self.kind.as_ref().unwrap().to_string(),
                self.specific.as_ref().unwrap().to_string()
            )
        } else if self.kind.is_some() {
            format!(
                "<{}<{}>>",
                self.resource_type,
                self.kind.as_ref().unwrap().to_string()
            )
        } else {
            format!("<{}>", self.resource_type)
        }
    }
}

impl FromStr for ResourceKindParts {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (leftover, rtn) = parse_kind(s)?;
        if leftover.len() > 0 {
            return Err(format!(
                "ResourceKindParts ERROR: could not parse extra: '{}' in string '{}'",
                leftover, s
            )
            .into());
        }
        Ok(rtn)
    }
}
