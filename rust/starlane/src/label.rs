
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use crate::error::Error;
use tokio::sync::{mpsc, oneshot};
use crate::keys::{ResourceKey, ResourceType, UserKey, AppKey, Resource, ResourceKind, SpaceKey, SubSpaceKey};
use crate::names::{Name, Specific};
use rusqlite::{Connection, params, ToSql, Statement, Rows, params_from_iter};

use rusqlite::types::{ToSqlOutput, Value, ValueRef};
use std::iter::FromIterator;
use std::str::FromStr;
use bincode::ErrorKind;

pub type Labels = HashMap<String,String>;

pub struct ResourceSave
{
    pub resource: Resource,
    pub labels: Labels,
    pub name: Option<String>,
}


pub struct Selector
{
    pub meta: MetaSelector,
    pub fields: HashSet<FieldSelection>
}

pub enum MetaSelector
{
    None,
    Name(String),
    Label(LabelSelector)
}

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


    pub fn and( &mut self, field: FieldSelection ) {
        self.fields.insert(field);
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
                Err("Selector is already set to a label meta selector".into())

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
                Err("Selector is already set to a named meta selector".into())
            }
            MetaSelector::Label(selector) => {
                selector.labels.insert( label );
                Ok(())
            }
        }

    }
}

pub type AppSelector = Selector;
pub type ActorSelector = Selector;

pub struct Selectors {
}

impl Selectors {

    pub fn app_selector()->AppSelector {
      let mut selector = AppSelector::new();
      selector.and(FieldSelection::Type(ResourceType::App));
      selector
    }

    pub fn actor_selector()->ActorSelector {
        let mut selector = ActorSelector::new();
        selector.and(FieldSelection::Type(ResourceType::Actor));
        selector
    }

}

#[derive(Clone,Hash,Eq,PartialEq)]
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


#[derive(Clone,Hash,Eq,PartialEq)]
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

impl ToSql for AppKey
{
    fn to_sql(&self) -> Result<ToSqlOutput<'_>, rusqlite::Error> {
        let blob= bincode::serialize(self);
        match blob
        {
            Ok(blob) => {
                Ok(ToSqlOutput::Owned(Value::Blob(blob)))
            }
            Err(error) => {
                Err(rusqlite::Error::InvalidQuery)
            }
        }
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
                Ok(ToSqlOutput::Owned(Value::Integer(sub_space.id.index() as _)))
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

#[derive(Clone,Hash,Eq,PartialEq)]
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

pub struct LabelRequest
{
    pub tx: oneshot::Sender<LabelResult>,
    pub command: LabelCommand
}

impl LabelRequest
{
    pub fn new( command: LabelCommand )->(Self,oneshot::Receiver<LabelResult>)
    {
        let (tx,rx) = oneshot::channel();
        (LabelRequest{ tx: tx, command: command },rx)
    }
}

pub enum LabelCommand
{
    Close,
    Save(ResourceSave),
    Select(Selector),
}

#[derive(Clone)]
pub enum LabelResult
{
    Ok,
    Error(String),
    Resources(Vec<Resource>)
}

pub struct LabelDb {
   pub conn: Connection,
   pub rx: mpsc::Receiver<LabelRequest>
}

impl LabelDb {
    pub async fn new() -> mpsc::Sender<LabelRequest>
    {
        let (tx, rx) = mpsc::channel(8 * 1024);

        tokio::spawn(async move {

            let conn = Connection::open_in_memory();
            if conn.is_ok()
            {
                let mut db = LabelDb {
                    conn: conn.unwrap(),
                    rx: rx
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
            if let LabelCommand::Close = request.command
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
                    request.tx.send(LabelResult::Error(err.to_string()));
                }
            }
        }

        Ok(())
    }

    fn process(&mut self, command: LabelCommand ) -> Result<LabelResult, Error> {
        match command
        {
            LabelCommand::Close => {
                Ok(LabelResult::Ok)
            }
            LabelCommand::Save(save) => {
                let resource = save.resource;
                let labels = save.labels;
                let key = bincode::serialize(&resource.key)?;
                let resource_type = format!("{}", &resource.key.resource_type());
                let kind = format!("{}", &resource.kind);

                let owner = bincode::serialize(&resource.owner)?;
                let space = resource.key.space().index();
                let sub_space = resource.key.sub_space().id.index();

                let trans = self.conn.transaction()?;
                trans.execute("DELETE FROM labels WHERE labels.resource_key=?1", [key.clone()]);
                trans.execute("DELETE FROM names WHERE key=?1", [key.clone()])?;
                trans.execute("DELETE FROM resources WHERE key=?1", [key.clone()])?;

                trans.execute("INSERT INTO resources (key,resource_type,kind,specific,space,sub_space,owner,app) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)", params![key.clone(),resource_type,kind,resource.specific.clone(),space,sub_space,owner,resource.app()])?;
                if save.name.is_some()
                {
                    trans.execute("INSERT INTO names (key,name,resource_type,kind,specific,space,sub_space,owner,app) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)", params![key.clone(),save.name,resource_type,kind,resource.specific.clone(),space,sub_space,owner,resource.app()])?;
                }

                for (name, value) in labels
                {
                    trans.execute("INSERT INTO labels (resource_key,name,value) VALUES (?1,?2,?3)", params![key.clone(),name, value])?;
                }

                trans.commit()?;
                Ok(LabelResult::Ok)
            }
            LabelCommand::Select(selector) => {
                let mut params = vec![];
                let mut where_clause = String::new();

                for (index, field) in Vec::from_iter(selector.fields).iter().map(|x| x.clone() ).enumerate()
                {
                    if index != 0 {
                        where_clause.push_str(", ");
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



                let mut statement = format!(r#"SELECT DISTINCT r.key,r.kind,r.specific,r.owner
                              FROM {} as r
                              WHERE {}"#, "{}", where_clause );


                let mut statement = match selector.meta
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
                        format!("SELECT DISTINCT r.key,r.kind,r.specific,r.owner FROM names as r WHERE {} AND r.name='{}'", where_clause, name )
                    }
                };
                println!("STATEMENT {}",statement);

                let mut statement = self.conn.prepare(statement.as_str())?;
                let mut rows= statement.query( params_from_iter(params.iter() ) )?;

                let mut resources = vec![];
                while let Option::Some(row) = rows.next()?
                {
                    let key:Vec<u8> = row.get(0)?;
                    let key = bincode::deserialize::<ResourceKey>(key.as_slice() )?;

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
                Ok(LabelResult::Resources(resources) )
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
          sub_space INTEGER NOT NULL,
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
         sub_space INTEGER NOT NULL,
         app TEXT,
         owner BLOB
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
       // transaction.execute(labels_to_resources, [])?;
        transaction.commit();

        Ok(())
    }
}

#[cfg(test)]
mod test
{
    use std::sync::Arc;

    use tokio::runtime::Runtime;
    use tokio::sync::oneshot::error::RecvError;
    use tokio::time::Duration;
    use tokio::time::timeout;

    use crate::app::{AppController, AppKind, AppSpecific, ConfigSrc, InitData};
    use crate::artifact::{Artifact, ArtifactId, ArtifactKind};
    use crate::error::Error;
    use crate::keys::{SpaceKey, SubSpaceKey, UserKey, ResourceType, Resource, ResourceKind, ResourceKey};
    use crate::label::{Labels, LabelDb, ResourceSave, LabelRequest, LabelCommand, Selectors, LabelResult, FieldSelection, LabelSelection};
    use crate::logger::{Flag, Flags, Log, LogAggregate, ProtoStarLog, ProtoStarLogPayload, StarFlag, StarLog, StarLogPayload};
    use crate::names::{Name, Specific};
    use crate::permissions::Authentication;
    use crate::space::CreateAppControllerFail;
    use crate::star::{StarController, StarInfo, StarKey, StarKind};
    use crate::starlane::{ConstellationCreate, StarControlRequestByName, Starlane, StarlaneCommand};
    use crate::template::{ConstellationData, ConstellationTemplate};
    use crate::label::LabelResult::Resources;
    use tokio::sync::mpsc;
    use crate::actor::ActorKind;

    fn create( index: usize, kind: ResourceKind, specific: Option<Specific>, sub_space: SubSpaceKey, owner: UserKey ) -> ResourceSave
    {
        if index == 0
        {
          eprintln!("don't use 0 index, it messes up the tests.  Start with 1");
          assert!(false)
        }
        let key = kind.test_key(sub_space,index);

        let resource = Resource{
            key: key,
            owner: owner,
            kind: kind,
            specific: specific
        };

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

        let save = ResourceSave{
            resource: resource,
            labels: labels,
            name: name
        };
        save
    }

    async fn create_10( tx: mpsc::Sender<LabelRequest>, kind: ResourceKind, specific: Option<Specific>, sub_space: SubSpaceKey, owner: UserKey )
    {
        for index in 1..11
        {
            let save = create(index,kind.clone(),specific.clone(),sub_space.clone(),owner.clone());
            let (request,rx) =LabelRequest::new(LabelCommand::Save(save));
            tx.send( request ).await;
            timeout( Duration::from_secs(5),rx).await.unwrap().unwrap();
        }
    }


    #[test]
    pub fn test10()
    {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let tx = LabelDb::new().await;

            create_10(tx.clone(), ResourceKind::App(AppKind::Normal),Option::None,SubSpaceKey::hyper_default(), UserKey::hyper_user() ).await;
            let mut selector = Selectors::app_selector();
            let (request,rx) = LabelRequest::new(LabelCommand::Select(selector) );
            tx.send(request).await;
            let result = timeout( Duration::from_secs(5),rx).await.unwrap().unwrap();
            assert_result_count(result,10);

            let mut selector = Selectors::app_selector();
            selector.add_label( LabelSelection::exact("parity", "Even") );
            let (request,rx) = LabelRequest::new(LabelCommand::Select(selector) );
            tx.send(request).await;
            let result = timeout( Duration::from_secs(5),rx).await.unwrap().unwrap();
            assert_result_count(result.clone(),5);

            let mut selector = Selectors::app_selector();
            selector.add_label( LabelSelection::exact("parity", "Odd") );
            selector.add_label( LabelSelection::exact("index", "3") );
            let (request,rx) = LabelRequest::new(LabelCommand::Select(selector) );
            tx.send(request).await;
            let result = timeout( Duration::from_secs(5),rx).await.unwrap().unwrap();
            assert_result_count(result,1);


            let mut selector = Selectors::app_selector();
            selector.name("Highest".to_string()).unwrap();
            let (request,rx) = LabelRequest::new(LabelCommand::Select(selector) );
            tx.send(request).await;
            let result = timeout( Duration::from_secs(5),rx).await.unwrap().unwrap();
            assert_result_count(result,1);


        });
    }

    fn results( result:LabelResult )->Vec<Resource>
    {
        if let LabelResult::Resources(resources) = result
        {
            resources
        }
        else
        {
            assert!(false);
            vec!()
        }
    }


    fn assert_result_count( result: LabelResult, count: usize )
    {
        if let LabelResult::Resources(resources) = result
        {
            assert_eq!(resources.len(),count);
println!("PASS");
        }
        else if let LabelResult::Error(error) = result
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




