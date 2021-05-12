
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::error::Error;
use tokio::sync::{mpsc, oneshot};
use rusqlite::Connection;
use crate::keys::{ResourceKey, ResourceType, UserKey, AppKey, Resource};
use crate::artifact::Name;

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
                    let resource_key = bincode::serialize(&resource.key ).unwrap();
                    let trans = self.conn.transaction()?;
                    trans.execute("DELETE FROM labels, resources, labels_to_resources WHERE resources.key=?1 AND resources.key=labels_to_resources.resource_key AND labels.key=labels_to_resources.label_key", [resource_key]).unwrap();

                    trans.execute( "INSERT INTO resources (key,) VALUES ()", [] );

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




