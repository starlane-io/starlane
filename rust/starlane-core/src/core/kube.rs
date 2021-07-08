use std::collections::HashSet;
use std::convert::{TryFrom, TryInto};
use std::iter::FromIterator;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use rusqlite::Connection;
use tokio::sync::{mpsc, Mutex};

use crate::core::Host;
use crate::error::Error;
use crate::file_access::{FileAccess, FileEvent};
use crate::keys::{FileSystemKey, ResourceKey};
use crate::message::Fail;
use crate::resource::store::{
    ResourceStore, ResourceStoreAction, ResourceStoreCommand, ResourceStoreResult,
};


use crate::resource::{
    AddressCreationSrc, ArtifactBundleKind, AssignResourceStateSrc, DataTransfer, FileKind,
    KeyCreationSrc, MemoryDataTransfer, Path, RemoteDataSrc, Resource, ResourceAddress,
    ResourceArchetype, ResourceAssign, ResourceCreate, ResourceCreateStrategy,
    ResourceCreationChamber, ResourceIdentifier, ResourceKind, ResourceStateSrc, ResourceStub,
    ResourceType,
};
use crate::star::StarSkel;

use crate::artifact::ArtifactBundleKey;
use crate::util;
use std::fs;
use std::fs::File;
use std::io::Write;
use tempdir::TempDir;
use serde::{Serialize,Deserialize};
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::{Api, Client, Config};
use k8s_openapi::api::core::v1::Pod;
use kube::api::ListParams;
use kube::client::ConfigExt;
use hyper_native_tls::NativeTlsClient;
use hyper_tls::native_tls::TlsConnector;

pub struct KubeCore {
    skel: StarSkel,
    store: ResourceStore,
}

impl KubeCore {
    pub async fn new(skel: StarSkel) -> Result<Self, Error> {

        let rtn = KubeCore {
            skel: skel,
            store: ResourceStore::new().await
        };

        Ok(rtn)
    }
}


#[async_trait]
impl Host for KubeCore {
    async fn assign(
        &mut self,
        assign: ResourceAssign<AssignResourceStateSrc>,
    ) -> Result<Resource, Fail> {

println!("CREATE KIND: {}", assign.stub.archetype.kind.to_string() );
        let config = Config::infer().await?;
println!("CONFIG: cluster url {}", config.cluster_url.to_string()) ;



//        let client = Client::try_default().await?;



        let config = Config::infer().await?;

        /*
        let mut https = hyper_tls::HttpsConnector::new();
        let client = hyper::Client::builder()
            .build::<_, hyper::Body>(https);

         */

        let client = kube::Client::try_default().await?;


        let pods: Api<Pod> = Api::default_namespaced(client);
        let pods = pods.list(&ListParams::default()).await?;

        for pod in pods {
            println!("POD: {}", pod.metadata.name.unwrap() )
        }



        let data_transfer: Arc<dyn DataTransfer> = Arc::new(MemoryDataTransfer::none());

        let assign = ResourceAssign {
            stub: assign.stub.clone(),
            state_src: data_transfer,
        };

        let resource = self.store.put(assign).await?;
        Ok(resource)
    }

    async fn get(&self, identifier: ResourceIdentifier) -> Result<Option<Resource>, Fail> {
        self.store.get(identifier).await
    }

    async fn state(&self, identifier: ResourceIdentifier) -> Result<RemoteDataSrc, Fail> {
        if let Ok(Option::Some(resource)) = self.store.get(identifier.clone()).await {
            Ok(RemoteDataSrc::None)
        } else {
            Err(Fail::ResourceNotFound(identifier))
        }
    }

    async fn delete(&self, identifier: ResourceIdentifier) -> Result<(), Fail> {
        unimplemented!("I don't know how to DELETE yet.");
        Ok(())
    }
}



#[derive(kube::CustomResource, Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
#[kube(group = "starlane.starlane.io", version = "v1alpha1", kind = "Starlane", namespaced)]
struct StarlaneSpec{
}



#[derive(kube::CustomResource, Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
#[kube(group = "starlane.starlane.io", version = "v1alpha1", kind = "StarlaneResource", namespaced)]
struct StarlaneResourceSpec{
    pub snakeKey: String,
    pub address: String,
    pub provisioner: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub createArgs: Option<Vec<String>>,
}

#[derive(kube::CustomResource, Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
#[kube(group = "starlane.starlane.io", version = "v1alpha1", kind = "StarlaneProvisioner", namespaced)]
struct StarlaneProvisionerSpec{

}




















