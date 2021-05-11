
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::error::Error;
use tokio::sync::{mpsc, oneshot};
use rusqlite::Connection;

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
    Close
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

        while let Option::Some(r)= self.rx.recv().await{
        }

        Ok(())
    }

    pub fn setup(&self)->Result<(),Error>
    {
      let setup = r#"
       CREATE TABLE labels (
	      id INTEGER PRIMARY KEY,
	      name TEXT NOT NULL,
	      value TEXT NOT NULL
        ) [WITHOUT ROWID];

       CREATE TABLE resources (
         id TEXT PRIMARY KEY,
         type INTEGER NOT NULL,
         kind TEXT
        ) [WITHOUT ROWID];

        CREATE TABLE labels_to_resources
        {
           resource_id BLOB,
           label_id INTEGER,
           type: TEXT NOT NULL,
           kind: TEXT NOT NULL,
           PRIMARY KEY (resource_id, label_id),
           FOREIGN KEY (resource_id)
              REFERENCES resources (id)
                  ON DELETE CASCADE,
                  ON UPDATE NO ACTION,
           FOREIGN KEY (label_id)
              REFERENCES labels (id)
                  ON DELETE CASCADE,
                  ON UPDATE NO ACTION,
        }  [WITHOUT ROWID];
        "#;

        self.conn.execute(setup, [])?;

        Ok(())
    }
}




