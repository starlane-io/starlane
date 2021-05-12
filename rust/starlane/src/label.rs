
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

pub struct Selector
{
    pub fields: HashSet<FieldSelection>,
    pub labels: HashSet<LabelSelection>
}

impl Selector {
    pub fn new()->Self {
        Selector {
            fields: HashSet::new(),
            labels: HashSet::new()
        }
    }

    pub fn and( &mut self, field: FieldSelection ) {
        self.fields.insert(field);
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
pub struct LabelSelection
{
    pub name: String,
    pub value: String
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
                let owner = bincode::serialize(&owner );
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
        }
    }
}

/*impl FieldSelection
{
    pub fn to_sql(&self)->Result<ToSqlOutput,Error>
    {
       Ok(match self
       {
           FieldSelection::Type(resource_type) => {
               ToSqlOutput::Owned(Value::Text(resource_type.to_string()))
           }
           FieldSelection::Kind(kind) => {
               ToSqlOutput::Owned(Value::Text(kind.to_string()))
           }
           FieldSelection::Specific(specific) => {
               ToSqlOutput::Owned(Value::Text(specific.to_string()))
           }
           FieldSelection::Owner(owner) => {
               let owner = bincode::serialize(&owner )?;
               ToSqlOutput::Owned(Value::Blob(owner))
           }
           FieldSelection::Space(space) => {
               ToSqlOutput::Owned(Value::Integer(space.index() as _))
           }
           FieldSelection::SubSpace(sub_space) => {
               ToSqlOutput::Owned(Value::Integer(sub_space.id.index() as _))
           }
       })
    }

}
 */

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
    Save(Resource,Labels),
    Select(Selector),
}

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
            if conn.is_err()
            {
                let mut db = LabelDb {
                    conn: conn.unwrap(),
                    rx: rx
                };
                tokio::spawn(async move {
                    db.run().await;
                });
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
            LabelCommand::Save(resource, labels) => {
                let key = bincode::serialize(&resource.key)?;
                let resource_type = format!("{}", &resource.key.resource_type());
                let kind = format!("{}", &resource.kind);
                let specific = match resource.specific {
                    None => Option::None,
                    Some(specific) => { Option::Some(specific.to_string()) }
                };

                let owner = bincode::serialize(&resource.owner)?;
                let space = resource.key.space().index();
                let sub_space = resource.key.sub_space().id.index();

                let trans = self.conn.transaction()?;
                trans.execute("DELETE FROM labels, resources, labels_to_resources WHERE resources.key=?1 AND resources.key=labels_to_resources.resource_key AND labels.key=labels_to_resources.label_key", [key.clone()])?;

                match specific
                {
                    None => {
                        trans.execute("INSERT INTO resources (key,type,kind,space,sub_space,owner) VALUES (?1,?2,?3,?4,?5,?6)", params![key.clone(),resource_type,kind,space,sub_space,owner])?;
                    }
                    Some(specific) => {
                        trans.execute("INSERT INTO resources (key,type,kind,specific,space,sub_space,owner) VALUES (?1,?2,?3,?4,?5,?6,?7)", params![key.clone(),resource_type,kind,specific,space,sub_space,owner])?;
                    }
                }

                for (name, value) in labels
                {
                    trans.execute("INSERT INTO resources (name,value) VALUES (?1,?2)", [name, value]);
                    trans.execute("INSERT INTO labels_to_resources (label_key,resource_key) VALUES (SELECT last_insert_rowid(),?2)", params![key]);
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
                    };
                    where_clause.push_str(f.as_str());
                    params.push(field);
                }

                let statement = format!(r#"SELECT resources.key,resources.kind,resources.specific,resources.owner
                              FROM resources,labels,labels_to_resources
                              WHERE {}"#, where_clause);

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

                    let owner:Vec<u8> = row.get(3)?;
                    let owner= bincode::deserialize::<UserKey>(owner.as_slice() )?;

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
	      name TEXT NOT NULL,
	      value TEXT NOT NULL,
        )"#;

      let resources = r#"CREATE TABLE IF NOT EXISTS resources (
         key BLOB PRIMARY KEY,
         type TEXT NOT NULL,
         kind BLOB NOT NULL,
         specific TEXT
         space INTEGER NOT NULL,
         sub_space INTEGER NOT NULL,
         owner BLOB
        )"#;

      let labels_to_resources = r#"CREATE TABLE IF NOT EXISTS labels_to_resources
        (
           resource_key BLOB,
           label_key INTEGER,
           kind: TEXT NOT NULL,
           specific: TEXT NOT NULL,
           PRIMARY KEY (resource_key, label_key),
           FOREIGN KEY (resource_key) REFERENCES resources (key),
           FOREIGN KEY (label_key) REFERENCES labels (key)
        )
        "#;

        let transaction = self.conn.transaction()?;
        transaction.execute(labels, [])?;
        transaction.execute(resources, [])?;
        transaction.execute(labels_to_resources, [])?;
        transaction.commit();

        Ok(())
    }
}




