use crate::util::{Call, AsyncRunner, AsyncProcessor};
use crate::star::{StarSkel, StarKey};
use crate::star::core::resource::host::HostCall;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use rusqlite::{Connection, params_from_iter, Row, Transaction};
use crate::error::Error;
use crate::mesh::serde::id::{Address, Kind, Specific};
use crate::mesh::serde::resource::command::common::{SetRegistry, SetProperties};
use crate::fail::Fail;
use crate::mesh::serde::payload::{Payload, Primitive};
use crate::mesh::serde::fail;
use crate::mesh::serde::resource::command::select::Select;
use crate::mesh::serde::resource::ResourceStub;
use crate::mesh::serde::pattern::Hop;
use crate::mesh::serde::pattern::Pattern;
use crate::mesh::serde::pattern::SegmentPattern;
use crate::mesh::serde::pattern::ExactSegment;
use crate::mesh::serde::id::Version;
use mesh_portal_serde::version::latest::generic::pattern::ExactSegment;
use mesh_portal_serde::version::v0_0_1::util::ValuePattern;
use mesh_portal_serde::version::v0_0_1::pattern::SpecificPattern;
use crate::resource::{ResourceRecord, ResourceLocation};
use rusqlite::types::ValueRef;
use std::str::FromStr;
use crate::mesh::serde::generic::resource::Archetype;
use futures::SinkExt;
use mesh_portal_serde::version::v0_0_1::generic::payload::PrimitiveList;

static RESOURCE_QUERY_FIELDS: &str = "parent,address_segment,resource_type,kind,vendor,product,variant,version,version_pre,host,status";

#[derive(Clone)]
pub struct RegistryApi {
    pub tx: mpsc::Sender<RegistryCall>,
}

impl RegistryApi {
    pub fn new(tx: mpsc::Sender<RegistryCall>) -> Self {
        Self { tx }
    }

    pub async fn register( &self, registration: Registration ) -> Result<(),Fail> {
        let (tx,rx) = oneshot::channel();
        self.tx.send(RegistryCall::Register {registration, tx });
        rx.await?
    }

    pub async fn select( &self, select: Select, address: Address) -> Result<Payload,Fail> {
        let (tx,rx) = oneshot::channel();
        self.tx.send(RegistryCall::Select{select, address, tx });
        rx.await?
    }

    pub async fn sub_select( &self, selector: SubSelector ) -> Result<Vec<ResourceStub>,Fail> {
        let (tx,rx) = oneshot::channel();
        self.tx.send(RegistryCall::SubSelect{selector, tx });
        rx.await?
    }
}

pub enum RegistryCall {
    Register{registration:Registration, tx: oneshot::Sender<Result<(),Fail>>},
    Select{select: Select, address: Address, tx: oneshot::Sender<Result<Payload,Fail>>},
    SubSelect { selector: SubSelector, tx: oneshot::Sender<Result<Vec<ResourceStub>,Fail>>},
}

impl Call for RegistryCall {}

pub struct RegistryComponent {
    skel: StarSkel,
    conn: Connection
}

impl RegistryComponent {
    pub fn start(skel: StarSkel, rx: mpsc::Receiver<RegistryCall>) {
        let mut conn = Connection::open_in_memory().expect("expected to get sqlite database connection");
        setup(&mut conn);

        AsyncRunner::new(
            Box::new(Self {
                skel: skel.clone(),
                conn
            }),
            skel.core_registry_api.tx.clone(),
            rx,
        );
    }
}

#[async_trait]
impl AsyncProcessor<RegistryCall> for RegistryComponent {
    async fn process(&mut self, call: RegistryCall) {
        match call {
            RegistryCall::Register { registration, tx } => {
                self.register(registration,tx);
            }
            RegistryCall::Select { select, address, tx} => {
                self.select(select, address, tx);
            }
            RegistryCall::SubSelect { selector, tx } => {
                self.sub_select(selector, tx);
            }
        }
    }
}

impl RegistryComponent {

    fn register( &mut self, registration: Registration, tx: oneshot::Sender<Result<(),Fail>>) {
        fn register( registry: &mut RegistryComponent, registration: Registration ) -> Result<(),Fail> {
            let params = RegistryParams::from_registration(&registration)?;
            trans.execute("INSERT INTO resources (address_segment,resource_type,kind,vendor,product,variant,version,version_pre,parent,status) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,'Pending')", params![params.address_segment,params.resource_type,params.kind,params.vendor,params.product,params.variant,params.version,params.version_pre,params.parent])?;

            fn properties( prefix: &str, properties: SetProperties, trans: &Transaction ) -> Result<(),Fail> {
                for (key, payload) in properties.iter() {
                    match payload {
                        Payload::Primitive(primitive) => {
                            match primitive {
                                Primitive::Text(text) => {
                                    trans.execute("INSERT INTO properties (address,key,value) VALUES (?1,?2,?3)", params![params.address,key.to_string(),text.to_string()])?;
                                }
                                Primitive::Address(address) => {
                                    trans.execute("INSERT INTO properties (address,key,value) VALUES (?1,?2,?3)", params![params.address,key.to_string(),address.to_string()])?;
                                }
                                found => {
                                    return Err(Fail::Fail(fail::Fail::Resource(fail::resource::Fail::Create(fail::resource::Create::InvalidProperty { expected: "Text|Address|PayloadMap".to_string(), found: found.primitive_type().to_string() }))));
                                }
                            }
                        }
                        Payload::Map(map) => {
                            let prefix = if prefix.len() == 0 {
                                key.clone()
                            } else {
                                format!("{}.{}",prefix,key)
                            };
                            properties( prefix.as_str(), map, &trans)?;
                        }
                        found => {
                            return Err(Fail::Fail(fail::Fail::Resource(fail::resource::Fail::Create(fail::resource::Create::InvalidProperty { expected: "Text|Address|PayloadMap".to_string(), found: found.payload_type().to_string() }))));
                        }
                    }
                }
                Ok(())
            }

            properties( "", registration.properties, &trans )?;

            trans.commit()?;
            Ok(())
        }

        tx.send(register( self, registration ));
    }

    fn select(&mut self, select: Select, address: Address, tx: oneshot::Sender<Result<Payload,Fail>>) {
        async fn sub_select(registry: &mut RegistryComponent, selector: Select ) -> Result<Payload, Fail> {
            if address != selector.address_pattern.query_root() {
                let fail = fail::Fail::Resource(fail::resource::Fail::Select(fail::resource::Select::WrongAddress {required:selector.address_pattern.query_root(), found: address }));
                return Err(Fail::Fail(fail));
            }
            let resource = registry.skel.resource_locator_api.locate(selector.address_pattern.query_root()).await?;
            if resource.location.host != registry.skel.info.key {
                let fail = fail::Fail::Resource(fail::resource::Fail::Select(fail::resource::Select::BadSelectRouting {required:resource.location.host.to_string(), found: registry.skel.info.key.to_string()}));
                return Err(Fail::Fail(fail));
            }

            let sub_selector = SubSelector{
                address,
                hops: selector.address_pattern.sub_select_hops()
            };

            let stubs = registry.skel.core_registry_api.sub_select(sub_selector).await?;

            let rtn  = selector.into_payload.to_primitive(stubs)?;

            let rtn = Payload::List(rtn);

            Ok(rtn)
        }
    }



            fn sub_select(&mut self, selector: SubSelector, tx: oneshot::Sender<Result<Vec<ResourceStub>,Fail>>) {
        fn sub_select(registry: &mut RegistryComponent, selector: SubSelector) -> Result<Vec<ResourceStub>,Fail> {
            let mut params: Vec<String> = vec![];
            let mut where_clause = String::new();
            let mut index = 0;
            if let Option::Some(hop) = selector.hops.first()
            {
                match &hop.segment {
                    SegmentPattern::Exact(exact) => {
                        index = index+1;
                        where_clause.push_str( format!("address_segment=?{}",index).as_str() );
                        match exact {
                            ExactSegment::Address(address) => {
                                params.push( address.to_string() );
                            }
                        }
                    }
                    _ => {}
                }

                match &hop.tks.resource_type {
                    Pattern::Any => {},
                    Pattern::Exact(resource_type)=> {
                        if index > 0 {
                            where_clause.push_str( " AND ");
                        }
                        index = index+1;
                        where_clause.push_str( format!("resource_type=?{}",index).as_str() );
                        params.push( resource_type.to_string() );
                    },
                }

                match &hop.tks.kind {
                    Pattern::Any => {},
                    Pattern::Exact(kind)=> {
                        match kind.sub_string() {
                            None => {}
                            Some(sub) => {
                                if index > 0 {
                                    where_clause.push_str( " AND ");
                                }
                                index = index+1;
                                where_clause.push_str(format!("kind=?{}", index).as_str());
                                params.push(sub);
                            }
                        }
                    }
                }

                match &hop.tks.specific {
                    ValuePattern::Any => {}
                    ValuePattern::None => {}
                    ValuePattern::Pattern(specific) => {
                        match &specific.vendor {
                            Pattern::Any => {}
                            Pattern::Exact(vendor) => {
                                if index > 0 {
                                    where_clause.push_str( " AND ");
                                }
                                index = index+1;
                                where_clause.push_str(format!("vendor=?{}", index).as_str());
                                params.push(vendor.clone() );
                            }
                        }
                        match &specific.product{
                            Pattern::Any => {}
                            Pattern::Exact(product) => {
                                if index > 0 {
                                    where_clause.push_str( " AND ");
                                }
                                index = index+1;
                                where_clause.push_str(format!("product=?{}", index).as_str());
                                params.push(product.clone() );
                            }
                        }
                        match &specific.variant{
                            Pattern::Any => {}
                            Pattern::Exact(variant) => {
                                if index > 0 {
                                    where_clause.push_str( " AND ");
                                }
                                index = index+1;
                                where_clause.push_str(format!("variant=?{}", index).as_str());
                                params.push(variant.clone());
                            }
                        }
                    }
                }
            }

            let statement = format!(
                "SELECT DISTINCT {} FROM resources as r WHERE {}",
               RESOURCE_QUERY_FIELDS, where_clause
            );

            let mut statement = registry.conn.prepare(statement.as_str())?;
            let mut rows = statement.query(params_from_iter(params.iter()))?;

            fn process_resource_row_catch(row: &Row) -> Result<ResourceRecord, Error> {
                match Self::process_resource_row(row) {
                    Ok(ok) => Ok(ok),
                    Err(error) => {
                        eprintln!("process_resource_rows: {}", error);
                        Err(error)
                    }
                }
            }


            let mut resources = vec![];
            while let Option::Some(row) = rows.next()? {
                resources.push(process_resource_row_catch(row)?);
            }

            // next IF there are more hops, must coordinate with possible other stars...


            Ok(vec![])
        }

        tx.send(sub_select(self, selector));
    }

//    static RESOURCE_QUERY_FIELDS: &str = "parent,address_segment,resource_type,kind,vendor,product,variant,version,version_pre,host,status";
    fn process_resource_row(row: &Row) -> Result<ResourceRecord, Error> {

        fn opt( row: &Row, index: usize ) -> Result<Option<String>,Error>
        {
            if let ValueRef::Null = row.get_ref(index)? {
                Ok(Option::None)
            } else {
                let specific: String = row.get(index)?;
                Ok(Option::Some(specific))
            }
        }

        let parent : String = row.get(1)?;
        let address_segment:String = row.get(2)?;
        let resource_type:String = row.get(3)?;
        let kind: Option<String> = opt(row,4)?;
        let vendor: Option<String> = opt(row,5)?;
        let product: Option<String> = opt(row,6)?;
        let variant: Option<String> = opt(row,7)?;
        let version: Option<String> = opt(row,8)?;
        let version_pre: Option<String> = opt(row,9)?;
        let host: Option<String> = opt(row,10)?;
        let status: String = row.get(11)?;

        let address = Address::from_str(parent.as_str())?;
        let address = address.push( address_segment )?;
        let resource_type = ResourceType::from_str(resource_type);
        let specific = if let Option::Some(vendor) = vendor {
            if let Option::Some(product) = product {
                if let Option::Some(variant) = variant {
                    if let Option::Some(version) = version {
                        let version = if let Option::Some(version_pre) = version_pre{
                            let version =  format!("{}-{}",version,version_pre);
                            Version::from_str(version)?
                        } else {
                            Version::from_str(version)?
                        };

                        Option::Some(Specific {
                            vendor,
                            product,
                            variant,
                            version
                        })
                    } else {
                        Option::None
                    }
                } else {
                    Option::None
                }
            } else {
                Option::None
            }
        } else {
            Option::None
        };

        let kind = Kind::from( resource_type, kind, specific)?;
        let host = StarKey::from_str(host)?;
        let status = Status::from_str(status)?;

        let stub = ResourceStub {
            address,
            kind,
            properties: Default::default(), // not implemented yet...
            status
        };

        let record = ResourceRecord {
            stub: stub,
            location: ResourceLocation { host: host },
        };

        Ok(record)
    }

}




pub struct SubSelector {
    pub address: Address,
    pub hops: Vec<Hop>
}



pub struct Registration {
    pub address: Address,
    pub kind: Kind,
    pub registry: SetRegistry,
    pub properties: SetProperties
}

struct RegistryParams {
    address_segment: String,
    resource_type: String,
    kind: Option<String>,
    vendor: Option<String>,
    product: Option<String>,
    variant: Option<String>,
    version: Option<String>,
    version_pre: Option<String>,
    parent: String,
}

impl RegistryParams {
    pub fn from_registration(registration: &Registration ) -> Result<Self, Error> {

        let address_segment = match registration.address.segments.last() {
            None => {"".to_string()}
            Some(segment) => {
                segment.to_string()
            }
        };
        let parent = match registration.address.parent()  {
            None => {"".to_string()}
            Some(parent) => {parent.to_string()}
        };

        let resource_type = registration.kind.resource_type().to_string();
        let kind = registration.kind.sub_string();

        let vendor = match &registration.kind.specific() {
            None => Option::None,
            Some(specific) => Option::Some(specific.vendor.clone()),
        };

        let product= match &registration.kind.specific() {
            None => Option::None,
            Some(specific) => Option::Some(specific.product.clone()),
        };

        let variant = match &registration.kind.specific() {
            None => Option::None,
            Some(specific) => Option::Some(specific.variant.clone()),
        };

        let version = match &registration.kind.specific() {
            None => Option::None,
            Some(specific) =>  {
                let version = &specific.version;
                Option::Some(format!( "{}.{}.{}", version.major, version.minor, version.patch ))
            }
        };

        let version_pre = match &registration.kind.specific() {
            None => Option::None,
            Some(specific) =>  {
                let version = &specific.version;
                if version.is_prerelease() {
                    let mut pre = String::new();
                    for (i, x) in version.pre.iter().enumerate() {
                        if i != 0 {
                            result.push_str(".");
                        }
                        result.push_str(format!("{}", x).as_ref());
                    }
                    Option::Some(pre)
                } else {
                    Option::None
                }
            }
        };

        Ok(RegistryParams {
            address_segment,
            parent,
            resource_type,
            kind,
            vendor,
            product,
            variant,
            version,
            version_pre
        })
    }
}





pub fn setup(conn: &mut Connection) -> Result<(), Error> {


    let resources = r#"CREATE TABLE IF NOT EXISTS resources (
         id INTEGER PRIMARY KEY AUTOINCREMENT,
         address_segment TEXT NOT NULL,
         parent TEXT NOT NULL,
         resource_type TEXT NOT NULL,
         kind TEXT NOT NULL,
         vendor TEXT,
         product TEXT,
         variant TEXT,
         version TEXT,
         version_variant TEXT,
         host TEXT,
         status TEXT NOT NULL,
         UNIQUE(parent,address_segment)
        )"#;

    let labels = r#"
       CREATE TABLE IF NOT EXISTS labels (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
	      resource_id INTEGER NOT NULL,
	      key TEXT NOT NULL,
	      value TEXT,
          UNIQUE(key,value),
          FOREIGN KEY (resource_id) REFERENCES resources (id)
        )"#;

    /// note that a tag may reference an address NOT in this database
    /// therefore it does not have a FOREIGN KEY constraint
    let tags = r#"
       CREATE TABLE IF NOT EXISTS tags(
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          parent TEXT NOT NULL,
          tag TEXT NOT NULL,
          address TEXT NOT NULL,
          UNIQUE(tag)
        )"#;


    let properties = r#"CREATE TABLE IF NOT EXISTS properties (
         id INTEGER PRIMARY KEY AUTOINCREMENT,
	     resource_id INTEGER NOT NULL,
         address TEXT NOT NULL,
         key TEXT NOT NULL,
         value TEXT NOT NULL,
         FOREIGN KEY (resource_id) REFERENCES resources (id),
         UNIQUE(address,key)
        )"#;

    let address_index = "CREATE UNIQUE INDEX resource_address_index ON resources(address)";

    let transaction = conn.transaction()?;
    transaction.execute(labels, [])?;
    transaction.execute(tags, [])?;
    transaction.execute(resources, [])?;
    transaction.execute(properties, [])?;
    transaction.execute(address_index, [])?;
    transaction.commit()?;

    Ok(())
}


