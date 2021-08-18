use std::collections::HashSet;
use std::iter::FromIterator;
use std::str::FromStr;
use std::sync::Arc;

use futures::TryFutureExt;
use rusqlite::types::{ToSqlOutput, Value, ValueRef};
use rusqlite::{params, params_from_iter, Connection, ToSql};
use tokio::sync::{mpsc, oneshot};

use tokio::time::Duration;

use crate::error::Error;

use crate::message::Fail;
use crate::resource::{
    RemoteResourceHost, ResourceAssign, ResourceHost, ResourceLocationAffinity, ResourceType,
};
use crate::star::{StarCommand, StarInfo, StarKey, StarKind, StarSkel, StarConscriptKind};

#[derive(Clone)]
pub struct StarWranglerBacking {
    tx: mpsc::Sender<StarHandleAction>,
    star_tx: mpsc::Sender<StarCommand>,
}

impl StarWranglerBacking {
    pub async fn new(star_tx: mpsc::Sender<StarCommand>) -> Self {
        StarWranglerBacking {
            tx: StarConscriptDB::new().await,
            star_tx: star_tx,
        }
    }

    pub async fn add_star_handle(&self, handle: StarConscript) -> Result<(), Fail> {
        let (action, rx) = StarHandleAction::new(StarConscriptCall::SetStar(handle));
        self.tx.send(action).await?;
        tokio::time::timeout(Duration::from_secs(5), rx).await??;
        self.star_tx.send(StarCommand::CheckStatus).await;
        Ok(())
    }

    pub async fn select(&self, selector: StarSelector) -> Result<Vec<StarConscript>, Fail> {
        let (action, rx) = StarHandleAction::new(StarConscriptCall::Select(selector));
        self.tx.send(action).await?;
        let result = tokio::time::timeout(Duration::from_secs(5), rx).await??;
        match result {
            StarConscriptResult::StarConscripts(handles) => Ok(handles),
            _what => Err(Fail::expected("StarHandleResult::StarHandles(handles)")),
        }
    }

    pub async fn next(&self, selector: StarSelector) -> Result<StarConscript, Fail> {
        let (action, rx) = StarHandleAction::new(StarConscriptCall::Next(selector));
        self.tx.send(action).await?;
        let result = tokio::time::timeout(Duration::from_secs(5), rx).await??;
        match result {
            StarConscriptResult::StarConscript(handle) => Ok(handle),
            _what => Err(Fail::expected("StarHandleResult::StarHandle(handle)")),
        }
    }

    // must have at least one of each StarKind
    pub async fn satisfied(&self, set: HashSet<StarConscriptKind>) -> Result<StarConscriptionSatisfaction, Fail> {
        let (action, rx) = StarHandleAction::new(StarConscriptCall::CheckSatisfaction(set));
        self.tx.send(action).await?;
        let result = tokio::time::timeout(Duration::from_secs(5), rx).await??;
        match result {
            StarConscriptResult::Satisfaction(satisfaction) => Ok(satisfaction),
            _what => Err(Fail::expected("StarHandleResult::Satisfaction(_)")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResourceHostSelector {
    skel: StarSkel,
}

impl ResourceHostSelector {
    pub fn new(skel: StarSkel) -> Self {
        ResourceHostSelector { skel: skel }
    }

    pub async fn select(&self, resource_type: ResourceType) -> Result<Arc<dyn ResourceHost>, Fail> {
        if StarKind::hosts(&resource_type) == self.skel.info.kind {
            let handle = StarConscript {
                key: self.skel.info.key.clone(),
                kind: self.skel.info.kind.clone(),
                hops: None,
            };
            let host = RemoteResourceHost {
                skel: self.skel.clone(),
                handle: handle,
            };
            Ok(Arc::new(host))
        } else {
            let handler = self.skel.star_handler.as_ref().ok_or(format!(
                "non-manager star {} does not have a host star selector",
                self.skel.info.kind.to_string()
            ))?;
            let mut selector = StarSelector::new();
            selector.add(StarFieldSelection::Kind(StarKind::hosts(&resource_type)));
            let handle = handler.next(selector).await?;

            let host = RemoteResourceHost {
                skel: self.skel.clone(),
                handle: handle,
            };

            Ok(Arc::new(host))
        }
    }
}

pub struct StarConscript {
    pub key: StarKey,
    pub kind: StarKind,
    pub hops: Option<usize>,
}

pub struct StarSelector {
    fields: HashSet<StarFieldSelection>,
}

impl ToString for StarSelector {
    fn to_string(&self) -> String {
        let mut rtn = String::new();

        for (index, field) in self.fields.iter().enumerate() {
            if index > 0 {
                rtn.push_str(", ");
            }
            rtn.push_str(field.to_string().as_str());
        }

        rtn
    }
}

impl StarSelector {
    pub fn new() -> Self {
        StarSelector {
            fields: HashSet::new(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    pub fn add(&mut self, field: StarFieldSelection) {
        self.fields.insert(field);
    }
}

#[derive(Clone, Hash, Eq, PartialEq)]
pub enum StarFieldSelection {
    Kind(StarKind),
    MinHops,
}

impl ToString for StarFieldSelection {
    fn to_string(&self) -> String {
        match self {
            StarFieldSelection::Kind(kind) => format!("Kind:{}", kind.to_string()),
            StarFieldSelection::MinHops => format!("MinHops"),
        }
    }
}

impl ToSql for StarFieldSelection {
    fn to_sql(&self) -> Result<ToSqlOutput<'_>, rusqlite::Error> {
        match self {
            StarFieldSelection::Kind(kind) => Ok(ToSqlOutput::Owned(Value::Text(kind.to_string()))),
            StarFieldSelection::MinHops => Ok(ToSqlOutput::Owned(Value::Null)),
        }
    }
}

impl StarFieldSelection {
    pub fn is_param(&self) -> bool {
        match self {
            StarFieldSelection::Kind(_) => true,
            StarFieldSelection::MinHops => false,
        }
    }
}

pub struct StarHandleAction {
    pub command: StarConscriptCall,
    pub tx: oneshot::Sender<StarConscriptResult>,
}

impl StarHandleAction {
    pub fn new(command: StarConscriptCall) -> (Self, oneshot::Receiver<StarConscriptResult>) {
        let (tx, rx) = oneshot::channel();
        (
            StarHandleAction {
                tx: tx,
                command: command,
            },
            rx,
        )
    }
}

#[derive(strum_macros::Display)]
pub enum StarConscriptCall {
    Close,
    SetStar(StarConscript),
    Select(StarSelector),
    Next(StarSelector),
    CheckSatisfaction(HashSet<StarConscriptKind>),
}

#[derive(strum_macros::Display)]
pub enum StarConscriptResult {
    Ok,
    StarConscripts(Vec<StarConscript>),
    StarConscript(StarConscript),
    Fail(Fail),
    Satisfaction(StarConscriptionSatisfaction),
}

#[derive(Eq, PartialEq, Debug)]
pub enum StarConscriptionSatisfaction {
    Ok,
    Lacking(HashSet<StarKind>),
}

pub struct StarConscriptDB {
    pub conn: Connection,
    pub rx: mpsc::Receiver<StarHandleAction>,
}

impl StarConscriptDB {
    pub async fn new() -> mpsc::Sender<StarHandleAction> {
        let (tx, rx) = mpsc::channel(8 * 1024);

        tokio::spawn(async move {
            let conn = Connection::open_in_memory();
            if conn.is_ok() {
                let mut db = StarConscriptDB {
                    conn: conn.unwrap(),
                    rx: rx,
                };
                db.run().await.unwrap()
            }
        });
        tx
    }

    async fn run(&mut self) -> Result<(), Error> {
        self.setup()?;

        while let Option::Some(request) = self.rx.recv().await {
            if let StarConscriptCall::Close = request.command {
                break;
            }
            match self.process(request.command).await {
                Ok(ok) => {
                    request.tx.send(ok);
                }
                Err(fail) => {
                    eprintln!("{}", fail.to_string());
                    request.tx.send(StarConscriptResult::Fail(fail));
                }
            }
        }
        Ok(())
    }

    async fn process(&mut self, command: StarConscriptCall) -> Result<StarConscriptResult, Fail> {
        match command {
            StarConscriptCall::Close => {
                // this is handle in the run() method
                Ok(StarConscriptResult::Ok)
            }
            StarConscriptCall::SetStar(handle) => {
                let key = handle.key.bin()?;
                let kind = handle.kind.to_string();

                let trans = self.conn.transaction()?;
                if handle.hops.is_some() {
                    trans.execute(
                        "REPLACE INTO stars (key,kind,hops) VALUES (?1,?2,?3)",
                        params![key, kind, handle.hops],
                    )?;
                } else {
                    trans.execute(
                        "REPLACE INTO stars (key,kind) VALUES (?1,?2)",
                        params![key, kind],
                    )?;
                }
                trans.commit()?;

                Ok(StarConscriptResult::Ok)
            }
            StarConscriptCall::Select(selector) => {
                let mut params = vec![];
                let mut where_clause = String::new();
                let mut param_index = 0;

                for (index, field) in Vec::from_iter(selector.fields.clone())
                    .iter()
                    .map(|x| x.clone())
                    .enumerate()
                {
                    if index != 0 {
                        where_clause.push_str(" AND ");
                    }

                    let f = match &field {
                        StarFieldSelection::Kind(_kind) => {
                            format!("kind=?{}", index + 1)
                        }
                        StarFieldSelection::MinHops => {
                            format!("hops NOT NULL AND hops=MIN(hops)")
                        }
                    };

                    where_clause.push_str(f.as_str());
                    if field.is_param() {
                        params.push(field);
                        param_index = param_index + 1;
                    }
                }

                // in case this search was for EVERYTHING
                let statement = if !selector.is_empty() {
                    format!(
                        "SELECT DISTINCT key,kind,hops  FROM stars WHERE {}",
                        where_clause
                    )
                } else {
                    "SELECT DISTINCT key,kind,hops  FROM stars".to_string()
                };

                let mut statement = self.conn.prepare(statement.as_str())?;
                let mut rows = statement.query(params_from_iter(params.iter()))?;

                let mut handles = vec![];
                while let Option::Some(row) = rows.next()? {
                    let key: Vec<u8> = row.get(0)?;
                    let key = StarKey::from_bin(key)?;

                    let kind: String = row.get(1)?;
                    let kind = StarKind::from_str(kind.as_str())?;

                    let hops = if let ValueRef::Null = row.get_ref(2)? {
                        Option::None
                    } else {
                        let hops: usize = row.get(2)?;
                        Option::Some(hops)
                    };

                    let handle = StarConscript {
                        key: key,
                        kind: kind,
                        hops: hops,
                    };

                    handles.push(handle);
                }
                Ok(StarConscriptResult::StarConscripts(handles))
            }
            StarConscriptCall::Next(selector) => {
                let mut params = vec![];
                let mut where_clause = String::new();
                let mut param_index = 0;

                for (index, field) in Vec::from_iter(selector.fields.clone())
                    .iter()
                    .map(|x| x.clone())
                    .enumerate()
                {
                    if index != 0 {
                        where_clause.push_str(" AND ");
                    }

                    let f = match &field {
                        StarFieldSelection::Kind(_kind) => {
                            format!("kind=?{}", index + 1)
                        }
                        StarFieldSelection::MinHops => {
                            format!("hops NOT NULL AND hops=MIN(hops)")
                        }
                    };

                    where_clause.push_str(f.as_str());
                    if field.is_param() {
                        params.push(field);
                        param_index = param_index + 1;
                    }
                }

                // in case this search was for EVERYTHING
                let statement = if !selector.is_empty() {
                    format!(
                        "SELECT DISTINCT key,kind,hops  FROM stars WHERE {} ORDER BY selections",
                        where_clause
                    )
                } else {
                    "SELECT DISTINCT key,kind,hops  FROM stars ORDER BY selections".to_string()
                };

                let trans = self.conn.transaction()?;

                let handle =
                    trans.query_row(statement.as_str(), params_from_iter(params.iter()), |row| {
                        let key: Vec<u8> = row.get(0)?;
                        let key = StarKey::from_bin(key)?;

                        let kind: String = row.get(1)?;
                        let kind = StarKind::from_str(kind.as_str())
                            .map_err(|_| rusqlite::Error::InvalidQuery)?;

                        let hops = if let ValueRef::Null = row.get_ref(2)? {
                            Option::None
                        } else {
                            let hops: usize = row.get(2)?;
                            Option::Some(hops)
                        };

                        let handle = StarConscript {
                            key: key,
                            kind: kind,
                            hops: hops,
                        };

                        Ok(handle)
                    });

                let handle = match handle {
                    Ok(handle) => handle,
                    Err(err) => {
                        match err {
                            rusqlite::Error::QueryReturnedNoRows => {
                                return Err(Fail::SuitableHostNotAvailable(format!(
                                    "could not select for: {}",
                                    selector.to_string()
                                )));
                            }
                            _ => {
                                return Err(err.to_string().into());
                            }
                        };
                    }
                };

                trans.execute(
                    "UPDATE stars SET selections=selections+1 WHERE key=?1",
                    params![handle.key.bin()?],
                )?;

                trans.commit()?;

                Ok(StarConscriptResult::StarConscript(handle))
            }

            StarConscriptCall::CheckSatisfaction(mut kinds) => {
                let mut lacking = HashSet::new();
                kinds.retain( |c| c.required );
                let kinds:Vec<StarKind> = kinds.iter().map(|c|c.kind.clone()).collect();

                for kind in kinds {
                    if !self.conn.query_row(
                        "SELECT count(*) AS count FROM stars WHERE kind=?1",
                        params![kind.to_string()],
                        |row| {
                            let count: usize = row.get(0)?;
                            return Ok(count > 0);
                        },
                    )? {
                        lacking.insert(kind);
                    }
                }
                if lacking.is_empty() {
                    Ok(StarConscriptResult::Satisfaction(StarConscriptionSatisfaction::Ok))
                } else {
                    Ok(StarConscriptResult::Satisfaction(StarConscriptionSatisfaction::Lacking(
                        lacking,
                    )))
                }
            }
        }
    }

    pub fn setup(&mut self) -> Result<(), Error> {
        let stars = r#"
       CREATE TABLE IF NOT EXISTS stars(
	      key BLOB PRIMARY KEY,
	      kind TEXT NOT NULL,
	      hops INTEGER,
	      selections INTEGER NOT NULL DEFAULT 0
        )"#;

        let transaction = self.conn.transaction()?;
        transaction.execute(stars, [])?;
        transaction.commit();

        Ok(())
    }
}
