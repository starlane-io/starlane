use std::collections::{HashMap, HashSet};
use std::convert::{TryFrom, TryInto};
use std::fmt;
use std::fmt::{Debug, Formatter};
use std::fs::DirBuilder;
use std::future::Future;
use std::hash::Hash;
use std::iter::FromIterator;
use std::path::PathBuf;
use std::str::FromStr;
use std::string::FromUtf8Error;
use std::sync::Arc;
use std::time::Duration;

use base64::DecodeError;
use bincode::ErrorKind;
use rusqlite::{Connection, params, params_from_iter, Row, Rows, Statement, ToSql, Transaction};
use rusqlite::types::{ToSqlOutput, Value, ValueRef};
use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;
use serde_json::to_string;
use tokio::sync::{mpsc, oneshot};
use tokio::sync::oneshot::error::RecvError;
use tokio::sync::oneshot::Receiver;
use url::Url;

use crate::{logger, util, resource};
use crate::actor::{ActorKey, ActorKind};
use crate::app::ConfigSrc;
use crate::artifact::{ArtifactKey, ArtifactKind, ArtifactAddress};
use crate::error::Error;
use crate::file_access::FileAccess;
use crate::frame::{
    ChildManagerResourceAction, MessagePayload, Reply, ResourceHostAction, SimpleReply,
    StarMessagePayload,
};
use crate::id::{Id, IdSeq};
use crate::keys::{
    AppFilesystemKey, AppKey, FileSystemKey, GatheringKey, ResourceKey, SpaceKey,
    SubSpaceFilesystemKey, SubSpaceId, SubSpaceKey, UserKey,
};
use crate::keys::{FileKey, ResourceId, Unique, UniqueSrc};
use crate::logger::{elog, LogInfo, StaticLogInfo};
use crate::message::{Fail, ProtoStarMessage};
use crate::message::resource::{
    Message, MessageFrom, MessageReply, MessageTo, ProtoMessage, ResourceRequestMessage,
    ResourceResponseMessage,
};
use crate::names::{Name, Specific};
use crate::permissions::User;
use crate::resource::ResourceKind::UrlPathPattern;
use crate::resource::space::{Space, SpaceState};
use crate::resource::sub_space::SubSpaceState;
use crate::resource::user::UserState;
use crate::star::{
    ResourceRegistryBacking, StarComm, StarCommand, StarInfo, StarKey, StarKind, StarSkel,
};
use crate::star::pledge::{ResourceHostSelector, StarHandle};
use crate::starlane::api::StarlaneApi;
use crate::util::AsyncHashMap;
use clap::{App, Arg};
use crate::resource::address::ResourceKindParts;

pub mod artifact;
pub mod config;
pub mod domain;
pub mod file;
pub mod file_system;
pub mod space;
pub mod store;
pub mod sub_space;
pub mod user;
pub mod selector;
pub mod create_args;
pub mod address;

lazy_static! {
    pub static ref ROOT_STRUCT: ResourceAddressStructure =
        ResourceAddressStructure::new(vec![], ResourceType::Root);
    pub static ref SPACE_ADDRESS_STRUCT: ResourceAddressStructure = ResourceAddressStructure::new(
        vec![ResourceAddressPartStruct::new(
            "space",
            ResourceAddressPartKind::SkewerCase
        )],
        ResourceType::Space
    );
    pub static ref SUB_SPACE_ADDRESS_STRUCT: ResourceAddressStructure =
        ResourceAddressStructure::new(
            vec![
                ResourceAddressPartStruct::new("space", ResourceAddressPartKind::SkewerCase),
                ResourceAddressPartStruct::new("sub-space", ResourceAddressPartKind::SkewerCase)
            ],
            ResourceType::SubSpace
        );
    pub static ref APP_ADDRESS_STRUCT: ResourceAddressStructure = ResourceAddressStructure::new(
        vec![
            ResourceAddressPartStruct::new("space", ResourceAddressPartKind::SkewerCase),
            ResourceAddressPartStruct::new("sub-space", ResourceAddressPartKind::SkewerCase),
            ResourceAddressPartStruct::new("app", ResourceAddressPartKind::SkewerCase)
        ],
        ResourceType::App
    );
    pub static ref ACTOR_ADDRESS_STRUCT: ResourceAddressStructure = ResourceAddressStructure::new(
        vec![
            ResourceAddressPartStruct::new("space", ResourceAddressPartKind::SkewerCase),
            ResourceAddressPartStruct::new("sub-space", ResourceAddressPartKind::SkewerCase),
            ResourceAddressPartStruct::new("app", ResourceAddressPartKind::SkewerCase),
            ResourceAddressPartStruct::new("actor", ResourceAddressPartKind::SkewerCase)
        ],
        ResourceType::Actor
    );
    pub static ref USER_ADDRESS_STRUCT: ResourceAddressStructure = ResourceAddressStructure::new(
        vec![
            ResourceAddressPartStruct::new("space", ResourceAddressPartKind::SkewerCase),
            ResourceAddressPartStruct::new("email", ResourceAddressPartKind::Email)
        ],
        ResourceType::User
    );
    pub static ref FILE_SYSTEM_ADDRESS_STRUCT: ResourceAddressStructure =
        ResourceAddressStructure::new(
            vec![
                ResourceAddressPartStruct::new("space", ResourceAddressPartKind::SkewerCase),
                ResourceAddressPartStruct::new("sub-space", ResourceAddressPartKind::SkewerCase),
                ResourceAddressPartStruct::new("app", ResourceAddressPartKind::WildcardOrSkewer),
                ResourceAddressPartStruct::new("file-system", ResourceAddressPartKind::SkewerCase)
            ],
            ResourceType::FileSystem
        );
    pub static ref DATABASE_ADDRESS_STRUCT: ResourceAddressStructure =
        ResourceAddressStructure::new(
            vec![
                ResourceAddressPartStruct::new("space", ResourceAddressPartKind::SkewerCase),
                ResourceAddressPartStruct::new("sub-space", ResourceAddressPartKind::SkewerCase),
                ResourceAddressPartStruct::new("app", ResourceAddressPartKind::WildcardOrSkewer),
                ResourceAddressPartStruct::new("file-system", ResourceAddressPartKind::SkewerCase)
            ],
            ResourceType::Database
        );
    pub static ref FILE_ADDRESS_STRUCT: ResourceAddressStructure = ResourceAddressStructure::new(
        vec![
            ResourceAddressPartStruct::new("space", ResourceAddressPartKind::SkewerCase),
            ResourceAddressPartStruct::new("sub-space", ResourceAddressPartKind::SkewerCase),
            ResourceAddressPartStruct::new("app", ResourceAddressPartKind::WildcardOrSkewer),
            ResourceAddressPartStruct::new("file-system", ResourceAddressPartKind::SkewerCase),
            ResourceAddressPartStruct::new("path", ResourceAddressPartKind::Path)
        ],
        ResourceType::File
    );
    pub static ref ARTIFACT_BUNDLE_ADDRESS_STRUCT: ResourceAddressStructure =
        ResourceAddressStructure::new(
            vec![
                ResourceAddressPartStruct::new("space", ResourceAddressPartKind::SkewerCase),
                ResourceAddressPartStruct::new("sub-space", ResourceAddressPartKind::SkewerCase),
                ResourceAddressPartStruct::new("bundle", ResourceAddressPartKind::SkewerCase),
                ResourceAddressPartStruct::new("version", ResourceAddressPartKind::Version)
            ],
            ResourceType::ArtifactBundle
        );
    pub static ref ARTIFACT_ADDRESS_STRUCT: ResourceAddressStructure =
        ResourceAddressStructure::new(
            vec![
                ResourceAddressPartStruct::new("space", ResourceAddressPartKind::SkewerCase),
                ResourceAddressPartStruct::new("sub-space", ResourceAddressPartKind::SkewerCase),
                ResourceAddressPartStruct::new("bundle", ResourceAddressPartKind::SkewerCase),
                ResourceAddressPartStruct::new("version", ResourceAddressPartKind::Version),
                ResourceAddressPartStruct::new("path", ResourceAddressPartKind::Path)
            ],
            ResourceType::Artifact
        );
    pub static ref URL_ADDRESS_STRUCT: ResourceAddressStructure = ResourceAddressStructure::new(
        vec![
            ResourceAddressPartStruct::new("space", ResourceAddressPartKind::SkewerCase),
            ResourceAddressPartStruct::new("domain", ResourceAddressPartKind::Domain),
            ResourceAddressPartStruct::new("url", ResourceAddressPartKind::Url)
        ],
        ResourceType::UrlPathPattern
    );
    pub static ref PROXY_ADDRESS_STRUCT: ResourceAddressStructure = ResourceAddressStructure::new(
        vec![
            ResourceAddressPartStruct::new("space", ResourceAddressPartKind::SkewerCase),
            ResourceAddressPartStruct::new("url", ResourceAddressPartKind::Url)
        ],
        ResourceType::Proxy
    );
    pub static ref DOMAIN_ADDRESS_STRUCT: ResourceAddressStructure = ResourceAddressStructure::new(
        vec![
            ResourceAddressPartStruct::new("space", ResourceAddressPartKind::SkewerCase),
            ResourceAddressPartStruct::new("domain", ResourceAddressPartKind::Domain)
        ],
        ResourceType::Domain
    );
    pub static ref HYPERSPACE_ADDRESS: ResourceAddress =
        SPACE_ADDRESS_STRUCT.from_str("hyperspace").unwrap();
    pub static ref HYPERSPACE_DEFAULT_ADDRESS: ResourceAddress = SUB_SPACE_ADDRESS_STRUCT
        .from_str("hyperspace:default")
        .unwrap();
}

static RESOURCE_ADDRESS_DELIM: &str = ":";

//static RESOURCE_QUERY_FIELDS: &str = "r.key,r.address,r.kind,r.specific,r.owner,r.config,r.host,r.gathering";
static RESOURCE_QUERY_FIELDS: &str = "r.key,r.address,r.kind,r.specific,r.owner,r.config,r.host";

pub type Labels = HashMap<String, String>;
pub type Names = Vec<String>;



#[derive(Debug,Clone, Serialize, Deserialize)]
pub struct ResourceSelector {
    meta: MetaSelector,
    fields: HashSet<FieldSelection>,
}


impl ResourceSelector {
    pub async fn to_keyed(self, starlane_api: StarlaneApi ) -> Result<ResourceSelector,Error>{
        let mut fields:HashSet<FieldSelection> = HashSet::new();

        for field in self.fields  {
            fields.insert(field.to_keyed(&starlane_api).await?.into() );
        }

        Ok(ResourceSelector {
            meta: self.meta,
            fields: fields
        })
    }

    pub fn children_selector( parent: ResourceIdentifier ) -> Self {
        let mut selector = Self::new();
        selector.add_field(FieldSelection::Parent(parent));
        selector
    }

    pub fn children_of_type_selector( parent: ResourceIdentifier, child_type: ResourceType ) -> Self {
        let mut selector = Self::new();
        selector.add_field(FieldSelection::Parent(parent));
        selector.add_field(FieldSelection::Type(child_type));
        selector
    }


    pub fn app_selector() -> Self {
        let mut selector = Self::new();
        selector.add_field(FieldSelection::Type(ResourceType::App));
        selector
    }

    pub fn actor_selector() -> Self {
        let mut selector = Self::new();
        selector.add_field(FieldSelection::Type(ResourceType::Actor));
        selector
    }
}


#[derive(Debug,Clone, Serialize, Deserialize)]
pub enum MetaSelector {
    None,
    Name(String),
    Label(LabelSelector),
}

#[derive(Debug,Clone, Serialize, Deserialize)]
pub struct LabelSelector {
    pub labels: HashSet<LabelSelection>,
}

impl ResourceSelector{
    pub fn new() -> Self{
        let mut fields = HashSet::new();
        ResourceSelector {
            meta: MetaSelector::None,
            fields: fields
        }
    }

    pub fn resource_types(&self) -> HashSet<ResourceType> {
        let mut rtn = HashSet::new();
        for field in &self.fields {
            if let FieldSelection::Type(resource_type) = field {
                rtn.insert(resource_type.clone());
            }
        }
        rtn
    }

    pub fn add(&mut self, field: FieldSelection) {
        self.fields.retain(|f| !f.is_matching_kind(&field));
        self.fields.insert(field);
    }

    pub fn is_empty(&self) -> bool {
        if !self.fields.is_empty() {
            return false;
        }

        match &self.meta {
            MetaSelector::None => {
                return true;
            }
            MetaSelector::Name(_) => {
                return false;
            }
            MetaSelector::Label(labels) => {
                return labels.labels.is_empty();
            }
        };
    }

    pub fn name(&mut self, name: String) -> Result<(), Error> {
        match &mut self.meta {
            MetaSelector::None => {
                self.meta = MetaSelector::Name(name.clone());
                Ok(())
            }
            MetaSelector::Name(_) => {
                self.meta = MetaSelector::Name(name.clone());
                Ok(())
            }
            MetaSelector::Label(selector) => {
                Err("Selector is already set to a LABEL meta selector".into())
            }
        }
    }

    pub fn add_label(&mut self, label: LabelSelection) -> Result<(), Error> {
        match &mut self.meta {
            MetaSelector::None => {
                self.meta = MetaSelector::Label(LabelSelector {
                    labels: HashSet::new(),
                });
                self.add_label(label)
            }
            MetaSelector::Name(_) => Err("Selector is already set to a NAME meta selector".into()),
            MetaSelector::Label(selector) => {
                selector.labels.insert(label);
                Ok(())
            }
        }
    }

    pub fn add_field(&mut self, field: FieldSelection) {
        self.fields.insert(field);
    }
}


#[derive(Debug,Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum LabelSelection {
    Exact(Label),
}

impl LabelSelection {
    pub fn exact(name: &str, value: &str) -> Self {
        LabelSelection::Exact(Label {
            name: name.to_string(),
            value: value.to_string(),
        })
    }
}

#[derive(Debug,Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum FieldSelection {
    Identifier(ResourceIdentifier),
    Type(ResourceType),
    Kind(ResourceKind),
    Specific(Specific),
    Owner(UserKey),
    Parent(ResourceIdentifier)
}

impl FieldSelection {
    pub async fn to_keyed(mut self, starlane_api: &StarlaneApi ) -> Result<FieldSelection,Error> {
        match self{
            FieldSelection::Identifier(id) => {
                Ok(FieldSelection::Identifier(id.to_key(starlane_api).await?.into()))
            }
            FieldSelection::Type(resource_type) => Ok(FieldSelection::Type(resource_type)),
            FieldSelection::Kind(kind) => Ok(FieldSelection::Kind(kind)),
            FieldSelection::Specific(specific) => Ok(FieldSelection::Specific(specific)),
            FieldSelection::Owner(owner) => Ok(FieldSelection::Owner(owner)),
            FieldSelection::Parent(id) => {
                Ok(FieldSelection::Parent(id.to_key(starlane_api).await?.into()))
            }
        }
    }
}

impl ToString for FieldSelection{
    fn to_string(&self) -> String {
        match self {
            FieldSelection::Identifier(id) => {
                id.to_string()
            }
            FieldSelection::Type(rt) => {
                rt.to_string()
            }
            FieldSelection::Kind(kind) => {
                kind.to_string()
            }
            FieldSelection::Specific(specific) => {
                specific.to_string()
            }
            FieldSelection::Owner(owner) => {
                owner.to_string()
            }
            FieldSelection::Parent(parent) => {
                parent.to_string()
            }
        }
    }
}

impl ToSql for Name {
    fn to_sql(&self) -> Result<ToSqlOutput<'_>, rusqlite::Error> {
        Ok(ToSqlOutput::Owned(Value::Text(self.to())))
    }
}

impl  FieldSelection {
    pub fn is_matching_kind(&self, field: &FieldSelection) -> bool {
        match self {
            FieldSelection::Identifier(_) => {
                if let FieldSelection::Identifier(_) = field {
                    return true;
                }
            }
            FieldSelection::Type(_) => {
                if let FieldSelection::Type(_) = field {
                    return true;
                }
            }
            FieldSelection::Kind(_) => {
                if let FieldSelection::Kind(_) = field {
                    return true;
                }
            }
            FieldSelection::Specific(_) => {
                if let FieldSelection::Specific(_) = field {
                    return true;
                }
            }
            FieldSelection::Owner(_) => {
                if let FieldSelection::Owner(_) = field {
                    return true;
                }
            }
            FieldSelection::Parent(_) => {
                if let FieldSelection::Parent(_) = field {
                    return true;
                }
            }
        };
        return false;
    }
}

impl ToSql for FieldSelection {
    fn to_sql(&self) -> Result<ToSqlOutput<'_>, rusqlite::Error> {
        match self {
            FieldSelection::Identifier(id) => Ok(ToSqlOutput::Owned(Value::Blob(id.clone().key_or("(Identifier) selection fields must be turned into ResourceKeys before they can be used by the ResourceRegistry")?.bin()?))),
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
            FieldSelection::Parent(parent_id) => Ok(ToSqlOutput::Owned(Value::Blob(parent_id.clone().key_or("(Parent) selection fields must be turned into ResourceKeys before they can be used by the ResourceRegistry")?.bin()?))),
        }
    }
}

#[derive(Debug,Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct Label {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LabelConfig {
    pub name: String,
    pub index: bool,
}

pub struct ResourceRegistryAction {
    pub tx: oneshot::Sender<ResourceRegistryResult>,
    pub command: ResourceRegistryCommand,
}

impl ResourceRegistryAction {
    pub fn new(
        command: ResourceRegistryCommand,
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

pub enum ResourceRegistryCommand {
    Close,
    Clear,
    //Accepts(HashSet<ResourceType>),
    Reserve(ResourceNamesReservationRequest),
    Commit(ResourceRegistration),
    Select(ResourceSelector),
    SetLocation(ResourceRecord),
    Get(ResourceIdentifier),
    Next { key: ResourceKey, unique: Unique },
}

pub enum ResourceRegistryResult {
    Ok,
    Error(String),
    Resource(Option<ResourceRecord>),
    Resources(Vec<ResourceRecord>),
    Address(ResourceAddress),
    Reservation(RegistryReservation),
    Key(ResourceKey),
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
    key: Option<Blob>,
    address: Option<String>,
    resource_type: String,
    kind: String,
    specific: Option<String>,
    config: Option<String>,
    owner: Option<Blob>,
    host: Option<Blob>,
    parent: Option<Blob>,
}

impl RegistryParams {
    pub fn from_registration(registration: ResourceRegistration) -> Result<Self, Error> {
        Self::new(
            registration.resource.stub.archetype,
            registration.resource.stub.key.parent(),
            Option::Some(registration.resource.stub.key),
            registration.resource.stub.owner,
            Option::Some(registration.resource.stub.address),
            Option::Some(registration.resource.location.host),
        )
    }

    pub fn from_archetype(
        archetype: ResourceArchetype,
        parent: Option<ResourceKey>,
    ) -> Result<Self, Error> {
        Self::new(
            archetype,
            parent,
            Option::None,
            Option::None,
            Option::None,
            Option::None,
        )
    }

    pub fn new(
        archetype: ResourceArchetype,
        parent: Option<ResourceKey>,
        key: Option<ResourceKey>,
        owner: Option<UserKey>,
        address: Option<ResourceAddress>,
        host: Option<StarKey>,
    ) -> Result<Self, Error> {
        let key = if let Option::Some(key) = key {
            Option::Some(key.bin()?)
        } else {
            Option::None
        };

        let address = if let Option::Some(address) = address {
            Option::Some(address.to_string())
        } else {
            Option::None
        };

        let resource_type = archetype.kind.resource_type().to_string();
        let kind = archetype.kind.to_string();

        let owner = if let Option::Some(owner) = owner {
            Option::Some(owner.bin()?)
        } else {
            Option::None
        };

        let specific = match &archetype.specific {
            None => Option::None,
            Some(specific) => Option::Some(specific.to_string()),
        };

        let config = match &archetype.config {
            None => Option::None,
            Some(config) => Option::Some(config.to_string()),
        };

        let parent = match parent {
            None => Option::None,
            Some(parent) => {
                Option::Some(parent.bin()?)
            },
        };

        let host = match host {
            Some(host) => Option::Some(host.bin()?),
            None => Option::None,
        };

        Ok(RegistryParams {
            key: key,
            address: address,
            resource_type: resource_type,
            kind: kind,
            specific: specific,
            parent: parent,
            config: config,
            owner: owner,
            host: host,
        })
    }
}

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
        if let Result::Err(_) = dir_builder.create(path.clone())
        {
            eprintln!("FATAL: could not create star data directory: {}",path );
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
                let log_info = StaticLogInfo::new("ResourceRegistry".to_string(), star_info.log_kind().to_string(), star_info.key.to_string()  );
                eprintln!("connection ERROR!");
                logger::elog(
                    &log_info,
                    &star_info,
                    "new()",
                    format!(
                        "ERROR: could not create SqLite connection to database: '{}'", conn.err().unwrap().to_string(),
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
            if let ResourceRegistryCommand::Close = request.command {
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
        command: ResourceRegistryCommand,
    ) -> Result<ResourceRegistryResult, Error> {
        match command {
            ResourceRegistryCommand::Close => Ok(ResourceRegistryResult::Ok),
            ResourceRegistryCommand::Clear => {
                let trans = self.conn.transaction()?;
                trans.execute("DELETE FROM labels", [])?;
                trans.execute("DELETE FROM names", [])?;
                trans.execute("DELETE FROM resources", [])?;
                trans.execute("DELETE FROM uniques", [])?;
                trans.commit()?;

                Ok(ResourceRegistryResult::Ok)
            }

            ResourceRegistryCommand::Commit(registration) => {
                let params = RegistryParams::from_registration(registration.clone())?;

                let trans = self.conn.transaction()?;

                if params.key.is_some() {
                    trans.execute(
                        "DELETE FROM labels WHERE labels.resource_key=?1",
                        [params.key.clone()],
                    );
                    trans.execute("DELETE FROM resources WHERE key=?1", [params.key.clone()])?;
                }

                trans.execute("INSERT INTO resources (key,address,resource_type,kind,specific,parent,owner,config,host) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)", params![params.key,params.address,params.resource_type,params.kind,params.specific,params.parent,params.owner,params.config,params.host])?;
                if let Option::Some(info) = registration.info {
                    for name in info.names {
                        trans.execute("UPDATE names SET key=?1 WHERE name=?1", [name])?;
                    }
                    for (name, value) in info.labels {
                        trans.execute(
                            "INSERT INTO labels (resource_key,name,value) VALUES (?1,?2,?3)",
                            params![params.key, name, value],
                        )?;
                    }
                }

                trans.commit()?;
                Ok(ResourceRegistryResult::Ok)
            }
            ResourceRegistryCommand::Select(selector) => {
                let mut params = vec![];
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
                        FieldSelection::Identifier(_) => {
                            format!("r.key=?{}", index + 1)
                        }
                        FieldSelection::Type(_) => {
                            format!("r.resource_type=?{}", index + 1)
                        }
                        FieldSelection::Kind(_) => {
                            format!("r.kind=?{}", index + 1)
                        }
                        FieldSelection::Specific(_) => {
                            format!("r.specific=?{}", index + 1)
                        }
                        FieldSelection::Owner(_) => {
                            format!("r.owner=?{}", index + 1)
                        }
                        FieldSelection::Parent(_) => {
                            format!("r.parent=?{}", index + 1)
                        }
                    };
                    where_clause.push_str(f.as_str());
                    params.push(field);
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
                        for (index, label_selection) in
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
            ResourceRegistryCommand::SetLocation(location_record) => {
                let key = location_record.stub.key.bin()?;
                let host = location_record.location.host.bin()?;
                let gathering = match location_record.location.gathering {
                    None => Option::None,
                    Some(key) => Option::Some(key.bin()?),
                };
                let mut trans = self.conn.transaction()?;
                trans.execute(
                    "UPDATE resources SET host=?1, gathering=?2 WHERE key=?3",
                    params![host, gathering, key],
                )?;
                trans.commit()?;
                Ok(ResourceRegistryResult::Ok)
            }
            ResourceRegistryCommand::Get(identifier) => {

                if( identifier.resource_type() == ResourceType::Root ) {
                    return Ok(ResourceRegistryResult::Resource(Option::Some(ResourceRecord::root())));
                }

                let result = match &identifier {
                    ResourceIdentifier::Key(key) => {
                        let key = key.bin()?;
                        let statement = format!(
                            "SELECT {} FROM resources as r WHERE key=?1",
                            RESOURCE_QUERY_FIELDS
                        );
                        let mut statement = self.conn.prepare(statement.as_str())?;
                        statement.query_row(params![key], |row| {
                            Ok(Self::process_resource_row_catch(row)?)
                        })
                    }
                    ResourceIdentifier::Address(address) => {
                        let address = address.to_string();
                        let statement = format!(
                            "SELECT {} FROM resources as r WHERE address=?1",
                            RESOURCE_QUERY_FIELDS
                        );
                        let mut statement = self.conn.prepare(statement.as_str())?;
                        statement.query_row(params![address], |row| {
                            Ok(Self::process_resource_row_catch(row)?)
                        })
                    }
                };

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
                            eprintln!(
                                "for {} SQL ERROR: {}",
                                identifier.to_string(),
                                err.to_string()
                            );
                            Err(err.into())
                        }
                    },
                }
            }

            ResourceRegistryCommand::Reserve(request) => {
                let trans = self.conn.transaction()?;
                trans.execute("DELETE FROM names WHERE key IS NULL AND datetime(reservation_timestamp) < datetime('now')", [] )?;
                let params = RegistryParams::new(
                    request.archetype.clone(),
                    Option::Some(request.parent.clone()),
                    Option::None,
                    Option::None,
                    Option::None,
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
                let log_info = StaticLogInfo::clone_info(Box::new(self));
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
                            ResourceRegistryCommand::Commit(registration),
                        );
                        action_tx.send(action).await;
                        rx.await;
                        result_tx.send(Ok(()));
                    } else if let Result::Err(error) = result {
                        logger::elog(
                            &log_info,
                            &request.archetype,
                            "Reserve()",
                            format!(
                                "ERROR: reservation failed to commit due to RecvErr: '{}'",
                                error.to_string()
                            )
                            .as_str(),
                        );
                    } else {
                        logger::elog(
                            &log_info,
                            &request.archetype,
                            "Reserve()",
                            "ERROR: reservation failed to commit.",
                        );
                    }
                });
                Ok(ResourceRegistryResult::Reservation(reservation))
            }

            ResourceRegistryCommand::Next { key, unique } => {
                let mut trans = self.conn.transaction()?;
                let key = key.bin()?;
                let column = match unique {
                    Unique::Sequence => "sequence",
                    Unique::Index => "id_index",
                };

                trans.execute(
                    "INSERT OR IGNORE INTO uniques (key) VALUES (?1)",
                    params![key],
                )?;
                trans.execute(
                    format!("UPDATE uniques SET {}={}+1 WHERE key=?1", column, column).as_str(),
                    params![key],
                )?;
                let rtn = trans.query_row(
                    format!("SELECT {} FROM uniques WHERE key=?1", column).as_str(),
                    params![key],
                    |r| {
                        let rtn: u64 = r.get(0)?;
                        Ok(rtn)
                    },
                )?;
                trans.commit()?;

                Ok(ResourceRegistryResult::Unique(rtn))
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
        let key: Vec<u8> = row.get(0)?;
        let key = ResourceKey::from_bin(key)?;

        let address: String = row.get(1)?;
        let address = ResourceAddress::from_str(address.as_str())?;

        let kind: String = row.get(2)?;
        let kind = ResourceKind::from_str(kind.as_str())?;

        let specific = if let ValueRef::Null = row.get_ref(3)? {
            Option::None
        } else {
            let specific: String = row.get(3)?;
            let specific = Specific::from_str(specific.as_str())?;
            Option::Some(specific)
        };

        let owner = if let ValueRef::Null = row.get_ref(4)? {
            Option::None
        } else {
            let owner: Vec<u8> = row.get(4)?;
            let owner: UserKey = UserKey::from_bin(owner)?;
            Option::Some(owner)
        };

        let config = if let ValueRef::Null = row.get_ref(5)? {
            Option::None
        } else {
            let config: String = row.get(5)?;
            let config = ConfigSrc::from_str(config.as_str())?;
            Option::Some(config)
        };

        let host: Vec<u8> = row.get(6)?;
        let host = StarKey::from_bin(host)?;

        let stub = ResourceStub {
            key: key,
            address: address,
            archetype: ResourceArchetype {
                kind: kind,
                specific: specific,
                config: config,
            },
            owner: owner,
        };

        let record = ResourceRecord {
            stub: stub,
            location: ResourceLocation {
                host: host,
                gathering: Option::None,
            },
        };

        Ok(record)
    }

    fn process_names(
        trans: &Transaction,
        names: &Names,
        params: &RegistryParams,
    ) -> Result<(), Error> {
        for name in names {
            trans.execute("INSERT INTO names (key,name,resource_type,kind,specific,parent,owner,config,reservation_timestamp) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,timestamp('now','+5 minutes')", params![params.key,name,params.resource_type,params.kind,params.specific,params.parent,params.owner,params.config])?;
        }
        Ok(())
    }

    pub fn setup(&mut self) -> Result<(), Error> {
        let labels = r#"
       CREATE TABLE IF NOT EXISTS labels (
	      key INTEGER PRIMARY KEY AUTOINCREMENT,
	      resource_key BLOB,
	      name TEXT NOT NULL,
	      value TEXT NOT NULL,
          UNIQUE(key,name),
          FOREIGN KEY (resource_key) REFERENCES resources (key)
        )"#;

        let names = r#"
       CREATE TABLE IF NOT EXISTS names(
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          key BLOB,
	      name TEXT NOT NULL,
	      resource_type TEXT NOT NULL,
          kind BLOB NOT NULL,
          specific TEXT,
          parent BLOB,
          app TEXT,
          owner BLOB,
          reservation_timestamp TEXT,
          UNIQUE(name,resource_type,kind,specific,parent)
        )"#;

        let resources = r#"CREATE TABLE IF NOT EXISTS resources (
         key BLOB PRIMARY KEY,
         address TEXT NOT NULL,
         resource_type TEXT NOT NULL,
         kind BLOB NOT NULL,
         specific TEXT,
         config TEXT,
         parent BLOB,
         owner BLOB,
         host BLOB
        )"#;

        let address_index = "CREATE UNIQUE INDEX resource_address_index ON resources(address)";

        let uniques = r#"CREATE TABLE IF NOT EXISTS uniques(
         key BLOB PRIMARY KEY,
         sequence INTEGER NOT NULL DEFAULT 0,
         id_index INTEGER NOT NULL DEFAULT 0
        )"#;

        let transaction = self.conn.transaction()?;
        transaction.execute(labels, [])?;
        transaction.execute(names, [])?;
        transaction.execute(resources, [])?;
        transaction.execute(uniques, [])?;
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

#[derive(Debug,Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum ResourceKind {
    Root,
    Space,
    SubSpace,
    App,
    Actor(ActorKind),
    User,
    FileSystem,
    File(FileKind),
    Domain,
    UrlPathPattern,
    Proxy(ProxyKind),
    ArtifactBundle(ArtifactBundleKind),
    Artifact,
    Database(DatabaseKind),
}

impl ResourceKind {
    pub fn resource_type(&self) -> ResourceType {
        match self {
            ResourceKind::Root => ResourceType::Root,
            ResourceKind::Space => ResourceType::Space,
            ResourceKind::SubSpace => ResourceType::SubSpace,
            ResourceKind::App => ResourceType::App,
            ResourceKind::Actor(_) => ResourceType::Actor,
            ResourceKind::User => ResourceType::User,
            ResourceKind::File(_) => ResourceType::File,
            ResourceKind::FileSystem => ResourceType::FileSystem,
            ResourceKind::Domain => ResourceType::Domain,
            ResourceKind::UrlPathPattern => ResourceType::UrlPathPattern,
            ResourceKind::Proxy(_) => ResourceType::Proxy,
            ResourceKind::ArtifactBundle(_) => ResourceType::ArtifactBundle,
            ResourceKind::Artifact => ResourceType::Artifact,
            ResourceKind::Database(_) => ResourceType::Database,
        }
    }

    pub fn init_clap_config(&self) -> Result<Option<ArtifactAddress>,Error>{
        match self{
            ResourceKind::Space => {
                Ok(Option::Some(ArtifactAddress::with_parent(&create_args::ARTIFACT_BUNDLE, "/init-args/space.yaml")?))
            }
            ResourceKind::SubSpace => {
                Ok(Option::Some(ArtifactAddress::with_parent(&create_args::ARTIFACT_BUNDLE, "/init-args/sub_space.yaml")?))
            }
            _ => {
                Ok(Option::None)
            }
        }
    }

    pub fn has_sub(&self)->bool{
        match self {
            ResourceKind::Actor(_) => true,
            ResourceKind::File(_) => true,
            ResourceKind::Proxy(_) => true,
            ResourceKind::ArtifactBundle(_) => true,
            ResourceKind::Database(_) => true,
            _ => {
                false
            }
        }
    }

    pub fn has_specific(&self)->bool{
        match self {

           ResourceKind::Database(_) => true,
            _ => {
                false
            }
        }
    }

    pub fn sub_string(&self) -> Option<String> {
        match self {
            ResourceKind::Actor(v) => Option::Some(v.to_string()),
            ResourceKind::File(v) => Option::Some(v.to_string()),
            ResourceKind::Proxy(v) =>Option::Some(v.to_string()),
            ResourceKind::ArtifactBundle(v) => Option::Some(v.to_string()),
            ResourceKind::Database(kind) => Option::Some(kind.to_string()),
            _ => Option::None
        }
    }
    pub fn specific(&self) -> Option<resource::address::Specific> {
        match self {
            ResourceKind::Database(kind) => Option::Some(kind.specific()),
            _ => Option::None
        }
    }
}



impl Into<ResourceKindParts> for ResourceKind {
    fn into(self) -> ResourceKindParts {

        let specific = self.specific();
        let sub_kind = self.sub_string();
        ResourceKindParts{
            resource_type: self.resource_type().to_string(),
            kind: sub_kind,
            specific: specific
        }
    }
}

impl ToString for ResourceKind {
    fn to_string(&self) -> String {
        if self.has_specific() {
            format!("<{}<{}<{}>>>", self.resource_type().to_string(), self.sub_string().expect("expected subtring"), self.specific().expect("expected specific").to_string())
        }
        else if self.has_sub() {
            format!("<{}<{}>>", self.resource_type().to_string(), self.sub_string().expect("expected subtring"))
        }
        else{
          format!("<{}>",self.resource_type().to_string())
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum ArtifactBundleKind {
    Volatile,
    Final,
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum DatabaseKind {
    Relational(resource::address::Specific),
}

impl DatabaseKind{
    pub fn specific(&self) -> resource::address::Specific {

        match self {
            DatabaseKind::Relational(specific) => {
                specific.clone()
            }
        }
    }
}


impl ToString for DatabaseKind {
    fn to_string(&self) -> String {
        match self {
            DatabaseKind::Relational(specific) => format!("<Database<Relational<{}>>>",specific.to_string())
        }
    }
}

impl From<semver::Version> for ArtifactBundleKind {
    fn from(value: semver::Version) -> Self {
        match value.is_prerelease() {
            true => ArtifactBundleKind::Volatile,
            false => ArtifactBundleKind::Final,
        }
    }
}

impl TryFrom<ResourceAddress> for ArtifactBundleKind {
    type Error = Fail;

    fn try_from(address: ResourceAddress) -> Result<Self, Self::Error> {
        let address = match address.resource_type() {
            ResourceType::ArtifactBundle => address,
            ResourceType::Artifact => address
                .parent()
                .ok_or("expected artifact resource address to have a parent")?,
            got => {
                return Err(Fail::WrongResourceType {
                    expected: HashSet::from_iter(vec![
                        ResourceType::ArtifactBundle,
                        ResourceType::Artifact,
                    ]),
                    received: got,
                })
            }
        };
        let version = semver::Version::from_str(address.last_to_string()?.as_str())?;

        match version.is_prerelease() {
            true => Ok(ArtifactBundleKind::Volatile),
            false => Ok(ArtifactBundleKind::Final),
        }
    }
}

impl FromStr for ArtifactBundleKind {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Volatile" => Ok(ArtifactBundleKind::Volatile),
            "Final" => Ok(ArtifactBundleKind::Final),
            _ => Err(format!("cannot match ArtifactBundleKind: {}", s).into()),
        }
    }
}

impl ToString for ArtifactBundleKind {
    fn to_string(&self) -> String {
        match self {
            Self::Volatile => "Volatile".to_string(),
            Self::Final => "Final".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum FileKind {
    File,
    Directory,
}

impl FromStr for FileKind {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Directory" => Ok(FileKind::Directory),
            "File" => Ok(FileKind::File),
            _ => Err(format!("cannot match FileKind: {}", s).into()),
        }
    }
}

impl ToString for FileKind {
    fn to_string(&self) -> String {
        match self {
            FileKind::File => "File".to_string(),
            FileKind::Directory => "Directory".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum ProxyKind {
    Http,
}

impl ToString for ProxyKind {
    fn to_string(&self) -> String {
        match self {
            ProxyKind::Http => "Http".to_string(),
        }
    }
}

impl ResourceType {
    pub fn magic(&self) -> u8 {
        match self {
            ResourceType::Root => 255,
            ResourceType::Space => 0,
            ResourceType::SubSpace => 1,
            ResourceType::App => 2,
            ResourceType::Actor => 3,
            ResourceType::User => 4,
            ResourceType::File => 5,
            ResourceType::FileSystem => 6,
            ResourceType::Domain => 7,
            ResourceType::UrlPathPattern => 8,
            ResourceType::Proxy => 9,
            ResourceType::ArtifactBundle => 10,
            ResourceType::Artifact => 11,
            ResourceType::Database => 12,
        }
    }

    pub fn from_magic(magic: u8) -> Result<Self, Error> {
        match magic {
            0 => Ok(ResourceType::Space),
            1 => Ok(ResourceType::SubSpace),
            2 => Ok(ResourceType::App),
            3 => Ok(ResourceType::Actor),
            4 => Ok(ResourceType::User),
            5 => Ok(ResourceType::File),
            6 => Ok(ResourceType::FileSystem),
            7 => Ok(ResourceType::Domain),
            8 => Ok(ResourceType::UrlPathPattern),
            9 => Ok(ResourceType::Proxy),
            10 => Ok(ResourceType::ArtifactBundle),
            11 => Ok(ResourceType::Artifact),
            12 => Ok(ResourceType::Database),
            255 => Ok(ResourceType::Root),
            _ => Err(format!("no resource type for magic number {}", magic).into()),
        }
    }
}


/*
impl fmt::Display for ResourceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ResourceKind::Root => "Nothing".to_string(),
                ResourceKind::Space => "Space".to_string(),
                ResourceKind::SubSpace => "SubSpace".to_string(),
                ResourceKind::App => "App".to_string(),
                ResourceKind::Actor(kind) => format!("Actor::{}", kind).to_string(),
                ResourceKind::User => "User".to_string(),
                ResourceKind::File(kind) => format!("File::{}", kind.to_string()).to_string(),
                ResourceKind::FileSystem => format!("Filesystem").to_string(),
                ResourceKind::Domain => "Domain".to_string(),
                ResourceKind::UrlPathPattern => "UrlPathPattern".to_string(),
                ResourceKind::Proxy(kind) => format!("Proxy::{}", kind.to_string()).to_string(),
                ResourceKind::ArtifactBundle(kind) =>
                    format!("ArtifactBundle::{}", kind.to_string()).to_string(),
                ResourceKind::Artifact => "Artifact".to_string(),
                ResourceKind::Database(kind) =>
                    format!("Database::{}", kind.to_string()).to_string(),
            }
        )
    }
}

 */

impl ResourceKind {
    pub fn test_key(&self, sub_space: SubSpaceKey, index: usize) -> ResourceKey {
        match self {
            ResourceKind::Root => ResourceKey::Root,
            ResourceKind::Space => ResourceKey::Space(SpaceKey::from_index(index as u32)),
            ResourceKind::SubSpace => {
                ResourceKey::SubSpace(SubSpaceKey::new(sub_space.space, index as _))
            }
            ResourceKind::App => ResourceKey::App(AppKey::new(sub_space, index as _)),
            ResourceKind::Actor(_) => {
                let app = AppKey::new(sub_space, index as _);
                ResourceKey::Actor(ActorKey::new(app, Id::new(0, index as _)))
            }
            ResourceKind::User => ResourceKey::User(UserKey::new(sub_space.space, index as _)),
            ResourceKind::File(_) => ResourceKey::File(FileKey {
                filesystem: FileSystemKey::SubSpace(SubSpaceFilesystemKey { sub_space, id: 0 }),
                id: index as _,
            }),

            ResourceKind::FileSystem => {
                ResourceKey::FileSystem(FileSystemKey::SubSpace(SubSpaceFilesystemKey {
                    sub_space: sub_space,
                    id: index as _,
                }))
            }
            _ => {
                unimplemented!()
            }
        }
    }
}

impl TryFrom<ResourceKindParts> for ResourceKind {
    type Error = Error;

    fn try_from(parts: ResourceKindParts) -> Result<Self,Self::Error> {
        let resource_type = ResourceType::from_str(parts.resource_type.as_str() )?;
        Ok(match resource_type{
            ResourceType::Root => ResourceKind::Root,
            ResourceType::Space => ResourceKind::Space,
            ResourceType::SubSpace => ResourceKind::SubSpace,
            ResourceType::App => ResourceKind::App,
            ResourceType::Actor => {
                ResourceKind::Actor(ActorKind::from_str(parts.kind.ok_or("expected Actor to have Kind <Statless|Stateful>")?.as_str() )?)
            }
            ResourceType::User => ResourceKind::User,
            ResourceType::FileSystem => ResourceKind::FileSystem,
            ResourceType::File => {
                ResourceKind::File(FileKind::from_str(parts.kind.ok_or("expected File to have Kind <Directory|File>")?.as_str() )?)
            }
            ResourceType::Domain => ResourceKind::Domain,
            ResourceType::UrlPathPattern => ResourceKind::UrlPathPattern,
            ResourceType::Proxy => {
                let kind = match parts.kind.ok_or( "expected Proxy to have Kind <Http>")?.as_str()
                {
                    "Http" => {
                        ProxyKind::Http
                    }
                    kind => {
                        return Err(format!("could not find proxy kind matching {}",kind).into());
                    }
                };
                ResourceKind::Proxy(kind)
            }
            ResourceType::ArtifactBundle => {
                let kind = match parts.kind.ok_or( "expected ArtifactBundle to have Kind <Final|Volatile>")?.as_str()
                {
                    "Final" => {
                        ArtifactBundleKind::Final
                    }
                    "Volatile" => {
                        ArtifactBundleKind::Volatile
                    }
                    kind => {
                        return Err(format!("could not find ArtifactBundle kind matching {}",kind).into());
                    }
                };
                ResourceKind::ArtifactBundle(kind)
            },
            ResourceType::Artifact => ResourceKind::Artifact,
            ResourceType::Database => {
                let kind = match parts.kind.ok_or( "expected Database to have Kind <Relational<specific>>?")?.as_str()
                {
                    "Relational" => {
                        DatabaseKind::Relational(parts.specific.ok_or("expected DatabaseKind::Relational to have a Specific <Database<Relational<vendor:product:variant:version>>>")?)
                    }
                    kind => {
                        return Err(format!("could not find database kind matching {}",kind).into());
                    }
                };
                ResourceKind::Database(kind)
            }
        })
    }
}

impl FromStr for ResourceKind {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts = ResourceKindParts::from_str(s)?;
        Self::try_from(parts)
    }
}

/*
impl FromStr for ResourceKind {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.contains("::") {
            let mut split = s.split("::");
            match split.next().ok_or("error")? {
                "File" => {
                    return Ok(ResourceKind::File(FileKind::from_str(
                        split.next().ok_or("error")?,
                    )?));
                }
                "ArtifactBundle" => {
                    return Ok(ResourceKind::ArtifactBundle(ArtifactBundleKind::from_str(
                        split.next().ok_or("error")?,
                    )?));
                }
                _ => {
                    return Err(format!("cannot find a match for {}", s).into());
                }
            }
        }
        match s {
            "Nothing" => Ok(ResourceKind::Root),
            "Space" => Ok(ResourceKind::Space),
            "SubSpace" => Ok(ResourceKind::SubSpace),
            "User" => Ok(ResourceKind::User),
            "Filesystem" => Ok(ResourceKind::FileSystem),
            "Artifact" => Ok(ResourceKind::Artifact),
            "App" => Ok(ResourceKind::App),
            "Domain" => Ok(ResourceKind::Domain),
            _ => Err(format!("cannot match ResourceKind: {}", s).into()),
        }
    }
}

 */

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum ResourceType {
    Root,
    Space,
    SubSpace,
    App,
    Actor,
    User,
    FileSystem,
    File,
    Domain,
    UrlPathPattern,
    Proxy,
    ArtifactBundle,
    Artifact,
    Database,
}

impl ResourceType {
    pub fn requires_owner(&self) -> bool {
        /*        match self{
            Self::Nothing => false,
            Self::Space => false,
            Self::SubSpace => true,
            Self::App => true,
            Self::Actor => true,
            Self::User => false,
            Self::FileSystem => true,
            Self::File => true,
            Self::Artifact => true
        }*/
        // for now let's not worry about owners
        false
    }

    pub fn default_kind(&self) -> Result<ResourceKind,Error>{
        match self{
            ResourceType::Root => Ok(ResourceKind::Root),
            ResourceType::Space => Ok(ResourceKind::Space),
            ResourceType::SubSpace => Ok(ResourceKind::SubSpace),
            ResourceType::App => Ok(ResourceKind::App),
            ResourceType::User => Ok(ResourceKind::User),
            ResourceType::FileSystem => Ok(ResourceKind::FileSystem),
            ResourceType::Domain => Ok(ResourceKind::Domain),
            ResourceType::UrlPathPattern => Ok(ResourceKind::UrlPathPattern),
            ResourceType::Artifact => Ok(ResourceKind::Artifact),
            _ => {
                Err(format!("no default kind for resource: {}",self.to_string()).into())
            }
        }
    }

    pub fn state_persistence(&self) -> ResourceStatePersistenceManager {
        match self {
            ResourceType::File => ResourceStatePersistenceManager::Host,
            _ => ResourceStatePersistenceManager::Store,
        }
    }

    pub fn from(parent: &ResourceKey) -> Option<Self> {
        match parent {
            ResourceKey::Root => Option::None,
            parent => Option::Some(parent.resource_type()),
        }
    }

    pub fn hash_to_string(set: &HashSet<ResourceType>) -> String {
        let mut string = String::new();
        for (index, resource_type) in set.iter().enumerate() {
            if index > 0 {
                string.push_str(",");
            }
            string.push_str(resource_type.to_string().as_str());
        }
        string
    }
}

impl ResourceType {
    pub fn append_address(
        &self,
        parent: ResourceAddress,
        tail: String,
    ) -> Result<ResourceAddress, Error> {
        match self {
            ResourceType::FileSystem => match parent.resource_type() {
                ResourceType::SubSpace => {
                    let address = format!(
                        "{}{}*{}{}",
                        parent.to_parts_string(),
                        RESOURCE_ADDRESS_DELIM,
                        RESOURCE_ADDRESS_DELIM,
                        tail
                    );
                    Ok(self.address_structure().from_str(address.as_str())?)
                }
                ResourceType::App => {
                    let address = format!(
                        "{}{}{}",
                        parent.to_parts_string(),
                        RESOURCE_ADDRESS_DELIM,
                        tail
                    );
                    Ok(self.address_structure().from_str(address.as_str())?)
                }
                resource_type => Err(format!(
                    "illegal resource type parent for FileSystem: {}",
                    resource_type.to_string()
                )
                .into()),
            },
            _ => {
                let address = format!(
                    "{}{}{}",
                    parent.to_parts_string(),
                    RESOURCE_ADDRESS_DELIM,
                    tail
                );
                Ok(self.address_structure().from_str(address.as_str())?)
            }
        }
    }

    pub fn star_host(&self) -> StarKind {
        match self {
            ResourceType::Root => StarKind::Central,
            ResourceType::Space => StarKind::SpaceHost,
            ResourceType::SubSpace => StarKind::SpaceHost,
            ResourceType::App => StarKind::AppHost,
            ResourceType::Actor => StarKind::ActorHost,
            ResourceType::User => StarKind::SpaceHost,
            ResourceType::FileSystem => StarKind::FileStore,
            ResourceType::File => StarKind::FileStore,
            ResourceType::UrlPathPattern => StarKind::SpaceHost,
            ResourceType::Proxy => StarKind::SpaceHost,
            ResourceType::Domain => StarKind::SpaceHost,
            ResourceType::ArtifactBundle => StarKind::ArtifactStore,
            ResourceType::Artifact => StarKind::ArtifactStore,
            ResourceType::Database => StarKind::Kube,
        }
    }

    pub fn star_manager(&self) -> HashSet<StarKind> {
        match self {
            ResourceType::Root => HashSet::from_iter(vec![StarKind::Central]),
            ResourceType::Space => HashSet::from_iter(vec![StarKind::Central]),
            ResourceType::SubSpace => HashSet::from_iter(vec![StarKind::SpaceHost]),
            ResourceType::App => HashSet::from_iter(vec![StarKind::SpaceHost]),
            ResourceType::Actor => HashSet::from_iter(vec![StarKind::AppHost]),
            ResourceType::User => HashSet::from_iter(vec![StarKind::SpaceHost]),
            ResourceType::FileSystem => {
                HashSet::from_iter(vec![StarKind::SpaceHost, StarKind::AppHost])
            }
            ResourceType::File => HashSet::from_iter(vec![StarKind::FileStore]),
            ResourceType::UrlPathPattern => HashSet::from_iter(vec![StarKind::SpaceHost]),
            ResourceType::Proxy => HashSet::from_iter(vec![StarKind::SpaceHost]),
            ResourceType::Domain => HashSet::from_iter(vec![StarKind::SpaceHost]),
            ResourceType::ArtifactBundle => HashSet::from_iter(vec![StarKind::SpaceHost]),
            ResourceType::Artifact => HashSet::from_iter(vec![StarKind::FileStore]),
            ResourceType::Database => HashSet::from_iter(vec![StarKind::Kube]),
        }
    }
}

impl ResourceType {
    pub fn children(&self) -> HashSet<ResourceType> {
        let mut children = match self {
            Self::Root => vec![Self::Space],
            Self::Space => vec![Self::SubSpace, Self::User, Self::Domain, Self::Proxy],
            Self::SubSpace => vec![Self::App, Self::FileSystem],
            Self::App => vec![Self::Actor, Self::FileSystem],
            Self::Actor => vec![],
            Self::User => vec![],
            Self::FileSystem => vec![Self::File],
            Self::File => vec![],
            Self::UrlPathPattern => vec![],
            Self::Proxy => vec![],
            Self::Domain => vec![Self::UrlPathPattern],
            Self::ArtifactBundle => vec![Self::Artifact],
            Self::Artifact => vec![],
            Self::Database => vec![],
        };

        HashSet::from_iter(children.drain(..))
    }

    pub fn supports_automatic_key_generation(&self) -> bool {
        match self {
            ResourceType::Root => false,
            ResourceType::Space => false,
            ResourceType::SubSpace => false,
            ResourceType::App => true,
            ResourceType::Actor => false,
            ResourceType::User => true,
            ResourceType::FileSystem => false,
            ResourceType::File => false,
            ResourceType::UrlPathPattern => false,
            ResourceType::Proxy => false,
            ResourceType::Domain => false,
            ResourceType::ArtifactBundle => false,
            ResourceType::Artifact => false,
            ResourceType::Database => false,
        }
    }
}

impl ToString for ResourceType {
    fn to_string(&self) -> String {
        match self {
            Self::Root => "Nothing".to_string(),
            Self::Space => "Space".to_string(),
            Self::SubSpace => "SubSpace".to_string(),
            Self::App => "App".to_string(),
            Self::Actor => "Actor".to_string(),
            Self::User => "User".to_string(),
            Self::FileSystem => "FileSystem".to_string(),
            Self::File => "File".to_string(),
            Self::UrlPathPattern => "UrlPathPattern".to_string(),
            Self::Proxy => "Proxy".to_string(),
            Self::Domain => "Domain".to_string(),
            Self::ArtifactBundle => "ArtifactBundle".to_string(),
            Self::Artifact => "Artifact".to_string(),
            Self::Database => "Database".to_string(),
        }
    }
}

impl FromStr for ResourceType {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Nothing" => Ok(ResourceType::Root),
            "Space" => Ok(ResourceType::Space),
            "SubSpace" => Ok(ResourceType::SubSpace),
            "App" => Ok(ResourceType::App),
            "Actor" => Ok(ResourceType::Actor),
            "User" => Ok(ResourceType::User),
            "FileSystem" => Ok(ResourceType::FileSystem),
            "File" => Ok(ResourceType::File),
            "UrlPathPattern" => Ok(ResourceType::UrlPathPattern),
            "Proxy" => Ok(ResourceType::Proxy),
            "Domain" => Ok(ResourceType::Domain),
            "ArtifactBundle" => Ok(ResourceType::ArtifactBundle),
            "Artifact" => Ok(ResourceType::Artifact),
            "Database" => Ok(ResourceType::Database),
            what => Err(format!("could not find resource type {}", what).into()),
        }
    }
}

#[derive(Eq, PartialEq)]
pub enum ResourceParent {
    None,
    Some(ResourceType),
    Multi(Vec<ResourceType>),
}

impl ResourceParent {
    pub fn matches_address(&self, address: Option<&ResourceAddress>) -> bool {
        match address {
            None => self.matches(Option::None),
            Some(address) => self.matches(Option::Some(&address.resource_type())),
        }
    }

    pub fn matches(&self, resource_type: Option<&ResourceType>) -> bool {
        match resource_type {
            None => *self == Self::None,
            Some(resource_type) => match self {
                ResourceParent::None => false,
                ResourceParent::Some(parent_type) => *parent_type == *resource_type,
                ResourceParent::Multi(multi) => multi.contains(resource_type),
            },
        }
    }

    pub fn types(&self) -> Vec<ResourceType> {
        match self {
            ResourceParent::None => vec![],
            ResourceParent::Some(some) => vec![some.to_owned()],
            ResourceParent::Multi(multi) => multi.to_owned(),
        }
    }
}

impl ToString for ResourceParent {
    fn to_string(&self) -> String {
        match self {
            ResourceParent::None => "None".to_string(),
            ResourceParent::Some(parent) => parent.to_string(),
            ResourceParent::Multi(_) => "Multi".to_string(),
        }
    }
}

impl ResourceType {
    pub fn parent(&self) -> ResourceParent {
        match self {
            ResourceType::Root => ResourceParent::None,
            ResourceType::Space => ResourceParent::Some(ResourceType::Root),
            ResourceType::SubSpace => ResourceParent::Some(ResourceType::Space),
            ResourceType::App => ResourceParent::Some(ResourceType::SubSpace),
            ResourceType::Actor => ResourceParent::Some(ResourceType::App),
            ResourceType::User => ResourceParent::Some(ResourceType::Space),
            ResourceType::File => ResourceParent::Some(ResourceType::FileSystem),
            ResourceType::FileSystem => {
                ResourceParent::Multi(vec![ResourceType::SubSpace, ResourceType::App])
            }
            ResourceType::UrlPathPattern => ResourceParent::Some(ResourceType::Domain),
            ResourceType::Proxy => ResourceParent::Some(ResourceType::Space),
            ResourceType::Domain => ResourceParent::Some(ResourceType::Space),
            ResourceType::ArtifactBundle => ResourceParent::Some(ResourceType::SubSpace),
            ResourceType::Artifact => ResourceParent::Some(ResourceType::ArtifactBundle),
            ResourceType::Database => {
                ResourceParent::Multi(vec![ResourceType::SubSpace, ResourceType::App])
            }
        }
    }

    pub fn is_specific_required(&self) -> bool {
        match self {
            ResourceType::Root => false,
            ResourceType::Space => false,
            ResourceType::SubSpace => false,
            ResourceType::App => true,
            ResourceType::Actor => true,
            ResourceType::User => false,
            ResourceType::File => false,
            ResourceType::FileSystem => false,
            ResourceType::UrlPathPattern => true,
            ResourceType::Proxy => true,
            ResourceType::Domain => true,
            ResourceType::ArtifactBundle => false,
            ResourceType::Artifact => false,
            ResourceType::Database => false,
        }
    }

    pub fn is_config_required(&self) -> bool {
        match self {
            ResourceType::Root => false,
            ResourceType::Space => false,
            ResourceType::SubSpace => false,
            ResourceType::App => true,
            ResourceType::Actor => true,
            ResourceType::User => false,
            ResourceType::File => false,
            ResourceType::FileSystem => false,
            ResourceType::UrlPathPattern => true,
            ResourceType::Proxy => true,
            ResourceType::Domain => true,
            ResourceType::ArtifactBundle => false,
            ResourceType::Artifact => false,
            ResourceType::Database => false,
        }
    }

    pub fn has_state(&self) -> bool {
        match self {
            ResourceType::Root => false,
            ResourceType::Space => false,
            ResourceType::SubSpace => false,
            ResourceType::App => false,
            ResourceType::Actor => true,
            ResourceType::User => false,
            ResourceType::File => true,
            ResourceType::FileSystem => false,
            ResourceType::UrlPathPattern => false,
            ResourceType::Proxy => false,
            ResourceType::Domain => false,
            ResourceType::ArtifactBundle => true,
            ResourceType::Artifact => true,
            ResourceType::Database => true,
        }
    }

    pub fn address_required(&self) -> bool {
        match self {
            ResourceType::Root => false,
            ResourceType::Space => true,
            ResourceType::SubSpace => true,
            ResourceType::App => false,
            ResourceType::Actor => false,
            ResourceType::User => false,
            ResourceType::File => true,
            ResourceType::FileSystem => true,
            ResourceType::UrlPathPattern => true,
            ResourceType::Proxy => true,
            ResourceType::Domain => true,
            ResourceType::ArtifactBundle => true,
            ResourceType::Artifact => true,
            ResourceType::Database => false,
        }
    }

    pub fn address_structure(&self) -> &ResourceAddressStructure {
        match self {
            ResourceType::Root => &ROOT_STRUCT,
            ResourceType::Space => &SPACE_ADDRESS_STRUCT,
            ResourceType::SubSpace => &SUB_SPACE_ADDRESS_STRUCT,
            ResourceType::App => &APP_ADDRESS_STRUCT,
            ResourceType::Actor => &ACTOR_ADDRESS_STRUCT,
            ResourceType::User => &USER_ADDRESS_STRUCT,
            ResourceType::FileSystem => &FILE_SYSTEM_ADDRESS_STRUCT,
            ResourceType::File => &FILE_ADDRESS_STRUCT,
            ResourceType::UrlPathPattern => &URL_ADDRESS_STRUCT,
            ResourceType::Proxy => &PROXY_ADDRESS_STRUCT,
            ResourceType::Domain => &DOMAIN_ADDRESS_STRUCT,
            ResourceType::ArtifactBundle => &ARTIFACT_BUNDLE_ADDRESS_STRUCT,
            ResourceType::Artifact => &ARTIFACT_ADDRESS_STRUCT,
            ResourceType::Database => &DATABASE_ADDRESS_STRUCT,
        }
    }
}

#[derive(Debug,Clone, Serialize, Deserialize)]
pub struct ResourceArchetype {
    pub kind: ResourceKind,
    pub specific: Option<Specific>,
    pub config: Option<ConfigSrc>,
}

impl ResourceArchetype {

    pub fn from_resource_type( kind: ResourceKind ) -> Self {
        ResourceArchetype {
            kind: kind,
            specific: Option::None,
            config: Option::None,
        }
    }


    pub fn root() -> ResourceArchetype {
        ResourceArchetype {
            kind: ResourceKind::Root,
            specific: Option::None,
            config: Option::None,
        }
    }

    pub fn valid(&self) -> Result<(), Fail> {
        if self.kind.resource_type() == ResourceType::Root {
            return Err(Fail::CannotCreateNothingResourceTypeItIsThereAsAPlaceholderDummy);
        }
        Ok(())
    }
}

impl LogInfo for ResourceArchetype {
    fn log_identifier(&self) -> String {
        "?".to_string()
    }

    fn log_kind(&self) -> String {
        self.kind.to_string()
    }

    fn log_object(&self) -> String {
        "ResourceArchetype".to_string()
    }
}

#[async_trait]
pub trait ResourceIdSeq: Send + Sync {
    async fn next(&self) -> ResourceId;
}

#[async_trait]
pub trait HostedResource: Send + Sync {
    fn key(&self) -> ResourceKey;
}

#[derive(Clone)]
pub struct HostedResourceStore {
    map: AsyncHashMap<ResourceKey, Arc<LocalHostedResource>>,
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

    pub async fn get(&self, key: ResourceKey) -> Result<Option<Arc<LocalHostedResource>>, Error> {
        self.map.get(key).await
    }

    pub async fn remove(
        &self,
        key: ResourceKey,
    ) -> Result<Option<Arc<LocalHostedResource>>, Error> {
        self.map.remove(key).await
    }

    pub async fn contains(&self, key: &ResourceKey) -> Result<bool, Error> {
        self.map.contains(key.clone()).await
    }
}

#[derive(Clone)]
pub struct RemoteHostedResource {
    key: ResourceKey,
    star_host: StarKey,
    local_skel: StarSkel,
}

pub struct LocalHostedResource {
    //    pub manager: Arc<dyn ResourceManager>,
    pub unique_src: Box<dyn UniqueSrc>,
    pub resource: ResourceStub,
}
impl HostedResource for LocalHostedResource {
    fn key(&self) -> ResourceKey {
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
    pub key: ResourceKey,
}

impl RemoteResourceManager {
    pub fn new(key: ResourceKey) -> Self {
        RemoteResourceManager { key: key }
    }
}

#[async_trait]
impl ResourceManager for RemoteResourceManager {
    async fn create(&self, create: ResourceCreate) -> Receiver<Result<ResourceRecord, Fail>> {
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
        f.debug_tuple("ParentCore").field(&self.skel).field(&self.stub).finish()
    }
}

pub struct Parent {
    pub core: ParentCore,
}

impl Parent {
    #[instrument]
    async fn create_child(
        core: ParentCore,
        create: ResourceCreate,
        tx: oneshot::Sender<Result<ResourceRecord, Fail>>,
    ) {

        let parent = match create.parent.clone().key_or("expected create.parent to already be a key") {
            Ok(key) => {key}
            Err(error) => {
                tx.send(Err(Fail::from(error)));
                return;
            }
        };

        if let Ok(reservation) = core
            .child_registry
            .reserve(ResourceNamesReservationRequest {
                parent: parent,
                archetype: create.archetype.clone(),
                info: create.registry_info.clone(),
            })
            .await
        {
            let mut rx =
                ResourceCreationChamber::new(core.stub.clone(), create.clone(), core.skel.clone())
                    .await;

            tokio::spawn(async move {
                match Self::process_create(core.clone(), create.clone(), reservation, rx).await {
                    Ok(resource) => {
                        tx.send(Ok(resource));
                    }
                    Err(fail) => {
                        error!("Failed to create child: FAIL: {}",fail.to_string());
                        tx.send(Err(fail));
                    }
                }
            });
        } else {
            elog(
                &core,
                &create,
                "create_child()",
                "ERROR: reservation failed.",
            );
            tx.send(Err("RESERVATION FAILED!".into()));
        }
    }

    async fn process_create(
        core: ParentCore,
        create: ResourceCreate,
        reservation: RegistryReservation,
        rx: oneshot::Receiver<Result<ResourceAssign<AssignResourceStateSrc>, Fail>>,
    ) -> Result<ResourceRecord, Fail> {
        let assign = rx.await??;
        let mut host = core
            .selector
            .select(create.archetype.kind.resource_type())
            .await?;
        let record = ResourceRecord::new(assign.stub.clone(), host.star_key());
        host.assign(assign).await?;
        let (commit_tx, commit_rx) = oneshot::channel();
        reservation.commit(record.clone(), commit_tx)?;
        Ok(record)
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
                ResourceKey::new(core.key.clone(), ResourceId::new(&create.archetype.kind.resource_type(), core.id_seq.next() ) )?
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
        create: ResourceCreate,
    ) -> oneshot::Receiver<Result<ResourceRecord, Fail>> {
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
    create: ResourceCreate,
    skel: StarSkel,
    tx: oneshot::Sender<Result<ResourceAssign<AssignResourceStateSrc>, Fail>>,
}

impl ResourceCreationChamber {
    pub async fn new(
        parent: ResourceStub,
        create: ResourceCreate,
        skel: StarSkel,
    ) -> oneshot::Receiver<Result<ResourceAssign<AssignResourceStateSrc>, Fail>> {


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

            if !self.create.parent.is_key() {
                self.tx.send( Err(Fail::Error("ResourceCreationChamber requires keyed ResourceCreate object.  Call ResourceCreate::to_keyed(starlane_api) to modify".to_string())) );
                return;
            }

            if !self
                .create
                .archetype
                .kind
                .resource_type()
                .parent()
                .matches(Option::Some(&self.parent.key.resource_type()))
            {
                println!("!!! -> Throwing Fail::WrongParentResourceType <- !!!");
                self.tx.send(Err(Fail::WrongParentResourceType {
                    expected: HashSet::from_iter(self.parent.key.resource_type().parent().types()),
                    received: Option::Some(self.create.parent.resource_type()),
                }));
                return;
            };

            match self.create.validate() {
                Ok(_) => {}
                Err(error) => {
                    self.tx.send(Err(error));
                    return;
                }
            }

            let key = match &self.create.key {
                KeyCreationSrc::None => {
                    let mut proto = ProtoMessage::new();
                    proto.to(self.parent.key.clone().into());
                    proto.from(MessageFrom::Resource(self.parent.key.clone().into()));
                    proto.payload = Option::Some(ResourceRequestMessage::Unique(
                        self.create.archetype.kind.resource_type(),
                    ));

                    let mut rx: Receiver<Result<MessageReply<ResourceResponseMessage>, Fail>> =
                        proto.reply();

                    let proto_star_message = match proto.to_proto_star_message().await {
                        Ok(proto_star_message) => proto_star_message,
                        Err(error) => {
                            eprintln!(
                                "ERROR when process proto_star_message from ProtoMessage: {}",
                                error
                            );
                            return;
                        }
                    };

                    self.skel
                        .star_tx
                        .send(StarCommand::SendProtoMessage(proto_star_message))
                        .await;

                    tokio::spawn(async move {
                        if let Ok(Ok(MessageReply {
                            id: _,
                            from: _,
                            reply_to: _,
                            payload: ResourceResponseMessage::Unique(id),
                            trace,
                            log,
                        })) = util::wait_for_it_whatever(rx).await
                        {
                            match ResourceKey::new(self.parent.key.clone(), id.clone()) {
                                Ok(key) => {
                                    let final_create = self.finalize_create(key.clone()).await;
                                    self.tx.send(final_create);
                                    return;
                                }
                                Err(error) => {
                                    self.tx.send(Err(format!(
                                        "error when trying to create resource key with id {}",
                                        id.to_string()
                                    )
                                    .into()));
                                    return;
                                }
                            }
                        } else {
                            self.tx.send(Err(
                                "unexpected response, expected ResourceResponse::Unique".into(),
                            ));
                            return;
                        }
                    });
                }
                KeyCreationSrc::Key(key) => {
                    if key.parent() != Option::Some(self.parent.key.clone()) {
                        let final_create = self.finalize_create(key.clone()).await;
                        self.tx.send(final_create);
                        return;
                    }
                }
            };
        });
    }

    async fn finalize_create(
        &self,
        key: ResourceKey,
    ) -> Result<ResourceAssign<AssignResourceStateSrc>, Fail> {
        let address = match &self.create.address {
            AddressCreationSrc::None => {
                let address = format!(
                    "{}:{}",
                    self.parent.address.to_parts_string(),
                    key.generate_address_tail()?
                );
                self.create
                    .archetype
                    .kind
                    .resource_type()
                    .address_structure()
                    .from_str(address.as_str())?
            }
            AddressCreationSrc::Append(tail) => self
                .create
                .archetype
                .kind
                .resource_type()
                .append_address(self.parent.address.clone(), tail.clone())?,
            AddressCreationSrc::Appends(tails) => {
                let mut address = self.parent.address.to_parts_string();
                for tail in tails {
                    address.push_str(":");
                    address.push_str(tail.as_str());
                }

                address.push_str("::<");
                address.push_str(key.resource_type().to_string().as_str());
                address.push_str(">");

                ResourceAddress::from_str(address.as_str())?
            }
            AddressCreationSrc::Space(space_name) => {
                if self.parent.key.resource_type() != ResourceType::Root {
                    return Err(format!(
                        "Space creation can only be used at top level (Nothing) not by {}",
                        self.parent.key.resource_type().to_string()
                    )
                    .into());
                }
                ResourceAddress::for_space(space_name.as_str())?
            }

            AddressCreationSrc::Exact(address) => {
                address.clone()
            }
        };

        let stub = ResourceStub {
            key: key,
            address: address.clone(),
            archetype: self.create.archetype.clone(),
            owner: None,
        };

        let assign = ResourceAssign {
            stub: stub,
            state_src: self.create.src.clone(),
        };
        Ok(assign)
    }
}

#[async_trait]
pub trait ResourceHost: Send + Sync {
    fn star_key(&self) -> StarKey;
    async fn assign(&self, assign: ResourceAssign<AssignResourceStateSrc>) -> Result<(), Fail>;
}

impl From<ActorKind> for ResourceKind {
    fn from(e: ActorKind) -> Self {
        ResourceKind::Actor(e)
    }
}

#[derive(Debug,Clone, Serialize, Deserialize)]
pub struct ResourceRegistryInfo {
    pub names: Names,
    pub labels: Labels,
}

impl ResourceRegistryInfo {
    pub fn new() -> Self {
        ResourceRegistryInfo {
            names: Names::new(),
            labels: Labels::new(),
        }
    }
}

pub struct ResourceNamesReservationRequest {
    pub info: Option<ResourceRegistryInfo>,
    pub parent: ResourceKey,
    pub archetype: ResourceArchetype,
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
            tx.send((record, result_tx)).or(Err(Fail::Error("could not send to tx".to_string())));
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

pub struct RegistryUniqueSrc {
    parent_key: ResourceIdentifier,
    tx: mpsc::Sender<ResourceRegistryAction>,
}

impl RegistryUniqueSrc {
    pub fn new(parent_key: ResourceIdentifier, tx: mpsc::Sender<ResourceRegistryAction>) -> Self {
        RegistryUniqueSrc {
            parent_key: parent_key,
            tx: tx,
        }
    }
}

#[async_trait]
impl UniqueSrc for RegistryUniqueSrc {
    async fn next(&self, resource_type: &ResourceType) -> Result<ResourceId, Fail> {
        if !resource_type
            .parent()
            .matches(Option::Some(&self.parent_key.resource_type()))
        {
            eprintln!("WRONG RESOURCE TYPE IN UNIQUE SRC");
            return Err(Fail::WrongResourceType {
                expected: HashSet::from_iter(self.parent_key.resource_type().children()),
                received: resource_type.clone(),
            });
        }
        let (tx, rx) = oneshot::channel();

        let parent_key = match &self.parent_key {
            ResourceIdentifier::Key(key) => key.clone(),
            ResourceIdentifier::Address(address) => {
                let (tx, rx) = oneshot::channel();
                self.tx
                    .send(ResourceRegistryAction {
                        tx: tx,
                        command: ResourceRegistryCommand::Get(address.clone().into()),
                    })
                    .await?;
                if let ResourceRegistryResult::Resource(Option::Some(record)) = rx.await? {
                    record.stub.key
                } else {
                    return Err(
                        format!("could not find key for address: {}", address.to_string()).into(),
                    );
                }
            }
        };

        self.tx
            .send(ResourceRegistryAction {
                tx: tx,
                command: ResourceRegistryCommand::Next {
                    key: parent_key.clone(),
                    unique: Unique::Index,
                },
            })
            .await?;

        match rx.await? {
           ResourceRegistryResult::Unique(index) => {
               match resource_type {
                   ResourceType::Root => Ok(ResourceId::Root),
                   ResourceType::Space => Ok(ResourceId::Space(index as _)),
                   ResourceType::SubSpace => Ok(ResourceId::SubSpace(index as _)),
                   ResourceType::App => Ok(ResourceId::App(index as _)),
                   ResourceType::Actor => Ok(ResourceId::Actor(Id::new(0, index as _))),
                   ResourceType::User => Ok(ResourceId::User(index as _)),
                   ResourceType::FileSystem => Ok(ResourceId::FileSystem(index as _)),
                   ResourceType::File => Ok(ResourceId::File(index as _)),
                   ResourceType::Domain => Ok(ResourceId::Domain(index as _)),
                   ResourceType::UrlPathPattern => Ok(ResourceId::UrlPathPattern(index as _)),
                   ResourceType::Proxy => Ok(ResourceId::Proxy(index as _)),
                   ResourceType::ArtifactBundle => Ok(ResourceId::ArtifactBundle(index as _)),
                   ResourceType::Artifact => Ok(ResourceId::Artifact(index as _)),
                   ResourceType::Database => Ok(ResourceId::Database(index as _)),
               }
           }
           what => {
               Err(Fail::Unexpected{ expected:"ResourceRegistryResult::Unique".to_string(), received: what.to_string() })
           }
        }
    }
}

#[derive(Debug,Clone, Serialize, Deserialize)]
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

#[derive(Debug,Clone, Serialize, Deserialize)]
pub struct ResourceLocationAffinity {
    pub star: StarKey,
}

#[derive(Debug,Clone, Serialize, Deserialize)]
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
        Self{
            stub: ResourceStub::root(),
            location: ResourceLocation::root()
        }
    }
}

impl From<ResourceRecord> for ResourceKey {
    fn from(record: ResourceRecord) -> Self {
        record.stub.key
    }
}

impl From<ResourceRecord> for ResourceAddress {
    fn from(record: ResourceRecord) -> Self {
        record.stub.address
    }
}

#[derive(Debug,Clone, Serialize, Deserialize)]
pub struct ResourceLocation {
    pub host: StarKey,
    pub gathering: Option<GatheringKey>,
}

impl ResourceLocation {
    pub fn new(host: StarKey) -> Self {
        ResourceLocation {
            host: host,
            gathering: Option::None,
        }
    }

    pub fn root() -> Self {
        Self{
            host: StarKey::central(),
            gathering: Option::None
        }
    }
}

pub enum ResourceManagerKey {
    Central,
    Key(ResourceKey),
}

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

    pub fn test_address(key: &ResourceKey) -> Result<Self, Error> {
        let mut parts = vec![];

        let mut mark = Option::Some(key.clone());
        while let Option::Some(key) = mark {
            match &key {
                ResourceKey::Root => {
                    // do nothing
                }
                ResourceKey::Space(space) => {
                    parts.push(ResourceAddressPart::SkewerCase(SkewerCase::new(
                        format!("space-{}", space.id()).as_str(),
                    )?));
                }
                ResourceKey::SubSpace(sub_space) => {
                    parts.push(ResourceAddressPart::SkewerCase(SkewerCase::new(
                        format!("sub-{}", sub_space.id).as_str(),
                    )?));
                }
                ResourceKey::App(app) => {
                    parts.push(app.address_part()?);
                }
                ResourceKey::Actor(actor) => {
                    parts.push(actor.address_part()?);
                }
                ResourceKey::User(user) => {
                    parts.push(ResourceAddressPart::SkewerCase(SkewerCase::new(
                        format!("user-{}", user.id).as_str(),
                    )?));
                }
                ResourceKey::File(file) => {
                    parts.push(ResourceAddressPart::Path(Path::new(
                        format!("/files/{}", file.id).as_str(),
                    )?));
                }
                ResourceKey::FileSystem(filesystem) => match filesystem {
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
                ResourceKey::Domain(domain) => {
                    parts.push(ResourceAddressPart::Domain(DomainCase::new(
                        format!("domain-{}", domain.id).as_str(),
                    )?));
                }
                ResourceKey::UrlPathPattern(pattern) => {
                    parts.push(ResourceAddressPart::UrlPathPattern(format!(
                        "url-path-pattern-{}",
                        pattern.id
                    )));
                }
                ResourceKey::Proxy(proxy) => {
                    parts.push(ResourceAddressPart::SkewerCase(SkewerCase::from_str(
                        format!("proxy-{}", proxy.id).as_str(),
                    )?));
                }
                ResourceKey::ArtifactBundle(bundle) => {
                    parts.push(ResourceAddressPart::SkewerCase(SkewerCase::from_str(
                        format!("artifact-bundle-{}", bundle.id).as_str(),
                    )?));
                    parts.push(ResourceAddressPart::Version(Version::from_str("1.0.0")?));
                }
                ResourceKey::Artifact(artifact) => {
                    parts.push(ResourceAddressPart::Path(Path::new(
                        format!("/artifacts/{}", artifact.id).as_str(),
                    )?));
                }
                ResourceKey::Database(_) => {
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

#[derive(Clone, Serialize, Deserialize)]
pub struct ResourceBinding {
    pub key: ResourceKey,
    pub address: ResourceAddress,
}

#[derive(Clone)]
pub struct ResourceAddressStructure {
    parts: Vec<ResourceAddressPartStruct>,
    resource_type: ResourceType,
}

impl ResourceAddressStructure {
    pub fn format(&self) -> String {
        let mut rtn = String::new();
        for (index, part) in self.parts.iter().enumerate() {
            if index != 0 {
                rtn.push_str(RESOURCE_ADDRESS_DELIM);
            }
            rtn.push_str(part.name.as_str());
        }
        rtn
    }

    pub fn new(parts: Vec<ResourceAddressPartStruct>, resource_type: ResourceType) -> Self {
        ResourceAddressStructure {
            parts: parts,
            resource_type: resource_type,
        }
    }

    pub fn with_parent(
        parent: Self,
        mut parts: Vec<ResourceAddressPartStruct>,
        resource_type: ResourceType,
    ) -> Self {
        let mut union = parent.parts.clone();
        union.append(&mut parts);
        Self::new(union, resource_type)
    }
}

impl ResourceAddressStructure {
    pub fn from_str(&self, s: &str) -> Result<ResourceAddress, Error> {
        if s == "<Root>" {
            return Ok(ResourceAddress {
                parts: vec![],
                resource_type: ResourceType::Root,
            });
        }

        let mut split = s.split(RESOURCE_ADDRESS_DELIM);

        if split.count() != self.parts.len() {
            return Err(format!(
                "part count not equal. expected format '{}' received: {}",
                self.format(),
                s
            )
            .into());
        }

        let mut split = s.split(RESOURCE_ADDRESS_DELIM);

        let mut parts = vec![];

        for part in &self.parts {
            parts.push(
                part.kind
                    .from_str(split.next().ok_or(part.kind.to_string())?.clone())?,
            );
        }

        Ok(ResourceAddress {
            parts: parts,
            resource_type: self.resource_type.clone(),
        })
    }

    pub fn matches(&self, parts: Vec<ResourceAddressPart>) -> bool {
        if parts.len() != self.parts.len() {
            return false;
        }
        for (index, part) in parts.iter().enumerate() {
            let part_struct = self.parts.get(index).unwrap();
            if !part_struct.kind.matches(part) {
                return false;
            }
        }

        return true;
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct ResourceAddressPartStruct {
    pub name: String,
    pub kind: ResourceAddressPartKind,
}

impl ResourceAddressPartStruct {
    pub fn new(name: &str, kind: ResourceAddressPartKind) -> Self {
        ResourceAddressPartStruct {
            name: name.to_string(),
            kind: kind,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum ResourceAddressPartKind {
    Domain,
    Url,
    UrlPathPattern,
    Wildcard,
    SkewerCase,
    Email,
    Version,
    WildcardOrSkewer,
    Path,
    Base64Encoded,
}

impl ToString for ResourceAddressPartKind {
    fn to_string(&self) -> String {
        match self {
            ResourceAddressPartKind::Domain => "Domain".to_string(),
            ResourceAddressPartKind::Wildcard => "Wildcard".to_string(),
            ResourceAddressPartKind::SkewerCase => "Skewer".to_string(),
            ResourceAddressPartKind::Version => "Version".to_string(),
            ResourceAddressPartKind::WildcardOrSkewer => "WildcardOrSkewer".to_string(),
            ResourceAddressPartKind::Path => "Path".to_string(),
            ResourceAddressPartKind::Base64Encoded => "Base64Encoded".to_string(),
            ResourceAddressPartKind::Email => "Email".to_string(),
            ResourceAddressPartKind::Url => "Url".to_string(),
            ResourceAddressPartKind::UrlPathPattern => "UrlPathPattern".to_string(),
        }
    }
}

impl ResourceAddressPartKind {
    pub fn matches(&self, part: &ResourceAddressPart) -> bool {
        match part {
            ResourceAddressPart::Wildcard => {
                *self == Self::Wildcard || *self == Self::WildcardOrSkewer
            }
            ResourceAddressPart::SkewerCase(_) => {
                *self == Self::SkewerCase || *self == Self::WildcardOrSkewer
            }
            ResourceAddressPart::Path(_) => *self == Self::Path,
            ResourceAddressPart::Version(_) => *self == Self::Version,
            ResourceAddressPart::Base64Encoded(_) => *self == Self::Base64Encoded,
            ResourceAddressPart::Email(_) => *self == Self::Email,
            ResourceAddressPart::Url(_) => *self == Self::Url,
            ResourceAddressPart::UrlPathPattern(_) => *self == Self::UrlPathPattern,
            ResourceAddressPart::Domain(_) => *self == Self::Domain,
        }
    }

    pub fn from_str(&self, s: &str) -> Result<ResourceAddressPart, Error> {
        if s.contains(RESOURCE_ADDRESS_DELIM) {
            return Err(format!(
                "resource part cannot contain resource address delimeter '{}' as in '{}'",
                RESOURCE_ADDRESS_DELIM, s
            )
            .into());
        }
        match self {
            ResourceAddressPartKind::Wildcard => {
                if s == "*" {
                    Ok(ResourceAddressPart::Wildcard)
                } else {
                    Err("expected wildcard".into())
                }
            }
            ResourceAddressPartKind::SkewerCase => {
                Ok(ResourceAddressPart::SkewerCase(SkewerCase::from_str(s)?))
            }
            ResourceAddressPartKind::WildcardOrSkewer => {
                if s == "*" {
                    Ok(ResourceAddressPart::Wildcard)
                } else {
                    Ok(ResourceAddressPart::SkewerCase(SkewerCase::from_str(s)?))
                }
            }
            ResourceAddressPartKind::Path => Ok(ResourceAddressPart::Path(Path::from_str(s)?)),
            ResourceAddressPartKind::Version => {
                Ok(ResourceAddressPart::Version(Version::from_str(s)?))
            }
            ResourceAddressPartKind::Base64Encoded => Ok(ResourceAddressPart::Base64Encoded(
                Base64Encoded::encoded(s.to_string())?,
            )),
            ResourceAddressPartKind::Email => {
                validate::rules::email().validate(s)?;
                Ok(ResourceAddressPart::Email(
                    s.to_string().trim().to_lowercase(),
                ))
            }
            ResourceAddressPartKind::Url => {
                Ok(ResourceAddressPart::Url(Url::parse(s)?.to_string()))
            }
            ResourceAddressPartKind::Domain => {
                Ok(ResourceAddressPart::Domain(DomainCase::from_str(s)?))
            }
            ResourceAddressPartKind::UrlPathPattern => {
                Ok(ResourceAddressPart::UrlPathPattern(s.to_string()))
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub enum ResourceAddressPart {
    Wildcard,
    SkewerCase(SkewerCase),
    Domain(DomainCase),
    Base64Encoded(Base64Encoded),
    Path(Path),
    Version(Version),
    Email(String),
    Url(String),
    UrlPathPattern(String),
}

impl ToString for ResourceAddressPart {
    fn to_string(&self) -> String {
        match self {
            ResourceAddressPart::Wildcard => "*".to_string(),
            ResourceAddressPart::SkewerCase(skewer) => skewer.to_string(),
            ResourceAddressPart::Base64Encoded(base64) => base64.encoded.clone(),
            ResourceAddressPart::Path(path) => path.to_string(),
            ResourceAddressPart::Version(version) => version.to_string(),
            ResourceAddressPart::Email(email) => email.to_string(),
            ResourceAddressPart::Url(url) => url.to_string(),
            ResourceAddressPart::UrlPathPattern(path) => path.to_string(),
            ResourceAddressPart::Domain(domain) => domain.to_string(),
        }
    }
}

impl ResourceAddressPart {
    pub fn is_wildcard(&self) -> bool {
        match self {
            ResourceAddressPart::Wildcard => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct Base64Encoded {
    encoded: String,
}

impl Base64Encoded {
    pub fn decoded(decoded: String) -> Result<Self, Error> {
        Ok(Base64Encoded {
            encoded: base64::encode(decoded.as_bytes()),
        })
    }

    pub fn encoded(encoded: String) -> Result<Self, Error> {
        match base64::decode(encoded.clone()) {
            Ok(decoded) => match String::from_utf8(decoded) {
                Ok(_) => Ok(Base64Encoded { encoded: encoded }),
                Err(err) => Err(err.to_string().into()),
            },
            Err(err) => Err(err.to_string().into()),
        }
    }
}

impl ToString for Base64Encoded {
    fn to_string(&self) -> String {
        self.encoded.clone()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct SkewerCase {
    string: String,
}

impl SkewerCase {
    pub fn new(string: &str) -> Result<Self, Error> {
        if string.is_empty() {
            return Err("cannot be empty".into());
        }

        for c in string.chars() {
            if !((c.is_lowercase() && c.is_ascii_alphabetic()) || c.is_numeric() || c == '-') {
                return Err(format!("must be lowercase, use only alphanumeric characters & dashes RECEIVED: '{}'", string).into());
            }
        }
        Ok(SkewerCase {
            string: string.to_string(),
        })
    }
}

impl ToString for SkewerCase {
    fn to_string(&self) -> String {
        self.string.clone()
    }
}

impl FromStr for SkewerCase {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(SkewerCase::new(s)?)
    }
}


#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct DisplayValue{
    string: String,
}

impl DisplayValue{
    pub fn new(string: &str) -> Result<Self, Error> {
        if string.is_empty() {
            return Err("cannot be empty".into());
        }

        Ok(DisplayValue{
            string: string.to_string(),
        })
    }
}

impl ToString for DisplayValue{
    fn to_string(&self) -> String {
        self.string.clone()
    }
}

impl FromStr for DisplayValue{
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(DisplayValue::new(s)?)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct DomainCase {
    string: String,
}

impl DomainCase {
    pub fn new(string: &str) -> Result<Self, Error> {
        if string.is_empty() {
            return Err("cannot be empty".into());
        }

        if string.contains("..") {
            return Err("cannot have two dots in a row".into());
        }

        for c in string.chars() {
            if !((c.is_lowercase() && c.is_alphanumeric()) || c == '-' || c == '.') {
                return Err("must be lowercase, use only alphanumeric characters & dashes".into());
            }
        }
        Ok(DomainCase {
            string: string.to_string(),
        })
    }
}

impl ToString for DomainCase {
    fn to_string(&self) -> String {
        self.string.clone()
    }
}

impl FromStr for DomainCase {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(DomainCase::new(s)?)
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use futures::SinkExt;
    use tokio::runtime::Runtime;
    use tokio::sync::mpsc;
    use tokio::sync::oneshot::error::RecvError;
    use tokio::time::Duration;
    use tokio::time::timeout;

    use crate::actor::{ActorKey, ActorKind};
    use crate::error::Error;
    use crate::id::Id;
    use crate::keys::{AppKey, ResourceKey, SpaceKey, SubSpaceId, SubSpaceKey, UserKey};
    use crate::logger::{
        Flag, Flags, Log, LogAggregate, ProtoStarLog, ProtoStarLogPayload, StarFlag, StarLog,
        StarLogPayload,
    };
    use crate::names::{Name, Specific};
    use crate::permissions::Authentication;
    use crate::resource::{
        FieldSelection, Labels, LabelSelection, Names, Registry, ResourceAddress,
        ResourceAddressPart, ResourceArchetype, ResourceAssign, ResourceKind, ResourceRecord,
        ResourceRegistration, ResourceRegistryAction, ResourceRegistryCommand,
        ResourceRegistryInfo, ResourceRegistryResult, ResourceSelector, ResourceStub, ResourceType,
        SkewerCase,
    };
    use crate::resource::ResourceRegistryResult::Resources;
    use crate::space::CreateAppControllerFail;
    use crate::star::{StarController, StarInfo, StarKey, StarKind};
    use crate::starlane::{
        ConstellationCreate, StarlaneApiRequest, StarlaneCommand, StarlaneMachineRunner,
    };
    use crate::template::{ConstellationData, ConstellationTemplate};

    fn create_save(index: usize, resource: ResourceRecord) -> ResourceRegistration {
        if index == 0 {
            eprintln!("don't use 0 index, it messes up the tests.  Start with 1");
            assert!(false)
        }
        let parity = match (index % 2) == 0 {
            true => "Even",
            false => "Odd",
        };

        let names = match index {
            1 => vec!["Lowest".to_string()],
            10 => vec!["Highest".to_string()],
            _ => vec![],
        };

        let mut labels = Labels::new();
        labels.insert("parity".to_string(), parity.to_string());
        labels.insert("index".to_string(), index.to_string());

        let save = ResourceRegistration {
            resource: resource,
            info: Option::Some(ResourceRegistryInfo {
                labels: labels,
                names: names,
            }),
        };
        save
    }

    fn create_with_key(
        key: ResourceKey,
        address: ResourceAddress,
        kind: ResourceKind,
        specific: Option<Specific>,
        sub_space: SubSpaceKey,
        owner: UserKey,
    ) -> ResourceRegistration {
        let stub = ResourceStub {
            key: key,
            address: address,
            owner: Option::Some(owner),
            archetype: ResourceArchetype {
                kind: kind,
                specific: specific,
                config: Option::None,
            },
        };

        let save = ResourceRegistration {
            resource: ResourceRecord::new(stub, StarKey::central()),
            info: Option::Some(ResourceRegistryInfo {
                labels: Labels::new(),
                names: Names::new(),
            }),
        };

        save
    }

    fn create(
        index: usize,
        kind: ResourceKind,
        specific: Option<Specific>,
        sub_space: SubSpaceKey,
        owner: UserKey,
    ) -> ResourceRegistration {
        if index == 0 {
            eprintln!("don't use 0 index, it messes up the tests.  Start with 1");
            assert!(false)
        }
        let key = kind.test_key(sub_space, index);
        let address = ResourceAddress::test_address(&key).unwrap();

        let resource = ResourceRecord::new(
            ResourceStub {
                key: key,
                address: address,
                owner: Option::Some(owner),
                archetype: ResourceArchetype {
                    kind: kind,
                    specific: specific,
                    config: Option::None,
                },
            },
            StarKey::central(),
        );

        create_save(index, resource)
    }

    async fn create_10(
        tx: mpsc::Sender<ResourceRegistryAction>,
        kind: ResourceKind,
        specific: Option<Specific>,
        sub_space: SubSpaceKey,
        owner: UserKey,
    ) {
        for index in 1..11 {
            let save = create(
                index,
                kind.clone(),
                specific.clone(),
                sub_space.clone(),
                owner.clone(),
            );
            let (request, rx) = ResourceRegistryAction::new(ResourceRegistryCommand::Commit(save));
            tx.send(request).await;
            timeout(Duration::from_secs(5), rx).await.unwrap().unwrap();
        }
    }

    async fn create_10_spaces(tx: mpsc::Sender<ResourceRegistryAction>) -> Vec<SpaceKey> {
        let mut spaces = vec![];
        for index in 1..11 {
            let space = SpaceKey::from_index(index as _);
            let address_part = format!("some-space-{}", index);
            let resource = ResourceRecord::new(
                ResourceStub {
                    key: ResourceKey::Space(space.clone()),
                    address: crate::resource::SPACE_ADDRESS_STRUCT
                        .from_str(address_part.as_str())
                        .unwrap(),
                    archetype: ResourceArchetype {
                        kind: ResourceKind::Space,
                        specific: None,
                        config: Option::None,
                    },
                    owner: None,
                },
                StarKey::central(),
            );

            let save = create_save(index, resource);
            let (request, rx) = ResourceRegistryAction::new(ResourceRegistryCommand::Commit(save));
            tx.send(request).await;
            timeout(Duration::from_secs(5), rx).await.unwrap().unwrap();
            spaces.push(space)
        }
        spaces
    }

    async fn create_10_actors(
        tx: mpsc::Sender<ResourceRegistryAction>,
        app: AppKey,
        specific: Option<Specific>,
        sub_space: SubSpaceKey,
        app_address: ResourceAddress,
        owner: UserKey,
    ) {
        for index in 1..11 {
            let actor_key = ResourceKey::Actor(ActorKey::new(app.clone(), Id::new(0, index)));
            let address = ResourceAddress::from_parent(
                &ResourceType::Actor,
                Option::Some(&app_address),
                ResourceAddressPart::SkewerCase(
                    SkewerCase::new(actor_key.encode().unwrap().as_str()).unwrap(),
                ),
            )
            .unwrap();

            let save = create_with_key(
                actor_key,
                address,
                ResourceKind::Actor(ActorKind::Stateful),
                specific.clone(),
                sub_space.clone(),
                owner.clone(),
            );
            let (request, rx) = ResourceRegistryAction::new(ResourceRegistryCommand::Commit(save));
            tx.send(request).await;
            timeout(Duration::from_secs(5), rx).await.unwrap().unwrap();
        }
    }

    async fn create_10_sub_spaces(
        tx: mpsc::Sender<ResourceRegistryAction>,
        space_resource: ResourceStub,
    ) -> Vec<SubSpaceKey> {
        let mut sub_spaces = vec![];
        for index in 1..11 {
            let space = space_resource
                .key
                .space()
                .unwrap_or(SpaceKey::hyper_space());
            let sub_space = SubSpaceKey::new(space.clone(), index as _);
            let address_part = ResourceAddressPart::SkewerCase(
                SkewerCase::new(format!("sub-space-{}", index).as_str()).unwrap(),
            );
            let address = ResourceAddress::from_parent(
                &ResourceType::SubSpace,
                Option::Some(&space_resource.address.clone()),
                address_part.clone(),
            )
            .unwrap();

            let resource = ResourceRecord::new(
                ResourceStub {
                    key: ResourceKey::SubSpace(sub_space.clone()),
                    address: address,
                    archetype: ResourceArchetype {
                        kind: ResourceKind::SubSpace,
                        specific: None,
                        config: None,
                    },
                    owner: None,
                },
                StarKey::central(),
            );

            let save = create_save(index, resource);
            let (request, rx) = ResourceRegistryAction::new(ResourceRegistryCommand::Commit(save));
            tx.send(request).await;
            timeout(Duration::from_secs(5), rx).await.unwrap().unwrap();
            sub_spaces.push(sub_space)
        }
        sub_spaces
    }

    #[test]
    pub fn test10() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let tx = Registry::new(StarInfo::mock(), "tmp/mock_registry_path".to_string() ).await;

            create_10(
                tx.clone(),
                ResourceKind::App,
                Option::None,
                SubSpaceKey::hyper_default(),
                UserKey::hyper_user(),
            )
            .await;
            let mut selector = ResourceSelector::app_selector();
            let (request, rx) =
                ResourceRegistryAction::new(ResourceRegistryCommand::Select(selector));
            tx.send(request).await;
            let result = timeout(Duration::from_secs(5), rx).await.unwrap().unwrap();
            assert_result_count(result, 10);

            let mut selector = ResourceSelector::app_selector();
            selector.add_label(LabelSelection::exact("parity", "Even"));
            let (request, rx) =
                ResourceRegistryAction::new(ResourceRegistryCommand::Select(selector));
            tx.send(request).await;
            let result = timeout(Duration::from_secs(5), rx).await.unwrap().unwrap();
            assert_result_count(result, 5);

            let mut selector = ResourceSelector::app_selector();
            selector.add_label(LabelSelection::exact("parity", "Odd"));
            selector.add_label(LabelSelection::exact("index", "3"));
            let (request, rx) =
                ResourceRegistryAction::new(ResourceRegistryCommand::Select(selector));
            tx.send(request).await;
            let result = timeout(Duration::from_secs(5), rx).await.unwrap().unwrap();
            assert_result_count(result, 1);

            let mut selector = ResourceSelector::app_selector();
            selector.name("Highest".to_string()).unwrap();
            let (request, rx) =
                ResourceRegistryAction::new(ResourceRegistryCommand::Select(selector));
            tx.send(request).await;
            let result = timeout(Duration::from_secs(5), rx).await.unwrap().unwrap();
            assert_result_count(result, 1);

            let mut selector = ResourceSelector::actor_selector();
            let (request, rx) =
                ResourceRegistryAction::new(ResourceRegistryCommand::Select(selector));
            tx.send(request).await;
            let result = timeout(Duration::from_secs(5), rx).await.unwrap().unwrap();
            assert_result_count(result, 0);
        });
    }

    #[test]
    pub fn test20() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let tx = Registry::new(StarInfo::mock(), "tmp/mock_registry_path".to_string() ).await;

            create_10(
                tx.clone(),
                ResourceKind::App,
                Option::None,
                SubSpaceKey::hyper_default(),
                UserKey::hyper_user(),
            )
            .await;
            create_10(
                tx.clone(),
                ResourceKind::Actor(ActorKind::Stateful),
                Option::None,
                SubSpaceKey::hyper_default(),
                UserKey::hyper_user(),
            )
            .await;

            let mut selector = ResourceSelector::new();
            let (request, rx) =
                ResourceRegistryAction::new(ResourceRegistryCommand::Select(selector));
            tx.send(request).await;
            let result = timeout(Duration::from_secs(5), rx).await.unwrap().unwrap();
            assert_result_count(result, 20);

            let mut selector = ResourceSelector::app_selector();
            selector.add_label(LabelSelection::exact("parity", "Even"));
            let (request, rx) =
                ResourceRegistryAction::new(ResourceRegistryCommand::Select(selector));
            tx.send(request).await;
            let result = timeout(Duration::from_secs(5), rx).await.unwrap().unwrap();
            assert_result_count(result, 5);

            let mut selector = ResourceSelector::app_selector();
            selector.add_label(LabelSelection::exact("parity", "Odd"));
            selector.add_label(LabelSelection::exact("index", "3"));
            let (request, rx) =
                ResourceRegistryAction::new(ResourceRegistryCommand::Select(selector));
            tx.send(request).await;
            let result = timeout(Duration::from_secs(5), rx).await.unwrap().unwrap();
            assert_result_count(result, 1);

            let mut selector = ResourceSelector::new();
            selector.name("Highest".to_string()).unwrap();
            let (request, rx) =
                ResourceRegistryAction::new(ResourceRegistryCommand::Select(selector));
            tx.send(request).await;
            let result = timeout(Duration::from_secs(5), rx).await.unwrap().unwrap();
            assert_result_count(result, 2);
        });
    }

    #[test]
    pub fn test_spaces() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let tx = Registry::new(StarInfo::mock(), "tmp/mock_registry_path".to_string() ).await;

            let spaces = create_10_spaces(tx.clone()).await;
            let mut sub_spaces = vec![];
            for space in spaces.clone() {
                let space_resource = ResourceStub {
                    key: ResourceKey::Space(space.clone()),
                    address: ResourceAddress::test_address(&ResourceKey::Space(space.clone()))
                        .unwrap(),
                    archetype: ResourceArchetype {
                        kind: ResourceKind::Space,
                        specific: None,
                        config: None,
                    },
                    owner: None,
                };
                sub_spaces.append(&mut create_10_sub_spaces(tx.clone(), space_resource).await);
            }

            for sub_space in sub_spaces.clone() {
                create_10(
                    tx.clone(),
                    ResourceKind::App,
                    Option::None,
                    sub_space,
                    UserKey::hyper_user(),
                )
                .await;
            }


            let mut selector = ResourceSelector::app_selector();
            let sub_space: ResourceKey = sub_spaces.get(0).cloned().unwrap().into();
            selector.fields.insert(FieldSelection::Parent(
                sub_space.into()
            ));
            let (request, rx) =
                ResourceRegistryAction::new(ResourceRegistryCommand::Select(selector));
            tx.send(request).await;
            let result = timeout(Duration::from_secs(5), rx).await.unwrap().unwrap();
            assert_result_count(result, 10);
        });
    }

    #[test]
    pub fn test_specific() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let tx = Registry::new(StarInfo::mock(), "tmp/mock_registry_path".to_string() ).await;

            create_10(
                tx.clone(),
                ResourceKind::App,
                Option::Some(crate::names::TEST_APP_SPEC.clone()),
                SubSpaceKey::hyper_default(),
                UserKey::hyper_user(),
            )
            .await;
            create_10(
                tx.clone(),
                ResourceKind::App,
                Option::Some(crate::names::TEST_ACTOR_SPEC.clone()),
                SubSpaceKey::hyper_default(),
                UserKey::hyper_user(),
            )
            .await;

            let mut selector = ResourceSelector::app_selector();
            let (request, rx) =
                ResourceRegistryAction::new(ResourceRegistryCommand::Select(selector));
            tx.send(request).await;
            let result = timeout(Duration::from_secs(5), rx).await.unwrap().unwrap();
            assert_result_count(result, 20);

            let mut selector = ResourceSelector::app_selector();
            selector.fields.insert(FieldSelection::Specific(
                crate::names::TEST_APP_SPEC.clone(),
            ));
            let (request, rx) =
                ResourceRegistryAction::new(ResourceRegistryCommand::Select(selector));
            tx.send(request).await;
            let result = timeout(Duration::from_secs(5), rx).await.unwrap().unwrap();
            assert_result_count(result, 10);
        });
    }
    #[test]
    pub fn test_app() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let tx = Registry::new(StarInfo::mock(), "tmp/mock_registry_path".to_string() ).await;

            let sub_space = SubSpaceKey::hyper_default();
            let app1 = AppKey::new(sub_space.clone(), 1);
            let app_address =
                ResourceAddress::test_address(&ResourceKey::App(app1.clone())).unwrap();
            create_10_actors(
                tx.clone(),
                app1.clone(),
                Option::None,
                sub_space.clone(),
                app_address,
                UserKey::hyper_user(),
            )
            .await;

            let app2 = AppKey::new(sub_space.clone(), 2);
            let app_address =
                ResourceAddress::test_address(&ResourceKey::App(app2.clone())).unwrap();
            create_10_actors(
                tx.clone(),
                app2.clone(),
                Option::None,
                sub_space.clone(),
                app_address,
                UserKey::hyper_user(),
            )
            .await;

            let mut selector = ResourceSelector::actor_selector();
            let (request, rx) =
                ResourceRegistryAction::new(ResourceRegistryCommand::Select(selector));
            tx.send(request).await;
            let result = timeout(Duration::from_secs(5), rx).await.unwrap().unwrap();
            assert_result_count(result, 20);

            let mut selector = ResourceSelector::actor_selector();
            let app1: ResourceKey = app1.clone().into();
            selector.add_field(FieldSelection::Parent(app1.into()));
            let (request, rx) =
                ResourceRegistryAction::new(ResourceRegistryCommand::Select(selector));
            tx.send(request).await;
            let result = timeout(Duration::from_secs(5), rx).await.unwrap().unwrap();
            assert_result_count(result, 10);
        });
    }

    fn results(result: ResourceRegistryResult) -> Vec<ResourceRecord> {
        if let ResourceRegistryResult::Resources(resources) = result {
            resources
        } else {
            assert!(false);
            vec![]
        }
    }

    fn assert_result_count(result: ResourceRegistryResult, count: usize) {
        if let ResourceRegistryResult::Resources(resources) = result {
            assert_eq!(resources.len(), count);
            println!("PASS");
        } else if let ResourceRegistryResult::Error(error) = result {
            eprintln!("FAIL: {}", error);
            assert!(false);
        } else {
            eprintln!("FAIL");
            assert!(false);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct Path {
    string: String,
}

impl Path {
    pub fn new(string: &str) -> Result<Self, Error> {
        if string.trim().is_empty() {
            return Err("path cannot be empty".into());
        }

        if string.contains("..") {
            return Err(format!(
                "path cannot contain directory traversal sequence [..] != '{}'",
                string
            )
            .into());
        }

        for c in string.chars() {
            if c == '*' || c == '?' || c == ':' {
                return Err(format!(
                    "path cannot contain wildcard characters [*,?] or [:] != '{}'",
                    string
                )
                .into());
            }
        }

        if !string.starts_with("/") {
            return Err(format!(
                "Paths must be absolute (must start with a '/') != '{}'",
                string
            )
            .into());
        }

        Ok(Path {
            string: string.to_string(),
        })
    }

    pub fn bin(&self) -> Result<Vec<u8>, Error> {
        let mut bin = bincode::serialize(self)?;
        Ok(bin)
    }

    pub fn is_absolute(&self) -> bool {
        self.string.starts_with("/")
    }

    pub fn cat(&self, path: &Path) -> Result<Self, Error> {
        if self.string.ends_with("/") {
            Path::new(format!("{}{}", self.string.as_str(), path.string.as_str()).as_str())
        } else {
            Path::new(format!("{}/{}", self.string.as_str(), path.string.as_str()).as_str())
        }
    }

    pub fn parent(&self) -> Option<Path> {
        let mut copy = self.string.clone();
        if copy.len() <= 1 {
            return Option::None;
        }
        copy.remove(0);
        let split = self.string.split("/");
        if split.count() <= 1 {
            Option::None
        } else {
            let mut segments = vec![];
            let mut split = copy.split("/");
            while let Option::Some(segment) = split.next() {
                segments.push(segment);
            }
            if segments.len() <= 1 {
                return Option::None;
            } else {
                segments.pop();
                let mut string = String::new();
                for segment in segments {
                    string.push_str("/");
                    string.push_str(segment);
                }
                Option::Some(Path::new(string.as_str()).unwrap())
            }
        }
    }

    pub fn to_relative(&self) -> String {
        let mut rtn = self.string.clone();
        rtn.remove(0);
        rtn
    }
}

impl TryInto<Arc<Vec<u8>>> for Path {
    type Error = Error;

    fn try_into(self) -> Result<Arc<Vec<u8>>, Self::Error> {
        Ok(Arc::new(bincode::serialize(&self)?))
    }
}

impl TryFrom<Arc<Vec<u8>>> for Path {
    type Error = Error;

    fn try_from(value: Arc<Vec<u8>>) -> Result<Self, Self::Error> {
        Ok(bincode::deserialize::<Self>(&value)?)
    }
}

impl TryFrom<&str> for Path {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(Path::new(value)?)
    }
}

impl TryFrom<String> for Path {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Ok(Path::new(value.as_str())?)
    }
}

impl ToString for Path {
    fn to_string(&self) -> String {
        self.string.clone()
    }
}

impl FromStr for Path {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Path::new(s)?)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct Version {
    string: String,
}

impl Version {
    pub fn new(string: &str) -> Result<Self, Error> {
        if string.is_empty() {
            return Err("path cannot be empty".into());
        }

        // here we are just verifying that it parses and normalizing the output
        let version = semver::Version::parse(string)?;

        Ok(Version {
            string: version.to_string(),
        })
    }
}

impl Version {
    pub fn as_semver(&self) -> Result<semver::Version, Error> {
        Ok(semver::Version::parse(self.string.as_str())?)
    }
}

impl ToString for Version {
    fn to_string(&self) -> String {
        self.string.clone()
    }
}

impl FromStr for Version {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Version::new(s)?)
    }
}

#[derive(Debug,Clone, Serialize, Deserialize)]
pub enum ResourceCreateStrategy {
    Create,
    Ensure,
}

#[derive(Debug,Clone, Serialize, Deserialize)]
pub struct ResourceCreate {
    pub parent: ResourceIdentifier,
    pub key: KeyCreationSrc,
    pub address: AddressCreationSrc,
    pub archetype: ResourceArchetype,
    pub src: AssignResourceStateSrc,
    pub registry_info: Option<ResourceRegistryInfo>,
    pub owner: Option<UserKey>,
    pub strategy: ResourceCreateStrategy,
}

impl ResourceCreate {
    pub fn create(archetype: ResourceArchetype, src: AssignResourceStateSrc) -> Self {
        ResourceCreate {
            parent: ResourceKey::Root.into(),
            key: KeyCreationSrc::None,
            address: AddressCreationSrc::None,
            archetype: archetype,
            src: src,
            registry_info: Option::None,
            owner: Option::None,
            strategy: ResourceCreateStrategy::Create,
        }
    }

    pub fn ensure_address(archetype: ResourceArchetype, src: AssignResourceStateSrc) -> Self {
        ResourceCreate {
            parent: ResourceKey::Root.into(),
            key: KeyCreationSrc::None,
            address: AddressCreationSrc::None,
            archetype: archetype,
            src: src,
            registry_info: Option::None,
            owner: Option::None,
            strategy: ResourceCreateStrategy::Ensure,
        }
    }

    pub fn validate(&self) -> Result<(), Fail> {
        let resource_type = self.archetype.kind.resource_type();

        self.archetype.valid()?;

        if resource_type.requires_owner() && self.owner.is_none() {
            return Err(Fail::ResourceTypeRequiresOwner);
        };

        if let KeyCreationSrc::Key(key) = &self.key {
            if key.resource_type() != resource_type {
                return Err(Fail::ResourceTypeMismatch("ResourceCreate: key: KeyCreationSrc::Key(key) resource type != init.archetype.kind.resource_type()".into()));
            }
        }

        Ok(())
    }

    pub async fn to_keyed(self, starlane_api: StarlaneApi )->Result<Self,Error>{
        Ok(Self{
            parent: self.parent.to_key(&starlane_api).await?.into(),
            key: self.key,
            address: self.address,
            archetype: self.archetype,
            src: self.src,
            registry_info: self.registry_info,
            owner: self.owner,
            strategy: self.strategy
        })
    }

    pub fn keyed_or(self, message: &str) -> Result<Self,Error> {
        if self.parent.is_key() {
            return Ok(self)
        } else {
            Err(message.into())
        }
    }


}

impl LogInfo for ResourceCreate {
    fn log_identifier(&self) -> String {
        self.archetype.log_identifier()
    }

    fn log_kind(&self) -> String {
        self.archetype.log_kind()
    }

    fn log_object(&self) -> String {
        "ResourceCreate".to_string()
    }
}

#[derive(Debug,Clone, Serialize, Deserialize)]
pub enum ResourceStatus {
    Unknown,
    Preparing,
    Ready,
}
impl ToString for ResourceStatus {
    fn to_string(&self) -> String {
        match self {
            Self::Unknown => "Unknown".to_string(),
            Self::Preparing => "Preparing".to_string(),
            Self::Ready => "Ready".to_string(),
        }
    }
}

impl FromStr for ResourceStatus {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Unknown" => Ok(Self::Unknown),
            "Preparing" => Ok(Self::Preparing),
            "Ready" => Ok(Self::Ready),
            what => Err(format!("not recognized: {}", what).into()),
        }
    }
}

#[derive(Debug,Clone, Serialize, Deserialize)]
pub enum AddressCreationSrc {
    None,
    Append(String),
    Appends(Vec<String>),
    Space(String),
    Exact(ResourceAddress)
}

#[derive(Debug,Clone, Serialize, Deserialize)]
pub enum KeyCreationSrc {
    None,
    Key(ResourceKey),
}

#[derive(Clone, Serialize, Deserialize)]
pub enum KeySrc {
    None,
    Key(ResourceKey),
    Address(ResourceAddress),
}

/// can have other options like to Initialize the state data
#[derive(Debug,Clone, Serialize, Deserialize,strum_macros::Display)]
pub enum AssignResourceStateSrc {
    None,
    Direct(Arc<Vec<u8>>),
    InitArgs(String),
    Hosted,
}

impl TryInto<ResourceStateSrc> for AssignResourceStateSrc {
    type Error = Error;

    fn try_into(self) -> Result<ResourceStateSrc, Self::Error> {
        match self {
            AssignResourceStateSrc::Direct(state) => Ok(ResourceStateSrc::Memory(state)),
            AssignResourceStateSrc::Hosted => Ok(ResourceStateSrc::Hosted),
            AssignResourceStateSrc::None => Ok(ResourceStateSrc::None),
            _ => {
                Err(format!("cannot turn {}", self.to_string() ).into())
            }
        }
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
    key: ResourceKey,
    archetype: ResourceArchetype,
}

#[derive(Debug,Clone, Serialize, Deserialize)]
pub struct ResourceStub {
    pub key: ResourceKey,
    pub address: ResourceAddress,
    pub archetype: ResourceArchetype,
    pub owner: Option<UserKey>,
}

impl ResourceStub {
    pub fn root() -> ResourceStub {
        ResourceStub {
            key: ResourceKey::Root,
            address: ResourceAddress::root(),
            archetype: ResourceArchetype::root(),
            owner: Option::None,
        }
    }

}

impl LogInfo for ResourceStub {
    fn log_identifier(&self) -> String {
        self.address.to_parts_string()
    }

    fn log_kind(&self) -> String {
        self.archetype.kind.to_string()
    }

    fn log_object(&self) -> String {
        "ResourceStub".to_string()
    }
}

impl From<Resource> for ResourceStub {
    fn from(resource: Resource) -> Self {
        ResourceStub {
            key: resource.key,
            address: resource.address,
            archetype: resource.archetype,
            owner: resource.owner,
        }
    }
}

impl ResourceStub {
    pub fn validate(&self, resource_type: ResourceType) -> bool {
        self.key.resource_type() == resource_type
            && self.address.resource_type == resource_type
            && self.archetype.kind.resource_type() == resource_type
    }
}

#[derive(Debug,Clone, Serialize, Deserialize)]
pub struct ResourceAssign<S> {
    pub stub: ResourceStub,
    pub state_src: S,
}

impl<S> ResourceAssign<S> {
    pub fn key(&self) -> ResourceKey {
        self.stub.key.clone()
    }

    pub fn archetype(&self) -> ResourceArchetype {
        self.stub.archetype.clone()
    }
}

impl TryInto<ResourceAssign<ResourceStateSrc>> for ResourceAssign<AssignResourceStateSrc> {
    type Error = Error;

    fn try_into(self) -> Result<ResourceAssign<ResourceStateSrc>, Self::Error> {
        let state_src = self.state_src.try_into()?;
        Ok(ResourceAssign {
            stub: self.stub,
            state_src: state_src,
        })
    }
}

pub struct LocalResourceHost {
    skel: StarSkel,
    resource: ResourceKey,
}

#[async_trait]
impl ResourceHost for LocalResourceHost {
    fn star_key(&self) -> StarKey {
        self.skel.info.key.clone()
    }

    async fn assign(&self, assign: ResourceAssign<AssignResourceStateSrc>) -> Result<(), Fail> {
        unimplemented!()
    }
}

pub struct RemoteResourceHost {
    pub comm: StarComm,
    pub handle: StarHandle,
}

#[async_trait]
impl ResourceHost for RemoteResourceHost {
    fn star_key(&self) -> StarKey {
        self.handle.key.clone()
    }

    async fn assign(&self, assign: ResourceAssign<AssignResourceStateSrc>) -> Result<(), Fail> {
        if !self
            .handle
            .kind
            .hosts()
            .contains(&assign.stub.key.resource_type())
        {
            return Err(Fail::WrongResourceType {
                expected: self.handle.kind.hosts().clone(),
                received: assign.stub.key.resource_type().clone(),
            });
        }

        let mut proto = ProtoStarMessage::new();
        proto.to = self.handle.key.clone().into();
        proto.payload = StarMessagePayload::ResourceHost(ResourceHostAction::Assign(assign));
        let reply = proto.get_ok_result().await;
        self.comm
            .star_tx
            .send(StarCommand::SendProtoMessage(proto))
            .await;

        match tokio::time::timeout(Duration::from_secs(25), reply).await {
            Ok(result) => {
                match result {
                    Result::Ok(StarMessagePayload::Reply(SimpleReply::Ok(_))) => {
                        Ok(())
                    }
                    Result::Ok(what) => {
                        Err(Fail::expected("Ok(StarMessagePayload::Reply(SimpleReply::Ok(_)))"))
                    }
                    Result::Err(err) => {
                        Err(Fail::expected("Ok(StarMessagePayload::Reply(SimpleReply::Ok(_)))"))
                    }
                }

            }
            Err(err) => Err(Fail::Timeout)
        }
    }
}

#[derive(Clone)]
pub struct Resource {
    key: ResourceKey,
    address: ResourceAddress,
    archetype: ResourceArchetype,
    state_src: Arc<dyn DataTransfer>,
    owner: Option<UserKey>,
}

impl Resource {
    pub fn new(
        key: ResourceKey,
        address: ResourceAddress,
        archetype: ResourceArchetype,
        state_src: Arc<dyn DataTransfer>,
    ) -> Resource {
        Resource {
            key: key,
            address: address,
            state_src: state_src,
            archetype: archetype,
            owner: Option::None, // fix later
        }
    }

    pub fn key(&self) -> ResourceKey {
        self.key.clone()
    }

    pub fn address(&self) -> ResourceAddress {
        self.address.clone()
    }

    pub fn resource_type(&self) -> ResourceType {
        self.key.resource_type()
    }

    pub fn state_src(&self) -> Arc<dyn DataTransfer> {
        self.state_src.clone()
    }
}

pub type ResourceStateSrc = LocalDataSrc;

#[derive(Clone)]
pub enum LocalDataSrc {
    None,
    Memory(Arc<Vec<u8>>),
    File(Path),
    Hosted,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum RemoteDataSrc {
    None,
    Memory(Arc<Vec<u8>>),
}

#[derive(Clone)]
pub struct SrcTransfer<S>
where
    S: TryInto<Arc<Vec<u8>>> + TryFrom<Arc<Vec<u8>>>,
{
    data_transfer: Option<Arc<dyn DataTransfer>>,
    data: Option<Arc<S>>,
}

impl<S> SrcTransfer<S>
where
    S: TryInto<Arc<Vec<u8>>> + TryFrom<Arc<Vec<u8>>>,
{
    pub fn new(data_transfer: Arc<dyn DataTransfer>) -> Self {
        SrcTransfer {
            data_transfer: Option::Some(data_transfer),
            data: Option::None,
        }
    }

    pub async fn get(&mut self) -> Result<Arc<S>, Error> {
        match &self.data {
            None => {
                let data = self.data_transfer.as_ref().unwrap().get().await?;
                let s = match S::try_from(data) {
                    Ok(s) => s,
                    Err(err) => return Err("could not convert to data".into()),
                };
                let s = Arc::new(s);
                self.data = Option::Some(s.clone());
                self.data_transfer = Option::None;
                Ok(s)
            }
            Some(s) => Ok(s.clone()),
        }
    }
}

#[async_trait]
pub trait DataTransfer: Send + Sync {
    async fn get(&self) -> Result<Arc<Vec<u8>>, Error>;
    fn src(&self) -> LocalDataSrc;
}

#[derive(Clone)]
pub struct MemoryDataTransfer {
    data: Arc<Vec<u8>>,
}

impl MemoryDataTransfer {
    pub fn none() -> Self {
        MemoryDataTransfer {
            data: Arc::new(vec![]),
        }
    }

    pub fn new(data: Arc<Vec<u8>>) -> Self {
        MemoryDataTransfer { data: data }
    }
}

#[async_trait]
impl DataTransfer for MemoryDataTransfer {
    async fn get(&self) -> Result<Arc<Vec<u8>>, Error> {
        Ok(self.data.clone())
    }

    fn src(&self) -> LocalDataSrc {
        LocalDataSrc::Memory(self.data.clone())
    }
}

pub struct FileDataTransfer {
    file_access: FileAccess,
    path: Path,
}

impl FileDataTransfer {
    pub fn new(file_access: FileAccess, path: Path) -> Self {
        FileDataTransfer {
            file_access: file_access,
            path: path,
        }
    }
}

#[async_trait]
impl DataTransfer for FileDataTransfer {
    async fn get(&self) -> Result<Arc<Vec<u8>>, Error> {
        self.file_access.read(&self.path).await
    }

    fn src(&self) -> LocalDataSrc {
        LocalDataSrc::File(self.path.clone())
    }
}

pub enum ResourceStatePersistenceManager {
    None,
    Store,
    Host,
}


pub trait ResourceSelectorId: Debug+Clone+Serialize+for <'de> Deserialize<'de>+Eq+PartialEq+Hash+Into<ResourceIdentifier>+Sized{}

#[derive(Debug, Clone, Serialize, Deserialize,Eq,PartialEq,Hash)]
pub enum ResourceIdentifier {
    Key(ResourceKey),
    Address(ResourceAddress),
}

impl ResourceSelectorId for ResourceIdentifier {}


impl ResourceIdentifier{

    pub fn is_key(&self) -> bool {
        match self {
            ResourceIdentifier::Key(_) => {
                true
            }
            ResourceIdentifier::Address(_) => {
                false
            }
        }
    }

    pub fn is_address(&self) -> bool {
        match self {
            ResourceIdentifier::Key(_) => {
                false
            }
            ResourceIdentifier::Address(_) => {
                true
            }
        }
    }


    pub fn key_or(self,error_message: &str ) -> Result<ResourceKey,Error> {
        match self {
            ResourceIdentifier::Key(key) => {
                Ok(key)
            }
            ResourceIdentifier::Address(_) => {
                Err(error_message.into())
            }
        }
    }

    pub fn address_or(self,error_message: &str ) -> Result<ResourceAddress,Error> {
        match self {
            ResourceIdentifier::Key(_) => {
                Err(error_message.into())
            }
            ResourceIdentifier::Address(address) => {
                Ok(address)
            }
        }
    }

    pub async fn to_key(mut self, starlane_api: &StarlaneApi ) -> Result<ResourceKey,Error> {
        match self{
            ResourceIdentifier::Key(key) => {Ok(key)}
            ResourceIdentifier::Address(address) => {
                Ok(starlane_api.fetch_resource_key(address).await?)
            }
        }
    }

    pub async fn to_address(mut self, starlane_api: &StarlaneApi ) -> Result<ResourceAddress,Error> {
        match self{
            ResourceIdentifier::Address(address) => {Ok(address)}
            ResourceIdentifier::Key(key) => {
                Ok(starlane_api.fetch_resource_address(key).await?)
            }
        }
    }
}

impl ResourceIdentifier {
    pub fn parent(&self) -> Option<ResourceIdentifier> {
        match self {
            ResourceIdentifier::Key(key) => match key.parent() {
                None => Option::None,
                Some(parent) => Option::Some(parent.into()),
            },
            ResourceIdentifier::Address(address) => match address.parent() {
                None => Option::None,
                Some(parent) => Option::Some(parent.into()),
            },
        }
    }

    pub fn resource_type(&self) -> ResourceType {
        match self {
            ResourceIdentifier::Key(key) => key.resource_type(),
            ResourceIdentifier::Address(address) => address.resource_type(),
        }
    }
}

impl From<ResourceAddress> for ResourceIdentifier {
    fn from(address: ResourceAddress) -> Self {
        ResourceIdentifier::Address(address)
    }
}

impl From<ResourceKey> for ResourceIdentifier {
    fn from(key: ResourceKey) -> Self {
        ResourceIdentifier::Key(key)
    }
}

impl ToString for ResourceIdentifier {
    fn to_string(&self) -> String {
        match self {
            ResourceIdentifier::Key(key) => key.to_string(),
            ResourceIdentifier::Address(address) => address.to_string(),
        }
    }
}
