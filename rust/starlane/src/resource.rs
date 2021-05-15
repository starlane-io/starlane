use std::collections::{HashMap, HashSet};
use std::fmt;
use std::iter::FromIterator;
use std::str::FromStr;

use bincode::ErrorKind;
use rusqlite::{Connection, params, params_from_iter, Rows, Statement, ToSql};
use rusqlite::types::{ToSqlOutput, Value, ValueRef};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};

use crate::actor::{ActorKey, ActorKind};
use crate::app::{App, AppKind};
use crate::artifact::{ArtifactKey, ArtifactKind};
use crate::error::Error;
use crate::filesystem::FileKey;
use crate::id::Id;
use crate::names::{Name, Specific};
use crate::permissions::User;
use crate::keys::{SubSpaceKey, ResourceKey, AppKey, SubSpaceId, SpaceKey, UserKey, GatheringKey, FileSystemKey, AppFilesystemKey, SubSpaceFilesystemKey};
use crate::star::StarKey;

lazy_static!
{
    pub static ref SPACE_ADDRESS_STRUCT:ResourceAddressStructure = ResourceAddressStructure::new( vec![ResourceAddressPartKind::Skewer] );
    pub static ref SUB_SPACE_ADDRESS_STRUCT:ResourceAddressStructure = ResourceAddressStructure::with( SPACE_ADDRESS_STRUCT,vec![ResourceAddressPartKind::Skewer] );
    pub static ref APP_ADDRESS_STRUCT:ResourceAddressStructure = ResourceAddressStructure::with( SUB_SPACE_ADDRESS_STRUCT,vec![ResourceAddressPartKind::Skewer] );
    pub static ref ACTOR_ADDRESS_STRUCT:ResourceAddressStructure = ResourceAddressStructure::with( APP_ADDRESS_STRUCT,vec![ResourceAddressPartKind::Skewer] );
    pub static ref USER_ADDRESS_STRUCT:ResourceAddressStructure = ResourceAddressStructure::with( SPACE_ADDRESS_STRUCT,vec![ResourceAddressPartKind::Skewer] );
    pub static ref FILE_SYSTEM_ADDRESS_STRUCT:ResourceAddressStructure = ResourceAddressStructure::new( vec![ResourceAddressPartKind::Skewer,ResourceAddressPartKind::Skewer,ResourceAddressPartKind::WildcardOrSkewer,ResourceAddressPartKind::Skewer] );
    pub static ref FILE_ADDRESS_STRUCT:ResourceAddressStructure = ResourceAddressStructure::with( ResourceType::FileSystem.address_structure(),vec![ResourceAddressPartKind::Path] );
    pub static ref ARTIFACT_ADDRESS_STRUCT:ResourceAddressStructure = ResourceAddressStructure::new( vec![ResourceAddressPartKind::Skewer,ResourceAddressPartKind::Skewer,ResourceAddressPartKind::Skewer,ResourceAddressPartKind::Version,ResourceAddressPartKind::Path] );

    pub static ref HYPERSPACE_ADDRESS: ResourceAddress = SPACE_ADDRESS_STRUCT.from_str("hyperspace").unwrap();
    pub static ref HYPERSPACE_DEFAULT_ADDRESS: ResourceAddress = SUB_SPACE_ADDRESS_STRUCT.from_str("hyperspace:default").unwrap();
}

pub type Labels = HashMap<String,String>;



#[derive(Clone,Serialize,Deserialize)]
pub struct Selector
{
    pub meta: MetaSelector,
    pub fields: HashSet<FieldSelection>
}

#[derive(Clone,Serialize,Deserialize)]
pub enum MetaSelector
{
    None,
    Name(String),
    Label(LabelSelector)
}

#[derive(Clone,Serialize,Deserialize)]
pub struct LabelSelector
{
    pub labels: HashSet<LabelSelection>
}

impl Selector {
    pub fn new()-> Selector{
        Selector{
            meta: MetaSelector::None,
            fields: HashSet::new()
        }
    }

    pub fn resource_types(&self)->HashSet<ResourceType>
    {
        let mut rtn = HashSet::new();
        for field in &self.fields
        {
            if let FieldSelection::Type(resource_type) = field
            {
                rtn.insert(resource_type.clone());
            }
        }
        rtn
    }

    pub fn add( &mut self, field: FieldSelection )
    {
        self.fields.retain( |f| !f.is_matching_kind(&field));
        self.fields.insert(field);
    }

    pub fn is_empty(&self) -> bool
    {
        if !self.fields.is_empty()
        {
            return false;
        }

        match &self.meta
        {
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

    pub fn name( &mut self, name: String ) -> Result<(),Error>
    {
        match &mut self.meta
        {
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

    pub fn add_label( &mut self, label: LabelSelection ) -> Result<(),Error>
    {
        match &mut self.meta
        {
            MetaSelector::None => {
                self.meta = MetaSelector::Label(LabelSelector{
                    labels : HashSet::new()
                });
                self.add_label(label)
            }
            MetaSelector::Name(_) => {
                Err("Selector is already set to a NAME meta selector".into())
            }
            MetaSelector::Label(selector) => {
                selector.labels.insert( label );
                Ok(())
            }
        }
    }

    pub fn add_field( &mut self, field: FieldSelection )
    {
        self.fields.insert(field);
    }
}

pub type AppSelector = Selector;
pub type ActorSelector = Selector;

impl Selector {

    pub fn app_selector()->AppSelector {
      let mut selector = AppSelector::new();
      selector.add(FieldSelection::Type(ResourceType::App));
      selector
    }

    pub fn actor_selector()->ActorSelector {
        let mut selector = ActorSelector::new();
        selector.add(FieldSelection::Type(ResourceType::Actor));
        selector
    }

}

#[derive(Clone,Hash,Eq,PartialEq,Serialize,Deserialize)]
pub enum LabelSelection
{
    Exact(Label)
}

impl LabelSelection
{
    pub fn exact( name: &str, value: &str )->Self
    {
        LabelSelection::Exact(Label{
            name: name.to_string(),
            value: value.to_string()
        })
    }
}


#[derive(Clone,Hash,Eq,PartialEq,Serialize,Deserialize)]
pub enum FieldSelection
{
    Type(ResourceType),
    Kind(ResourceKind),
    Specific(Specific),
    Owner(UserKey),
    Space(SpaceKey),
    SubSpace(SubSpaceKey),
    App(AppKey),
}


impl ToSql for Name
{
    fn to_sql(&self) -> Result<ToSqlOutput<'_>, rusqlite::Error> {
        Ok(ToSqlOutput::Owned(Value::Text(self.to())))
    }
}

impl FieldSelection
{
    pub fn is_matching_kind(&self, field: &FieldSelection ) ->bool
    {
        match self
        {
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
            FieldSelection::Space(_) => {
                if let FieldSelection::Space(_) = field {
                    return true;
                }
            }
            FieldSelection::SubSpace(_) => {
                if let FieldSelection::SubSpace(_) = field {
                    return true;
                }
            }
            FieldSelection::App(_) => {
                if let FieldSelection::App(_) = field {
                    return true;
                }
            }
        };
        return false;
    }
}

impl ToSql for FieldSelection
{
    fn to_sql(&self) -> Result<ToSqlOutput<'_>, rusqlite::Error> {
        match self
        {
            FieldSelection::Type(resource_type) => {
                Ok(ToSqlOutput::Owned(Value::Text(resource_type.to_string())))
            }
            FieldSelection::Kind(kind) => {
                Ok(ToSqlOutput::Owned(Value::Text(kind.to_string())))
            }
            FieldSelection::Specific(specific) => {
                Ok(ToSqlOutput::Owned(Value::Text(specific.to_string())))
            }
            FieldSelection::Owner(owner) => {
                let owner = bincode::serialize(owner );
                match owner
                {
                    Ok(owner) => {
                        Ok(ToSqlOutput::Owned(Value::Blob(owner)))
                    }
                    Err(error) => {
                        Err(rusqlite::Error::InvalidQuery)
                    }
                }
            }
            FieldSelection::Space(space) => {
                Ok(ToSqlOutput::Owned(Value::Integer(space.index() as _)))
            }
            FieldSelection::SubSpace(sub_space) => {
                let sub_space= bincode::serialize(sub_space );
                match sub_space
                {
                    Ok(sub_space) => {
                        Ok(ToSqlOutput::Owned(Value::Blob(sub_space)))
                    }
                    Err(error) => {
                        Err(rusqlite::Error::InvalidQuery)
                    }
                }
            }
            FieldSelection::App(app) => {
                let app = bincode::serialize(app);
                match app
                {
                    Ok(app) => {
                        Ok(ToSqlOutput::Owned(Value::Blob(app)))
                    }
                    Err(error) => {
                        Err(rusqlite::Error::InvalidQuery)
                    }
                }
            }
        }
    }
}

#[derive(Clone,Hash,Eq,PartialEq,Serialize,Deserialize)]
pub struct Label
{
    pub name: String,
    pub value: String
}

#[derive(Clone,Serialize,Deserialize)]
pub struct LabelConfig
{
    pub name: String,
    pub index: bool
}

pub struct RegistryAction
{
    pub tx: oneshot::Sender<RegistryResult>,
    pub command: RegistryCommand
}

impl RegistryAction
{
    pub fn new(command: RegistryCommand) ->(Self, oneshot::Receiver<RegistryResult>)
    {
        let (tx,rx) = oneshot::channel();
        (RegistryAction { tx: tx, command: command }, rx)
    }
}

pub enum RegistryCommand
{
    Close,
    Clear,
    Accept(HashSet<ResourceType>),
    Register(ResourceRegistration),
    Select(Selector),
    SetLocation(ResourceLocation),
    Find(ResourceKey),
    Bind(ResourceBinding),
    GetAddress(ResourceKey),
    GetKey(ResourceAddress)
}

#[derive(Clone,Serialize,Deserialize)]
pub enum RegistryResult
{
    Ok,
    Error(String),
    Resource(Resource),
    Resources(Vec<Resource>),
    Address(ResourceAddress),
    Location(ResourceLocation),
    Key(ResourceKey),
    NotFound,
    NotAccepted
}

pub struct Registry {
   pub conn: Connection,
   pub rx: mpsc::Receiver<RegistryAction>,
   pub accepted: Option<HashSet<ResourceType>>
}

impl Registry {
    pub async fn new() -> mpsc::Sender<RegistryAction>
    {
        let (tx, rx) = mpsc::channel(8 * 1024);

        tokio::spawn(async move {

            let conn = Connection::open_in_memory();
            if conn.is_ok()
            {
                let mut db = Registry {
                    conn: conn.unwrap(),
                    rx: rx,
                    accepted: Option::None
                };
                db.run().await.unwrap();
            }
        });
        tx
    }

    async fn run(&mut self) -> Result<(), Error>
    {
        self.setup()?;

        while let Option::Some(request) = self.rx.recv().await {
            if let RegistryCommand::Close = request.command
            {
                break;
            }
            match self.process( request.command )
            {
                Ok(ok) => {
                    request.tx.send(ok);
                }
                Err(err) => {
                    eprintln!("{}",err);
                    request.tx.send(RegistryResult::Error(err.to_string()));
                }
            }
        }

        Ok(())
    }

    fn accept( &self, resource_type: ResourceType )->bool
    {
        if self.accepted.is_none() {
            return true;
        }

        let accepted = self.accepted.as_ref().unwrap();

        return accepted.contains(&resource_type);
    }

    fn process(&mut self, command: RegistryCommand) -> Result<RegistryResult, Error> {
        match command
        {
            RegistryCommand::Close => {
                Ok(RegistryResult::Ok)
            }
            RegistryCommand::Clear => {
                let trans = self.conn.transaction()?;
                trans.execute("DELETE FROM labels", [] )?;
                trans.execute("DELETE FROM names", [] )?;
                trans.execute("DELETE FROM resources", [])?;
                trans.execute("DELETE FROM locations", [])?;
                trans.commit();

                Ok(RegistryResult::Ok)
            }
            RegistryCommand::Accept(accept)=> {
                self.accepted= Option::Some(accept);
                Ok(RegistryResult::Ok)
            }
            RegistryCommand::Register(register) => {
                if !self.accept(register.resource.key.resource_type() ) {
                    return Ok(RegistryResult::NotAccepted);
                }
                let resource = register.resource;
                let labels = register.labels;
                let key = resource.key.bin()?;

                let resource_type = format!("{}", &resource.key.resource_type());
                let kind = format!("{}", &resource.kind);

                let owner = match &resource.owner{
                    None => Option::None,
                    Some(owner) => {
                        Option::Some(bincode::serialize(owner)?)
                    }
                };

                let app = match &resource.app() {
                    None => Option::None,
                    Some(app) => {
                        Option::Some(app.bin()?)
                    }
                };


                let space = resource.key.space().index();
                let sub_space = bincode::serialize(&resource.key.sub_space())?;

                let trans = self.conn.transaction()?;
                trans.execute("DELETE FROM labels WHERE labels.resource_key=?1", [key.clone()]);
                trans.execute("DELETE FROM names WHERE key=?1", [key.clone()])?;
                trans.execute("DELETE FROM resources WHERE key=?1", [key.clone()])?;

                trans.execute("INSERT INTO resources (key,resource_type,kind,specific,space,sub_space,owner,app) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)", params![key.clone(),resource_type,kind,resource.specific.clone(),space,sub_space,owner,app])?;
                if register.name.is_some()
                {
                    trans.execute("INSERT INTO names (key,name,resource_type,kind,specific,space,sub_space,owner,app) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)", params![key.clone(),register.name,resource_type,kind,resource.specific.clone(),space,sub_space,owner,app])?;
                }

                for (name, value) in labels
                {
                    trans.execute("INSERT INTO labels (resource_key,name,value) VALUES (?1,?2,?3)", params![key.clone(),name, value])?;
                }

                trans.commit()?;
                Ok(RegistryResult::Ok)
            }
            RegistryCommand::Select(selector) => {

                for resource_type in selector.resource_types() {
                    if !self.accept(resource_type ) {
                        return Ok(RegistryResult::NotAccepted);
                    }
                }

                let mut params = vec![];
                let mut where_clause = String::new();

                for (index, field) in Vec::from_iter(selector.fields.clone()).iter().map(|x| x.clone() ).enumerate()
                {
                    if index != 0 {
                        where_clause.push_str(" AND ");
                    }

                    let f = match field {
                        FieldSelection::Type(_) => {
                            format!("resource_type=?{}", index + 1)
                        }
                        FieldSelection::Kind(_) => {
                            format!("kind=?{}", index + 1)
                        }
                        FieldSelection::Specific(_) => {
                            format!("specific=?{}", index + 1)
                        }
                        FieldSelection::Owner(_) => {
                            format!("owner=?{}", index + 1)
                        }
                        FieldSelection::Space(_) => {
                            format!("space=?{}", index + 1)
                        }
                        FieldSelection::SubSpace(_) => {
                            format!("sub_space=?{}", index + 1)
                        }
                        FieldSelection::App(_) => {
                            format!("app=?{}", index + 1)
                        }

                    };
                    where_clause.push_str(f.as_str());
                    params.push(field);
                }



                let mut statement = match &selector.meta
                {
                    MetaSelector::None => {
                        format!("SELECT DISTINCT r.key,r.kind,r.specific,r.owner FROM resources as r WHERE {}", where_clause )
                    }
                    MetaSelector::Label(label_selector) => {

                        let mut labels = String::new();
                        for (index, label_selection ) in Vec::from_iter(label_selector.labels.clone() ).iter().map(|x| x.clone() ).enumerate()
                        {
                            if let LabelSelection::Exact(label) = label_selection
                            {
                                labels.push_str(format!(" AND r.key IN (SELECT labels.resource_key FROM labels WHERE labels.name='{}' AND labels.value='{}')", label.name, label.value).as_str())
                            }
                        }

                        format!("SELECT DISTINCT r.key,r.kind,r.specific,r.owner FROM resources as r WHERE {} {}", where_clause, labels )
                    }
                    MetaSelector::Name(name) => {
                        if where_clause.is_empty() {
                            format!("SELECT DISTINCT r.key,r.kind,r.specific,r.owner FROM names as r WHERE r.name='{}'", name)
                        }
                        else {
                            format!("SELECT DISTINCT r.key,r.kind,r.specific,r.owner FROM names as r WHERE {} AND r.name='{}'", where_clause, name)
                        }
                    }
                };

                // in case this search was for EVERYTHING
                if selector.is_empty()
                {
                    statement = "SELECT DISTINCT r.key,r.kind,r.specific,r.owner FROM resources as r".to_string();
                }

                println!("STATEMENT {}",statement);

                let mut statement = self.conn.prepare(statement.as_str())?;
                let mut rows= statement.query( params_from_iter(params.iter() ) )?;

                let mut resources = vec![];
                while let Option::Some(row) = rows.next()?
                {
                    let key:Vec<u8> = row.get(0)?;
                    let key = ResourceKey::from_bin(key)?;

                    let kind:String = row.get(1)?;
                    let kind= ResourceKind::from_str(kind.as_str())?;

                    let specific = if let ValueRef::Null = row.get_ref(2)? {
                        Option::None
                    }
                    else {
                        let specific: String = row.get(2)?;
                        let specific: Specific = Specific::from(specific.as_str())?;
                        Option::Some(specific)
                    };

                    let owner = if let ValueRef::Null = row.get_ref(3)? {
                        Option::None
                    }
                    else {
                        let owner:Vec<u8> = row.get(3)?;
                        let owner = bincode::deserialize::<UserKey>(owner.as_slice() )?;
                        Option::Some(owner)
                    };

                    let resource = Resource{
                        key: key,
                        specific: specific,
                        owner: owner,
                        kind: kind
                    };
                    resources.push(resource);
                }
                Ok(RegistryResult::Resources(resources) )
            }
            RegistryCommand::SetLocation(location) => {
                if !self.accept(location.key.resource_type()) {
                   return Ok(RegistryResult::NotAccepted);
                }

                let key = location.key.bin()?;
                let host = location.host.bin()?;
                let gathering = match location.gathering{
                    None => Option::None,
                    Some(key) => Option::Some(key.bin()?)
                };
                let mut trans = self.conn.transaction()?;
                trans.execute("INSERT INTO locations (key,host,gathering) VALUES (?1,?2,?3)", params![key,host,gathering])?;
                trans.commit();
                Ok(RegistryResult::Ok)
            }
            RegistryCommand::Find(key) => {

                let key = key.bin()?;
                let statement = "SELECT (key,host,gathering) FROM locations WHERE key=?1";
                let mut statement = self.conn.prepare(statement)?;
                let result = statement.query_row( params![key], |row| {
                    let key: Vec<u8> = row.get(0)?;
                    let key = ResourceKey::from_bin(key)?;

                    let host: Vec<u8> = row.get(1)?;
                    let host = StarKey::from_bin(host)?;

                    let gathering = if let ValueRef::Null = row.get_ref(2)? {
                        Option::None
                    } else {
                        let gathering: Vec<u8> = row.get(2)?;
                        let gathering: GatheringKey = GatheringKey::from_bin(gathering)?;
                        Option::Some(gathering)
                    };
                    let location = ResourceLocation {
                        key: key,
                        host: host,
                        gathering: gathering
                    };
                    Ok(location)
                });

                match result
                {
                    Ok(location) => {
                        Ok(RegistryResult::Location(location))
                    }
                    Err(err) => {
                        Ok(RegistryResult::NotFound)
                    }
                }
            }
            RegistryCommand::Bind(bind) => {
                let key = bind.key.bin()?;
                let address = bind.address.to_string();

                let mut trans = self.conn.transaction()?;
                trans.execute("DELETE addresses WHERE key=?1 OR address=?2)", params![key,address])?;
                trans.execute("INSERT INTO addresses (key,address) VALUES (?1,?2)", params![key,address])?;
                trans.commit();
                Ok(RegistryResult::Ok)
            }
            RegistryCommand::GetAddress(key) => {
                let key_blob = key.bin()?;
                let statement = "SELECT address FROM addresses WHERE key=?1";
                let mut statement = self.conn.prepare(statement)?;
                let result = statement.query_row( params![key_blob], |row| {

                    let address: String = row.get(0)?;
                    let address=key.resource_type().address_structure().from_str(address.as_str())?;
                    Ok(address)
                });

                match result
                {
                    Ok(address) => {
                        Ok(RegistryResult::Address(address))
                    }
                    Err(err) => {
                        Ok(RegistryResult::NotFound)
                    }
                }
            }
            RegistryCommand::GetKey(address) => {
                let address = address.to_string();
                let statement = "SELECT key FROM addresses WHERE address=?1";
                let mut statement = self.conn.prepare(statement)?;
                let result = statement.query_row( params![address], |row| {

                    let key: Vec<u8>= row.get(0)?;
                    let key=ResourceKey::from_bin(key)?;
                    Ok(key)
                });

                match result
                {
                    Ok(key) => {
                        Ok(RegistryResult::Key(key))
                    }
                    Err(err) => {
                        Ok(RegistryResult::NotFound)
                    }
                }
            }
        }
    }



    pub fn setup(&mut self)->Result<(),Error>
    {
      let labels= r#"
       CREATE TABLE IF NOT EXISTS labels (
	      key INTEGER PRIMARY KEY AUTOINCREMENT,
	      resource_key BLOB,
	      name TEXT NOT NULL,
	      value TEXT NOT NULL,
          UNIQUE(key,name),
          FOREIGN KEY (resource_key) REFERENCES resources (key)
        )"#;

        let names= r#"
       CREATE TABLE IF NOT EXISTS names(
          key BLOB PRIMARY KEY,
	      name TEXT NOT NULL,
	      resource_type TEXT NOT NULL,
          kind BLOB NOT NULL,
          specific TEXT,
          space INTEGER NOT NULL,
          sub_space BLOB NOT NULL,
          app TEXT,
          owner BLOB,
          UNIQUE(name,resource_type,kind,specific,space,sub_space,app)
        )"#;


        let resources = r#"CREATE TABLE IF NOT EXISTS resources (
         key BLOB PRIMARY KEY,
         resource_type TEXT NOT NULL,
         kind BLOB NOT NULL,
         specific TEXT,
         space INTEGER NOT NULL,
         sub_space BLOB NOT NULL,
         app TEXT,
         owner BLOB
        )"#;

        let locations = r#"CREATE TABLE IF NOT EXISTS locations (
         key BLOB PRIMARY KEY,
         host BLOB NOT NULL,
         gathering BLOB
        )"#;

        let addresses = r#"CREATE TABLE IF NOT EXISTS addresses(
         key BLOB PRIMARY KEY,
         address TEXT NOT NULL
         UNIQUE(address)
        )"#;

        /*
      let labels_to_resources = r#"CREATE TABLE IF NOT EXISTS labels_to_resources
        (
           resource_key BLOB,
           label_key INTEGER,
           PRIMARY KEY (resource_key, label_key),
           FOREIGN KEY (resource_key) REFERENCES resources (key),
           FOREIGN KEY (label_key) REFERENCES labels (key)
        )
        "#;

         */

        let transaction = self.conn.transaction()?;
        transaction.execute(labels, [])?;
        transaction.execute(names, [])?;
        transaction.execute(resources, [])?;
        transaction.execute(locations, [])?;
        transaction.execute(addresses, [])?;
        transaction.commit();

        Ok(())
    }
}


#[derive(Clone,Serialize,Deserialize,Hash,Eq,PartialEq)]
pub enum ResourceKind
{
    Space,
    SubSpace,
    App(AppKind),
    Actor(ActorKind),
    User,
    File,
    FileSystem(FileSystemKind),
    Artifact(ArtifactKind)
}

#[derive(Clone,Serialize,Deserialize,Hash,Eq,PartialEq)]
pub enum FileSystemKind
{
    App,
    SubSpace
}

impl ResourceType
{
    pub fn magic(&self) -> u8
    {
        match self
        {
            ResourceType::Space => 0,
            ResourceType::SubSpace => 1,
            ResourceType::App => 2,
            ResourceType::Actor => 3,
            ResourceType::User => 4,
            ResourceType::File => 5,
            ResourceType::Artifact => 6,
            ResourceType::FileSystem => 7
        }
    }

    pub fn from_magic(magic: u8)->Result<Self,Error>
    {
        match magic
        {
            0 => Ok(ResourceType::Space),
            1 => Ok(ResourceType::SubSpace),
            2 => Ok(ResourceType::App),
            3 => Ok(ResourceType::Actor),
            4 => Ok(ResourceType::User),
            5 => Ok(ResourceType::File),
            6 => Ok(ResourceType::Artifact),
            7 => Ok(ResourceType::FileSystem),
            _ => Err(format!("no resource type for magic number {}",magic).into())
        }
    }
}

impl fmt::Display for ResourceKind{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!( f,"{}",
                match self{
                    ResourceKind::Space=> "Space".to_string(),
                    ResourceKind::SubSpace=> "SubSpace".to_string(),
                    ResourceKind::App(kind)=> format!("App:{}",kind).to_string(),
                    ResourceKind::Actor(kind)=> format!("Actor:{}",kind).to_string(),
                    ResourceKind::User=> "User".to_string(),
                    ResourceKind::File=> "File".to_string(),
                    ResourceKind::FileSystem(kind)=> format!("Filesystem:{}", kind).to_string(),
                    ResourceKind::Artifact(kind)=>format!("Artifact:{}",kind).to_string()
                })
    }

}

impl ResourceKind {
    pub fn test_key(&self, sub_space: SubSpaceKey, index: usize )->ResourceKey
    {
        match self
        {
            ResourceKind::Space => {
                ResourceKey::Space(SpaceKey::from_index(index as u32 ))
            }
            ResourceKind::SubSpace => {
                ResourceKey::SubSpace(SubSpaceKey::new( sub_space.space, SubSpaceId::Index(index as u32)))
            }
            ResourceKind::App(_) => {
                ResourceKey::App(AppKey::new(sub_space))
            }
            ResourceKind::Actor(_) => {
                let app = AppKey::new(sub_space);
                ResourceKey::Actor(ActorKey::new(app, Id::new(0, index as _)))
            }
            ResourceKind::User => {
                ResourceKey::User(UserKey::new(sub_space.space))
            }
            ResourceKind::File => {
                ResourceKey::File(FileKey{
                    filesystem: FileSystemKey::SubSpace(SubSpaceFilesystemKey{ sub_space, id: 0}),
                    path: index as _
                } )
            }
            ResourceKind::Artifact(_) => {
                ResourceKey::Artifact(ArtifactKey{
                    sub_space: sub_space,
                    id: index as _
                })
            }
            ResourceKind::FileSystem(kind) => {
                match kind
                {
                    FileSystemKind::App => {
                        ResourceKey::Filesystem(FileSystemKey::App(AppFilesystemKey{
                            app:AppKey::new(sub_space),
                            id: index as _
                        }))
                    }
                    FileSystemKind::SubSpace => {
                        ResourceKey::Filesystem(FileSystemKey::SubSpace(SubSpaceFilesystemKey{
                            sub_space: sub_space,
                            id: index as _
                        }))

                    }
                }

            }
        }
    }
}

impl FromStr for ResourceKind
{
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {

        if s.starts_with("App:") {
            let mut split = s.split(":");
            split.next().ok_or("error")?;
            return Ok( ResourceKind::App( AppKind::from_str(split.next().ok_or("error")?)? ));
        } else if s.starts_with("Actor:") {
            let mut split = s.split(":");
            split.next().ok_or("error")?;
            return Ok( ResourceKind::Actor( ActorKind::from_str(split.next().ok_or("error")?)? ) );
        } else if s.starts_with("Artifact:") {
            let mut split = s.split(":");
            split.next().ok_or("error")?;
            return Ok( ResourceKind::Artifact( ArtifactKind::from_str(split.next().ok_or("error")?)? ) );
        }


        match s
        {
            "Space" => Ok(ResourceKind::Space),
            "SubSpace" => Ok(ResourceKind::SubSpace),
            "User" => Ok(ResourceKind::User),
            "File" => Ok(ResourceKind::File),
            _ => {
                Err(format!("cannot match ResourceKind: {}", s).into())
            }
        }
    }
}

#[derive(Clone,Serialize,Deserialize,Hash,Eq,PartialEq)]
pub enum ResourceType
{
    Space,
    SubSpace,
    App,
    Actor,
    User,
    FileSystem,
    File,
    Artifact
}

impl ResourceType
{
    pub fn has_specific(&self)->bool
    {
        match self
        {
            ResourceType::Space => false,
            ResourceType::SubSpace => false,
            ResourceType::App => true,
            ResourceType::Actor => true,
            ResourceType::User => false,
            ResourceType::File => false,
            ResourceType::FileSystem => false,
            ResourceType::Artifact => true
        }
    }

    pub fn address_required(&self)->bool
    {
        match self
        {
            ResourceType::Space => true,
            ResourceType::SubSpace => true,
            ResourceType::App => false,
            ResourceType::Actor => false,
            ResourceType::User => false,
            ResourceType::File => true,
            ResourceType::FileSystem => true,
            ResourceType::Artifact => true
        }
    }

    pub fn address_structure(&self) ->&ResourceAddressStructure{
        match self{
            ResourceType::Space => {
                &SPACE_ADDRESS_STRUCT
            }
            ResourceType::SubSpace => {
                &SUB_SPACE_ADDRESS_STRUCT
            }
            ResourceType::App => {
                &APP_ADDRESS_STRUCT
            }
            ResourceType::Actor => {
                &ACTOR_ADDRESS_STRUCT
            }
            ResourceType::User => {
                &USER_ADDRESS_STRUCT
            }
            ResourceType::FileSystem => {
                &FILE_SYSTEM_ADDRESS_STRUCT
            }
            ResourceType::File => {
                &FILE_ADDRESS_STRUCT
            }
            ResourceType::Artifact => {
                &ARTIFACT_ADDRESS_STRUCT
            }
        }
    }
}


impl fmt::Display for ResourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!( f,"{}",
                match self{
                    ResourceType::Space=> "Space".to_string(),
                    ResourceType::SubSpace=> "SubSpace".to_string(),
                    ResourceType::App=> "App".to_string(),
                    ResourceType::Actor=> "Actor".to_string(),
                    ResourceType::User=> "User".to_string(),
                    ResourceType::File=> "File".to_string(),
                    ResourceType::FileSystem => "Filesystem".to_string(),
                    ResourceType::Artifact=> "Artifact".to_string(),
                })
    }
}

impl fmt::Display for FileSystemKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!( f,"{}",
                match self{
                    FileSystemKind::App => "App".to_string(),
                    FileSystemKind::SubSpace => "SubSpace".to_string()
                })
    }
}
#[derive(Clone,Serialize,Deserialize)]
pub struct Resource
{
    pub key: ResourceKey,
    pub kind: ResourceKind,
    pub owner: Option<UserKey>,
    pub specific: Option<Name>,
}

impl Resource
{
    pub fn app(&self)->Option<AppKey>
    {
        match &self.key
        {
            ResourceKey::Space(_) => Option::None,
            ResourceKey::SubSpace(_) => Option::None,
            ResourceKey::App(app) => Option::Some(app.clone()),
            ResourceKey::Actor(actor) => {
                Option::Some(actor.app.clone())
            }
            ResourceKey::User(_) => Option::None,
            ResourceKey::File(_) => Option::None,
            ResourceKey::Artifact(_) => Option::None,
            ResourceKey::Filesystem(filesystem) => {
                match filesystem
                {
                    FileSystemKey::App(app) => {
                        Option::Some(app.app.clone())
                    }
                    FileSystemKey::SubSpace(_) => {
                        Option::None
                    }
                }
            }
        }
    }
}

impl From<AppKind> for ResourceKind{
    fn from(e: AppKind) -> Self {
        ResourceKind::App(e)
    }
}

impl From<App> for ResourceKind{
    fn from(e: App) -> Self {
        ResourceKind::App(e.archetype.kind)
    }
}

impl From<ActorKind> for ResourceKind{
    fn from(e: ActorKind) -> Self {
        ResourceKind::Actor(e)
    }
}

impl From<ArtifactKind> for ResourceKind{
    fn from(e: ArtifactKind) -> Self {
        ResourceKind::Artifact(e)
    }
}

impl From<SpaceKey> for Resource{
    fn from(e: SpaceKey) -> Self {
        Resource{
            key: ResourceKey::Space(e),
            kind: ResourceKind::Space,
            owner: Option::Some(UserKey::hyper_user()),
            specific: None
        }
    }
}

impl From<SubSpaceKey> for Resource{
    fn from(e: SubSpaceKey) -> Self {
        Resource{
            key: ResourceKey::SubSpace(e.clone()),
            kind: ResourceKind::SubSpace,
            owner: Option::Some(UserKey::super_user(e.space.clone())),
            specific: None
        }
    }
}



impl From<User> for Resource{
    fn from(e: User) -> Self {
        Resource{
            key: ResourceKey::User(e.key.clone()),
            specific: Option::None,
            owner: Option::Some(e.key),
            kind: ResourceKind::User
        }
    }
}


#[derive(Clone,Serialize,Deserialize)]
pub struct ResourceMeta
{
    name: Option<String>,
    labels: Labels
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ResourceRegistration
{
    pub resource: Resource,
    pub name: Option<String>,
    pub labels: Labels,
}

impl ResourceRegistration
{
    pub fn new( resource: Resource, name: Option<String>, labels: Labels )->Self
    {
        ResourceRegistration{
            resource: resource,
            name: name,
            labels: labels
        }
    }
}

#[derive(Clone,Serialize,Deserialize)]
pub struct ResourceLocation
{
    pub key: ResourceKey,
    pub host: StarKey,
    pub gathering: Option<GatheringKey>
}

impl ResourceLocation
{
    pub fn new( key: ResourceKey, host: StarKey )->Self
    {
        ResourceLocation {
            key: key,
            host: host,
            gathering: Option::None
        }
    }
}

pub enum ResourceManagerKey
{
    Central,
    Key(ResourceKey)
}


#[derive(Clone,Serialize,Deserialize)]
pub struct ResourceAddress
{
   parts: Vec<ResourceAddressPart>
}


impl ToString for ResourceAddress {
    fn to_string(&self) -> String {
        let mut rtn = String::new();
        for (index,part) in self.parts.iter().enumerate() {
            if index != 0{
                rtn.push_str(":")
            }
            rtn.push_str(part.to_string().as_str() );
        }
        rtn
    }
}

pub struct ResourceBinding{
    pub key: ResourceKey,
    pub address: ResourceAddress
}

#[derive(Clone)]
pub struct ResourceAddressStructure
{
    kinds: Vec<ResourceAddressPartKind>
}

impl ResourceAddressStructure
{
    pub fn new( kinds: Vec<ResourceAddressPartKind> ) -> Self {
        ResourceAddressStructure{
            kinds: kinds
        }
    }

    pub fn with( parent: Self, mut kinds: Vec<ResourceAddressPartKind>) -> Self
    {
        let mut union = parent.kinds.clone();
        union.append( &mut kinds );
        Self::new(union)
    }
}




impl ResourceAddressStructure {

    fn from_str(&self, s: &str) -> Result<ResourceAddress, Error> {
        let mut split = s.split(":");

        if split.count()  != self.kinds.len() {
            return Err("part count not equal".into());
        }

        let mut split = s.split(":");

        let mut parts = vec![];

        for kind in &self.kinds {
            parts.push(kind.from_str(split.next().unwrap().clone() )?);
        }

        Ok(ResourceAddress{
            parts: parts
        })
    }

    pub fn matches( &self, parts: Vec<ResourceAddressPart> ) -> bool {
        if parts.len() != self.kinds.len() {
            return false;
        }
        for (index,part) in parts.iter().enumerate()
        {
            let kind = self.kinds.get(index).unwrap();
            if !kind.matches(part) {
                return false;
            }
        }

        return true;
    }
}

#[derive(Clone,Serialize,Deserialize,Eq,PartialEq)]
pub enum ResourceAddressPartKind
{
    Wildcard,
    Skewer,
    Version,
    WildcardOrSkewer,
    Path
}

impl ResourceAddressPartKind
{
    pub fn matches( &self, part: &ResourceAddressPart )->bool
    {
        match part
        {
            ResourceAddressPart::Wildcard => {
                *self == Self::Wildcard || *self == Self::WildcardOrSkewer
            }
            ResourceAddressPart::Skewer(_) => {
                *self == Self::Skewer || *self == Self::WildcardOrSkewer
            }
            ResourceAddressPart::Path(_) => {
                *self == Self::Path
            }
            ResourceAddressPart::Version(_) => {
                *self == Self::Version
            }
        }
    }

    pub fn from_str(&self, s: &str ) -> Result<ResourceAddressPart,Error> {
        match self{
            ResourceAddressPartKind::Wildcard => {
                if s == "*" {
                    Ok(ResourceAddressPart::Wildcard)
                }
                else
                {
                    Err("expected wildcard".into())
                }
            }
            ResourceAddressPartKind::Skewer => {
                Ok(ResourceAddressPart::Skewer(Skewer::from_str(s)?))
            }
            ResourceAddressPartKind::WildcardOrSkewer => {
                if s == "*" {
                    Ok(ResourceAddressPart::Wildcard)
                }
                else
                {
                    Ok(ResourceAddressPart::Skewer(Skewer::from_str(s)?))
                }
            }
            ResourceAddressPartKind::Path => {
                Ok(ResourceAddressPart::Path(Path::from_str(s)?))
            }
            ResourceAddressPartKind::Version => {
                Ok(ResourceAddressPart::Version(Version::from_str(s)?))
            }
        }
    }
}



#[derive(Clone,Serialize,Deserialize)]
pub enum ResourceAddressPart
{
    Wildcard,
    Skewer(Skewer),
    Path(Path),
    Version(Version)
}

impl ToString for ResourceAddressPart {
    fn to_string(&self) -> String {
        match self {
            ResourceAddressPart::Wildcard => "*".to_string(),
            ResourceAddressPart::Skewer(skewer) => skewer.to_string(),
            ResourceAddressPart::Path(path) => path.to_string(),
            ResourceAddressPart::Version(version) => version.to_string()
        }
    }
}

#[derive(Clone,Serialize,Deserialize)]
pub struct Skewer
{
    string: String
}

impl Skewer
{
    pub fn new( string: &str ) -> Result<Self,Error> {
        if string.is_empty() {
            return Err("cannot be empty".into());
        }

        for c in string.chars() {
            if !(c.is_lowercase() && (c.is_alphanumeric() || c == '-')) {
                return Err("must be lowercase, use only alphanumeric characters & dashes".into());
            }
        }
        Ok(Skewer{
            string: string.to_string()
        })
    }
}

impl ToString for Skewer {
    fn to_string(&self) -> String {
        self.string.clone()
    }
}

impl FromStr for Skewer {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Skewer::new(s )?)
    }
}






#[cfg(test)]
mod test
{
    use std::sync::Arc;

    use tokio::runtime::Runtime;
    use tokio::sync::mpsc;
    use tokio::sync::oneshot::error::RecvError;
    use tokio::time::Duration;
    use tokio::time::timeout;

    use crate::actor::{ActorKey, ActorKind};
    use crate::app::{AppController, AppKind, AppSpecific, ConfigSrc, InitData};
    use crate::artifact::{Artifact, ArtifactKind, ArtifactLocation};
    use crate::error::Error;
    use crate::id::Id;
    use crate::keys::{AppKey, ResourceKey, SpaceKey, SubSpaceId, SubSpaceKey, UserKey};
    use crate::logger::{Flag, Flags, Log, LogAggregate, ProtoStarLog, ProtoStarLogPayload, StarFlag, StarLog, StarLogPayload};
    use crate::names::{Name, Specific};
    use crate::permissions::Authentication;
    use crate::resource::{FieldSelection, Labels, LabelSelection, Registry, RegistryAction, RegistryCommand, RegistryResult, Resource, ResourceKind, ResourceType, Selector, ResourceRegistration};
    use crate::resource::FieldSelection::SubSpace;
    use crate::resource::RegistryResult::Resources;
    use crate::space::CreateAppControllerFail;
    use crate::star::{StarController, StarInfo, StarKey, StarKind};
    use crate::starlane::{ConstellationCreate, StarControlRequestByName, Starlane, StarlaneCommand};
    use crate::template::{ConstellationData, ConstellationTemplate};

    fn create_save( index: usize, resource: Resource ) -> ResourceRegistration
    {
        if index == 0
        {
            eprintln!("don't use 0 index, it messes up the tests.  Start with 1");
            assert!(false)
        }
        let parity = match (index%2)==0 {
            true => "Even",
            false => "Odd"
        };

        let name = match index
        {
            1 => Option::Some("Lowest".to_string()),
            10 => Option::Some("Highest".to_string()),
            _ => Option::None
        };

        let mut labels = Labels::new();
        labels.insert( "parity".to_string(), parity.to_string() );
        labels.insert( "index".to_string(), index.to_string() );

        let save = ResourceRegistration{
            resource: resource,
            labels: labels,
            name: name
        };
        save
    }

    fn create_with_key(  key: ResourceKey, kind: ResourceKind, specific: Option<Specific>, sub_space: SubSpaceKey, owner: UserKey ) -> ResourceRegistration
    {
        let resource = Resource{
            key: key,
            owner: Option::Some(owner),
            kind: kind,
            specific: specific
        };

        let save = ResourceRegistration{
            resource: resource,
            labels: Labels::new(),
            name: Option::None
        };

        save
    }


    fn create( index: usize, kind: ResourceKind, specific: Option<Specific>, sub_space: SubSpaceKey, owner: UserKey ) -> ResourceRegistration
    {
        if index == 0
        {
            eprintln!("don't use 0 index, it messes up the tests.  Start with 1");
            assert!(false)
        }
        let key = kind.test_key(sub_space,index);

        let resource = Resource{
            key: key,
            owner: Option::Some(owner),
            kind: kind,
            specific: specific
        };

        create_save(index,resource)
    }

    async fn create_10(tx: mpsc::Sender<RegistryAction>, kind: ResourceKind, specific: Option<Specific>, sub_space: SubSpaceKey, owner: UserKey )
    {
        for index in 1..11
        {
            let save = create(index,kind.clone(),specific.clone(),sub_space.clone(),owner.clone());
            let (request,rx) = RegistryAction::new(RegistryCommand::Register(save));
            tx.send( request ).await;
            timeout( Duration::from_secs(5),rx).await.unwrap().unwrap();
        }
    }

    async fn create_10_spaces(tx: mpsc::Sender<RegistryAction> ) ->Vec<SpaceKey>
    {
        let mut spaces = vec!();
        for index in 1..11
        {
            let space = SpaceKey::from_index(index as _);
            let resource: Resource = space.clone().into();

            let save = create_save(index,resource);
            let (request,rx) = RegistryAction::new(RegistryCommand::Register(save));
            tx.send( request ).await;
            timeout( Duration::from_secs(5),rx).await.unwrap().unwrap();
            spaces.push(space)
        }
        spaces
    }


    async fn create_10_actors(tx: mpsc::Sender<RegistryAction>, app: AppKey, specific: Option<Specific>, sub_space: SubSpaceKey, owner: UserKey )
    {
        for index in 1..11
        {
            let actor_key = ResourceKey::Actor(ActorKey::new(app.clone(), Id::new(0, index)));

            let save = create_with_key(actor_key,ResourceKind::Actor(ActorKind::Single),specific.clone(),sub_space.clone(),owner.clone());
            let (request,rx) = RegistryAction::new(RegistryCommand::Register(save));
            tx.send( request ).await;
            timeout( Duration::from_secs(5),rx).await.unwrap().unwrap();
        }
    }


    async fn create_10_sub_spaces(tx: mpsc::Sender<RegistryAction>, space: SpaceKey ) ->Vec<SubSpaceKey>
    {
        let mut sub_spaces = vec!();
        for index in 1..11
        {
            let sub_space = SubSpaceKey::new(space.clone(), SubSpaceId::from_index(index as _) );
            let resource: Resource = sub_space.clone().into();
            let save = create_save(index,resource);
            let (request,rx) = RegistryAction::new(RegistryCommand::Register(save));
            tx.send( request ).await;
            timeout( Duration::from_secs(5),rx).await.unwrap().unwrap();
            sub_spaces.push(sub_space)
        }
        sub_spaces
    }


    #[test]
    pub fn test10()
    {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let tx = Registry::new().await;

            create_10(tx.clone(), ResourceKind::App(AppKind::Normal),Option::None,SubSpaceKey::hyper_default(), UserKey::hyper_user() ).await;
            let mut selector = Selector::app_selector();
            let (request,rx) = RegistryAction::new(RegistryCommand::Select(selector) );
            tx.send(request).await;
            let result = timeout( Duration::from_secs(5),rx).await.unwrap().unwrap();
            assert_result_count(result,10);

            let mut selector = Selector::app_selector();
            selector.add_label( LabelSelection::exact("parity", "Even") );
            let (request,rx) = RegistryAction::new(RegistryCommand::Select(selector) );
            tx.send(request).await;
            let result = timeout( Duration::from_secs(5),rx).await.unwrap().unwrap();
            assert_result_count(result.clone(),5);

            let mut selector = Selector::app_selector();
            selector.add_label( LabelSelection::exact("parity", "Odd") );
            selector.add_label( LabelSelection::exact("index", "3") );
            let (request,rx) = RegistryAction::new(RegistryCommand::Select(selector) );
            tx.send(request).await;
            let result = timeout( Duration::from_secs(5),rx).await.unwrap().unwrap();
            assert_result_count(result,1);


            let mut selector = Selector::app_selector();
            selector.name("Highest".to_string()).unwrap();
            let (request,rx) = RegistryAction::new(RegistryCommand::Select(selector) );
            tx.send(request).await;
            let result = timeout( Duration::from_secs(5),rx).await.unwrap().unwrap();
            assert_result_count(result,1);

            let mut selector = Selector::actor_selector();
            let (request,rx) = RegistryAction::new(RegistryCommand::Select(selector) );
            tx.send(request).await;
            let result = timeout( Duration::from_secs(5),rx).await.unwrap().unwrap();
            assert_result_count(result,0);
        });
    }

    #[test]
    pub fn test20()
    {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let tx = Registry::new().await;

            create_10(tx.clone(), ResourceKind::App(AppKind::Normal),Option::None,SubSpaceKey::hyper_default(), UserKey::hyper_user() ).await;
            create_10(tx.clone(), ResourceKind::Actor(ActorKind::Single),Option::None,SubSpaceKey::hyper_default(), UserKey::hyper_user() ).await;

            let mut selector = Selector::new();
            let (request,rx) = RegistryAction::new(RegistryCommand::Select(selector) );
            tx.send(request).await;
            let result = timeout( Duration::from_secs(5),rx).await.unwrap().unwrap();
            assert_result_count(result,20);

            let mut selector = Selector::app_selector();
            selector.add_label( LabelSelection::exact("parity", "Even") );
            let (request,rx) = RegistryAction::new(RegistryCommand::Select(selector) );
            tx.send(request).await;
            let result = timeout( Duration::from_secs(5),rx).await.unwrap().unwrap();
            assert_result_count(result.clone(),5);

            let mut selector = Selector::app_selector();
            selector.add_label( LabelSelection::exact("parity", "Odd") );
            selector.add_label( LabelSelection::exact("index", "3") );
            let (request,rx) = RegistryAction::new(RegistryCommand::Select(selector) );
            tx.send(request).await;
            let result = timeout( Duration::from_secs(5),rx).await.unwrap().unwrap();
            assert_result_count(result,1);


            let mut selector = Selector::new();
            selector.name("Highest".to_string()).unwrap();
            let (request,rx) = RegistryAction::new(RegistryCommand::Select(selector) );
            tx.send(request).await;
            let result = timeout( Duration::from_secs(5),rx).await.unwrap().unwrap();
            assert_result_count(result,2);
        });
    }

    #[test]
    pub fn test_spaces()
    {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let tx = Registry::new().await;

            let spaces = create_10_spaces(tx.clone() ).await;
            let mut sub_spaces = vec![];
            for space in spaces.clone() {
                sub_spaces.append( &mut create_10_sub_spaces(tx.clone(), space ).await );
            }

            for sub_space in sub_spaces.clone()
            {
                create_10(tx.clone(), ResourceKind::App(AppKind::Normal),Option::None,sub_space, UserKey::hyper_user() ).await;
            }

            let mut selector = Selector::app_selector();
            selector.fields.insert(FieldSelection::Space(spaces.get(0).cloned().unwrap()));
            let (request,rx) = RegistryAction::new(RegistryCommand::Select(selector) );
            tx.send(request).await;
            let result = timeout( Duration::from_secs(5),rx).await.unwrap().unwrap();
            assert_result_count(result,100);

            let mut selector = Selector::app_selector();
            selector.fields.insert(FieldSelection::SubSpace(sub_spaces.get(0).cloned().unwrap()));
            let (request,rx) = RegistryAction::new(RegistryCommand::Select(selector) );
            tx.send(request).await;
            let result = timeout( Duration::from_secs(5),rx).await.unwrap().unwrap();
            assert_result_count(result,10);


        });
    }

    #[test]
    pub fn test_specific()
    {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let tx = Registry::new().await;


            create_10(tx.clone(), ResourceKind::App(AppKind::Normal),Option::Some(crate::names::TEST_APP_SPEC.clone()), SubSpaceKey::hyper_default(), UserKey::hyper_user() ).await;
            create_10(tx.clone(), ResourceKind::App(AppKind::Normal),Option::Some(crate::names::TEST_ACTOR_SPEC.clone()), SubSpaceKey::hyper_default(), UserKey::hyper_user() ).await;

            let mut selector = Selector::app_selector();
            let (request,rx) = RegistryAction::new(RegistryCommand::Select(selector) );
            tx.send(request).await;
            let result = timeout( Duration::from_secs(5),rx).await.unwrap().unwrap();
            assert_result_count(result,20);

            let mut selector = Selector::app_selector();
            selector.fields.insert(FieldSelection::Specific(crate::names::TEST_APP_SPEC.clone()));
            let (request,rx) = RegistryAction::new(RegistryCommand::Select(selector) );
            tx.send(request).await;
            let result = timeout( Duration::from_secs(5),rx).await.unwrap().unwrap();
            assert_result_count(result,10);


        });
    }
    #[test]
    pub fn test_app()
    {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let tx = Registry::new().await;

            let sub_space = SubSpaceKey::hyper_default();
            let app1 = AppKey::new(sub_space.clone());
            create_10_actors(tx.clone(), app1.clone(), Option::None, sub_space.clone(), UserKey::hyper_user() ).await;

            let app2 = AppKey::new(sub_space.clone());
            create_10_actors(tx.clone(), app2.clone(), Option::None, sub_space.clone(), UserKey::hyper_user() ).await;

            let mut selector = Selector::actor_selector();
            let (request,rx) = RegistryAction::new(RegistryCommand::Select(selector) );
            tx.send(request).await;
            let result = timeout( Duration::from_secs(5),rx).await.unwrap().unwrap();
            assert_result_count(result,20);

            let mut selector = Selector::actor_selector();
            selector.add_field(FieldSelection::App(app1.clone()));
            let (request,rx) = RegistryAction::new(RegistryCommand::Select(selector) );
            tx.send(request).await;
            let result = timeout( Duration::from_secs(5),rx).await.unwrap().unwrap();
            assert_result_count(result,10);
        });
    }

    fn results(result: RegistryResult) ->Vec<Resource>
    {
        if let RegistryResult::Resources(resources) = result
        {
            resources
        }
        else
        {
            assert!(false);
            vec!()
        }
    }


    fn assert_result_count(result: RegistryResult, count: usize )
    {
        if let RegistryResult::Resources(resources) = result
        {
            assert_eq!(resources.len(),count);
            println!("PASS");
        }
        else if let RegistryResult::Error(error) = result
        {
            eprintln!("FAIL: {}",error);
            assert!(false);
        }
        else
        {
            eprintln!("FAIL");
            assert!(false);
        }
    }
}





#[derive(Clone,Serialize,Deserialize)]
pub struct Path
{
    string: String
}

impl Path
{
    pub fn new( string: &str ) -> Result<Self,Error> {

        if string.trim().is_empty() {
            return Err("path cannot be empty".into());
        }

        if string.contains(".."){
            return Err("path cannot contain directory traversal sequence [..]".into());
        }

        for c in string.chars() {
            if c == '*' || c == '?' || c == ':' {
                return Err("path cannot contain wildcard characters [*,?] or [:]".into());
            }
        }
        Ok(Path{
            string: string.to_string()
        })
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
        Ok(Path::new(s )?)
    }
}




#[derive(Clone,Serialize,Deserialize)]
pub struct Version
{
    string: String
}

impl Version
{
    pub fn new( string: &str ) -> Result<Self,Error> {
        if string.is_empty() {
            return Err("path cannot be empty".into());
        }

        // here we are just verifying that it parses and normalizing the output
        let version = semver::Version::parse(string)?;

        Ok(Version{
            string: version.to_string()
        })
    }
}

impl Version
{
    pub fn as_semver(&self)->Result<semver::Version,Error>
    {
        Ok(semver::Version::parse(self.string.as_str() )?)
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
        Ok(Version::new(s )?)
    }
}




