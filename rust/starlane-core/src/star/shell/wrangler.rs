use std::collections::HashSet;
use std::iter::FromIterator;
use std::str::FromStr;
use std::sync::Arc;

use futures::TryFutureExt;
use rusqlite::{Connection, params, params_from_iter, ToSql};
use rusqlite::types::{ToSqlOutput, Value, ValueRef};
use tokio::sync::{mpsc, oneshot};
use tokio::time::Duration;

use crate::error::Error;
use crate::resource::{
    ResourceType,
};
use crate::star::{StarCommand, StarWrangleKind, StarInfo, StarKey, StarKind, StarSkel};
use crate::fail::Fail;

#[derive(Clone)]
pub struct StarWranglerApi {
    tx: mpsc::Sender<StarHandleAction>,
    star_tx: mpsc::Sender<StarCommand>,
}

impl StarWranglerApi {
    pub async fn new(star_tx: mpsc::Sender<StarCommand>) -> Self {
        StarWranglerApi {
            tx: StarWrangleDB::new().await,
            star_tx: star_tx,
        }
    }

    pub async fn add_star_handle(&self, handle: StarWrangle) -> Result<(), Error> {
        let (action, rx) = StarHandleAction::new(StarWrangleCall::SetStar(handle));
        self.tx.send(action).await?;
        tokio::time::timeout(Duration::from_secs(5), rx).await??;
        self.star_tx.send(StarCommand::CheckStatus).await;
        Ok(())
    }

    pub async fn select(&self, selector: StarSelector) -> Result<Vec<StarWrangle>, Error> {
        let (action, rx) = StarHandleAction::new(StarWrangleCall::Select(selector));
        self.tx.send(action).await?;
        let result = tokio::time::timeout(Duration::from_secs(5), rx).await??;
        match result {
            StarWrangleResult::StarWrangles(handles) => Ok(handles),
            _what => Err("Fail::expected(StarHandleResult::StarHandles(handles)".into()),
        }
    }

    pub async fn next(&self, selector: StarSelector) -> Result<StarWrangle, Error> {
        let (action, rx) = StarHandleAction::new(StarWrangleCall::Next(selector));
        self.tx.send(action).await?;
        let result = tokio::time::timeout(Duration::from_secs(5), rx).await??;
        match result {
            StarWrangleResult::StarWrangle(handle) => Ok(handle),
            _what => Err("Fail::expected(StarHandleResult::StarHandle(handle)".into()),
        }
    }

    // must have at least one of each StarKind
    pub async fn satisfied(&self, set: HashSet<StarWrangleKind>) -> Result<StarWrangleSatisfaction, Error> {
        let (action, rx) = StarHandleAction::new(StarWrangleCall::CheckSatisfaction(set));
        self.tx.send(action).await?;
        let result = tokio::time::timeout(Duration::from_secs(5), rx).await??;
        match result {
            StarWrangleResult::Satisfaction(satisfaction) => Ok(satisfaction),
            _what => Err("StarHandleResult::Satisfaction(_)".into()),
        }
    }
}

/*
#[derive(Debug, Clone)]
pub struct ResourceHostSelector {
    skel: StarSkel,
}

impl ResourceHostSelector {
    pub fn new(skel: StarSkel) -> Self {
        ResourceHostSelector { skel: skel }
    }

    pub async fn select(&self, resource_type: ResourceType) -> Result<Arc<dyn ResourceHost>, Error> {
        if StarKind::hosts(&resource_type) == self.skel.info.kind {
            let handle = StarWrangle {
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

            let mut selector = StarSelector::new();
            selector.add(StarFieldSelection::Kind(StarKind::hosts(&resource_type)));
            let handle = self.skel.star_wrangler_api.next(selector).await?;

            let host = RemoteResourceHost {
                skel: self.skel.clone(),
                handle: handle,
            };

            Ok(Arc::new(host))
        }
    }
}

 */

pub struct StarWrangle {
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
    pub command: StarWrangleCall,
    pub tx: oneshot::Sender<StarWrangleResult>,
}

impl StarHandleAction {
    pub fn new(command: StarWrangleCall) -> (Self, oneshot::Receiver<StarWrangleResult>) {
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
pub enum StarWrangleCall {
    Close,
    SetStar(StarWrangle),
    Select(StarSelector),
    Next(StarSelector),
    CheckSatisfaction(HashSet<StarWrangleKind>),
}

#[derive(strum_macros::Display)]
pub enum StarWrangleResult {
    Ok,
    StarWrangles(Vec<StarWrangle>),
    StarWrangle(StarWrangle),
    Fail(Error),
    Satisfaction(StarWrangleSatisfaction),
}

#[derive(Eq, PartialEq, Debug)]
pub enum StarWrangleSatisfaction {
    Ok,
    Lacking(HashSet<StarKind>),
}

pub struct StarWrangleDB {
    pub conn: Connection,
    pub rx: mpsc::Receiver<StarHandleAction>,
}

impl StarWrangleDB {
    pub async fn new() -> mpsc::Sender<StarHandleAction> {
        let (tx, rx) = mpsc::channel(8 * 1024);

        tokio::spawn(async move {
            let conn = Connection::open_in_memory();
            if conn.is_ok() {
                let mut db = StarWrangleDB {
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
            if let StarWrangleCall::Close = request.command {
                break;
            }
            match self.process(request.command).await {
                Ok(ok) => {
                    request.tx.send(ok);
                }
                Err(fail) => {
                    eprintln!("{}", fail.to_string());
                    request.tx.send(StarWrangleResult::Fail(fail));
                }
            }
        }
        Ok(())
    }

    async fn process(&mut self, command: StarWrangleCall) -> Result<StarWrangleResult, Error> {
        match command {
            StarWrangleCall::Close => {
                // this is handle in the run() method
                Ok(StarWrangleResult::Ok)
            }
            StarWrangleCall::SetStar(handle) => {
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

                Ok(StarWrangleResult::Ok)
            }
            StarWrangleCall::Select(selector) => {
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

                    let handle = StarWrangle {
                        key: key,
                        kind: kind,
                        hops: hops,
                    };

                    handles.push(handle);
                }
                Ok(StarWrangleResult::StarWrangles(handles))
            }
            StarWrangleCall::Next(selector) => {
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
                        let key = match StarKey::from_bin(key) {
                            Ok(key) => key,
                            Err(err) => {
                                error!("query row error when parsing StarKey: {}",err.to_string());
                                return Err(rusqlite::Error::InvalidQuery)
                            }
                        };

                        let kind: String = row.get(1)?;
                        let kind = StarKind::from_str(kind.as_str())
                            .map_err(|_| rusqlite::Error::InvalidQuery)?;

                        let hops = if let ValueRef::Null = row.get_ref(2)? {
                            Option::None
                        } else {
                            let hops: usize = row.get(2)?;
                            Option::Some(hops)
                        };

                        let handle = StarWrangle {
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
                                return Err(format!("could not select for: {}",
                                    selector.to_string()).into()
                                );
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

                Ok(StarWrangleResult::StarWrangle(handle))
            }

            StarWrangleCall::CheckSatisfaction(mut kinds) => {
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
                    Ok(StarWrangleResult::Satisfaction(StarWrangleSatisfaction::Ok))
                } else {
                    Ok(StarWrangleResult::Satisfaction(StarWrangleSatisfaction::Lacking(
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
