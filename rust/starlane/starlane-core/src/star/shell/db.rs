use std::collections::HashSet;
use std::iter::FromIterator;
use std::str::FromStr;
use std::sync::Arc;

use sqlx::postgres::{PgArguments, PgColumn, PgPoolOptions, PgRow};
use sqlx::{Connection, Executor, Pool, Postgres, Row, Transaction};
use sqlx::error::DatabaseError;
use cosmic_api::version::v0_0_1::id::StarKey;
use crate::databases::lookup_db_for_star;
use crate::error;
use crate::error::Error;
use crate::star::{StarKind, StarWrangleKind};


pub type StarDBApi = Arc<StarDB>;

pub struct StarDB {
    key: StarKey,
    schema: String,
    pool: Pool<Postgres>,
}

impl StarDB {
    pub async fn new(key: StarKey) -> Result<Self, Error> {
        let db = lookup_db_for_star(&key);
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(db.to_uri().as_str())
            .await?;
        let schema = key.to_sql_name();
        let star_db = Self { key, schema, pool };

        match star_db.setup().await {
            Ok(_) => {
                debug!("star_db setup complete.");
            }
            Err(err) => {
                let message = err.into_database_error().unwrap().message().to_string();
                error!("database setup failed {} ", message);
                return Err(message.into());
            }
        }

        Ok(star_db)
    }

    async fn setup(&self) -> Result<(), sqlx::Error> {
        let schema = format!("CREATE SCHEMA IF NOT EXISTS {}", self.key.to_sql_name());

        let wrangles = format!(
            r#"
       CREATE TABLE IF NOT EXISTS {}.wrangles(
	      key  TEXT PRIMARY KEY,
	      kind TEXT NOT NULL,
	      hops INTEGER NOT NULL,
	      selections INTEGER NOT NULL DEFAULT 0
        )"#,
            self.schema
        );

        let mut conn = self.pool.acquire().await?;
        let mut transaction = conn.begin().await?;
        transaction.execute(schema.as_str()).await?;
        transaction.execute(wrangles.as_str()).await?;

        transaction.commit().await?;

        Ok(())
    }

    async fn nuke(&self) -> Result<(), Error> {
        let mut conn = self.pool.acquire().await?;
        let mut trans = conn.begin().await?;
        trans
            .execute(format!("DROP SCHEMA {} CASCADE", self.schema).as_str())
            .await;
        trans.commit().await?;
        self.setup().await?;
        Ok(())
    }

    pub async fn set_wrangle(&self, wrangle: StarWrangle) -> anyhow::Result<()> {
        let key = wrangle.key.to_string();
        let kind = wrangle.kind.to_string();

        let mut conn = self.pool.acquire().await?;
        let mut trans = conn.begin().await?;

        let statement = format!("INSERT INTO {}.wrangles (key,kind,hops) VALUES ('{}','{}','{}') ON CONFLICT (key) DO UPDATE SET kind='{}', hops='{}'", self.schema, key, kind, wrangle.hops, kind, wrangle.hops );
        trans.execute(statement.as_str()).await?;

        trans.commit().await?;

        Ok(())
    }

    pub async fn select(&self, selector: StarSelector) -> anyhow::Result<Vec<StarWrangle>> {
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
                    format!("hops=MIN(hops)")
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
                "SELECT DISTINCT key,kind,hops  FROM {}.wrangles WHERE {}",
                self.schema, where_clause
            )
        } else {
            format!(
                "SELECT DISTINCT key,kind,hops  FROM {}.wrangles",
                self.schema
            )
        };

        let mut conn = self.pool.acquire().await?;
        let wrangles = sqlx::query_as::<Postgres, StarWrangle>(statement.as_str())
            .fetch_all(&mut conn)
            .await?;

        Ok(wrangles)
    }

    pub async fn next_wrangle(&self, selector: StarSelector) -> anyhow::Result<StarWrangle> {
        let mut where_clause = String::new();

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
                    format!("kind='{}'", _kind.to_string() )
                }
                StarFieldSelection::MinHops => {
                    format!("hops=MIN(hops)")
                }
            };

            where_clause.push_str(f.as_str());
        }

        // in case this search was for EVERYTHING
        let statement = if !selector.is_empty() {
            format!(
                "SELECT key,kind,hops  FROM {}.wrangles WHERE {} ORDER BY selections",
                self.schema, where_clause
            )
        } else {
            format!("SELECT key,kind,hops  FROM {}.wrangles ORDER BY selections", self.schema)
        };
        let mut conn = self.pool.acquire().await?;
        let mut trans = conn.begin().await?;
        let wrangle = sqlx::query_as::<Postgres, StarWrangle>(statement.as_str())
            .fetch_one(&mut trans)
            .await?;

        trans.execute(
            format!("UPDATE {}.wrangles SET selections=selections+1 WHERE key='{}'",
            self.schema,wrangle.key.to_string()).as_str()
        );

        trans.commit().await?;

        Ok(wrangle)
    }

    pub async fn wrangle_satisfaction(
        &self,
        mut kinds: HashSet<StarWrangleKind>,
    ) -> anyhow::Result<StarWrangleSatisfaction> {

        let mut lacking: HashSet<StarKind> = kinds.iter().filter(|wk|wk.required).map(|wk|wk.kind.clone()).collect();

        let mut conn = self.pool.acquire().await?;
        let wrangles = sqlx::query_as::<Postgres, WrangleCount>(format!("SELECT kind,count(*) as count FROM {}.wrangles group by kind",self.schema).as_str())
            .fetch_all(&mut conn)
            .await?;

        let wrangles : Vec<StarKind> = wrangles.into_iter().map(|w|w.kind).collect();

        lacking.retain( |k| !wrangles.contains(k));

        if lacking.is_empty() {
            Ok(StarWrangleSatisfaction::Ok)
        } else {
            Ok( StarWrangleSatisfaction::Lacking(lacking) )
        }
    }
}

impl sqlx::FromRow<'_, PgRow> for WrangleCount{
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        let kind: String = row.get(0);
        let count: i64 = row.get(1);
        let count: u32 = count.abs() as u32;
        let kind = match StarKind::from_str(kind.as_str()) {
            Ok(kind) => kind,
            Err(_) => {
                return Err(sqlx::Error::RowNotFound)
            }

        };
        Ok(Self {  kind, count })
    }
}

impl sqlx::FromRow<'_, PgRow> for StarWrangle {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        let key: String = row.get(0);
        let kind: String = row.get(1);
        let hops: i32= row.get(2);
        let hops = hops.abs() as u32;

        let key = match StarKey::from_str(key.as_str()) {
            Ok(key) => key,
            Err(_) => {
                return Err(sqlx::Error::RowNotFound)
            }
        };
        let kind = match StarKind::from_str(kind.as_str()) {
            Ok(kind) => kind,
            Err(_) => {
                return Err(sqlx::Error::RowNotFound)
            }

        };
        Ok(Self { key, kind, hops: hops as usize })
    }
}

#[cfg(test)]
pub mod test {

    #[test]
    pub async fn test() {

    }

}


pub struct StarWrangle {
    pub key: StarKey,
    pub kind: StarKind,
    pub hops: usize,
}

pub struct WrangleCount{
    pub kind: StarKind,
    pub count: u32,
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


impl StarFieldSelection {
    pub fn is_param(&self) -> bool {
        match self {
            StarFieldSelection::Kind(_) => true,
            StarFieldSelection::MinHops => false,
        }
    }
}

#[derive(Eq, PartialEq, Debug)]
pub enum StarWrangleSatisfaction {
    Ok,
    Lacking(HashSet<StarKind>),
}
