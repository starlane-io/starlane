
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::error::Error;
use tokio::sync::{mpsc, oneshot};
use crate::keys::{ResourceKey, ResourceType, UserKey, AppKey, Resource};
use crate::artifact::Name;
use rusqlite::{Connection,params};

pub type Labels = HashMap<String,String>;

#[derive(Clone,Serialize,Deserialize)]
pub struct LabelConfig
{
    pub name: String,
    pub index: bool
}

#[derive(Clone,Serialize,Deserialize)]
pub struct UniqueLabelConstraint
{
    pub labels: Vec<String>
}

#[derive(Clone,Serialize,Deserialize)]
pub enum LabelSelectionCriteria
{
    Exact(ExactLabelSelectionCriteria),
    Regex(RegexLabelSelectionCriteria)
}

#[derive(Eq,PartialEq,Clone,Serialize,Deserialize)]
pub struct ExactLabelSelectionCriteria
{
    pub name: String,
    pub value: String
}

impl ExactLabelSelectionCriteria
{
    pub fn new( name: String, value: String )->Self
    {
        ExactLabelSelectionCriteria{
            name: name,
            value: value
        }
    }
}


#[derive(Eq,PartialEq,Clone,Serialize,Deserialize)]
pub struct RegexLabelSelectionCriteria
{
    pub name: String,
    pub pattern: String
}

impl RegexLabelSelectionCriteria
{
    pub fn new(name: String, pattern: String ) ->Self
    {
        RegexLabelSelectionCriteria{
            name: name,
            pattern: pattern
        }
    }
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
}

pub enum LabelResult
{
    Ok,
    Error(String)
}

pub struct LabelDb {
   pub conn: Connection,
   pub rx: mpsc::Receiver<LabelRequest>
}

impl LabelDb {

    pub async fn new()-> mpsc::Sender<LabelRequest>
    {
        let (tx,rx) = mpsc::channel(8*1024 );

        tokio::spawn( async move {

            let conn = Connection::open_in_memory();
            if conn.is_err()
            {
                let mut db = LabelDb{
                    conn: conn.unwrap(),
                    rx: rx
                };
                tokio::spawn( async move {
                    db.run().await;
                });
            }

        } );
        tx
    }

    async fn run(&mut self)->Result<(),Error>
    {
        self.setup()?;

        while let Option::Some(request)= self.rx.recv().await{
            match request.command
            {
                LabelCommand::Close => {
                    break;
                }
                LabelCommand::Save(resource,labels) => {
                    let key = bincode::serialize(&resource.key ).unwrap();
                    let resource_type = format!("{}", &resource.key.rtype() );
                    let kind = format!("{}", &resource.kind );
                    let specific = match resource.specific{
                        None => Option::None,
                        Some(specific) => { Option::Some(specific.to_string()) }
                    };

                    let owner = bincode::serialize(&resource.owner ).unwrap();
                    let space = resource.key.space().index();
                    let sub_space = resource.key.sub_space().id.index();

                    let trans = self.conn.transaction()?;
                    trans.execute("DELETE FROM labels, resources, labels_to_resources WHERE resources.key=?1 AND resources.key=labels_to_resources.resource_key AND labels.key=labels_to_resources.label_key", [key.clone()]);

                    match specific
                    {
                        None => {
                            trans.execute( "INSERT INTO resources (key,type,kind,space,sub_space,owner) VALUES (?1,?2,?3,?4,?5,?6)", params![key.clone(),resource_type,kind,space,sub_space,owner] );
                        }
                        Some(specific) => {
                            trans.execute( "INSERT INTO resources (key,type,kind,specific,space,sub_space,owner) VALUES (?1,?2,?3,?4,?5,?6,?7)", params![key.clone(),resource_type,kind,specific,space,sub_space,owner] );
                        }
                    }

                    for (name,value) in labels
                    {
                        trans.execute( "INSERT INTO resources (name,value) VALUES (?1,?2)", [name,value] );
                        trans.execute( "INSERT INTO labels_to_resources (label_key,resrouce_key) VALUES (SELECT last_insert_rowid(),?2)", params![key] );
                    }

                    trans.commit();
                }
            }
        }

        Ok(())
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




