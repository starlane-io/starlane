use std::env;

use std::sync::Arc;

use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ObjectMeta, OwnerReference};
use kube::api::{ListParams, PostParams};
use kube::Api;

use serde::{Deserialize, Serialize};

use starlane_resources::{ResourceIdentifier, ResourceKindParts};

use crate::error::Error;

use crate::message::Fail;
use crate::resource::{
    AddressCreationSrc, AssignResourceStateSrc, KeyCreationSrc, Path, RemoteDataSrc, Resource,
    ResourceAddress, ResourceArchetype, ResourceAssign, ResourceCreate, ResourceCreateStrategy,
    ResourceCreationChamber, ResourceKey, ResourceKind, ResourceStub,
};

use crate::data::{BinSrc, DataSet};
use crate::star::StarSkel;

/*

pub struct KubeCore {
    skel: StarSkel,
    client: kube::Client,
    starlane_meta: ObjectMeta,
    namespace: String,
    api_version: String
}

impl KubeCore {
    pub async fn new(skel: StarSkel) -> Result<Self, Error> {

        let client = kube::Client::try_default().await?;

        let kubernetes_instance_name = match env::var("STARLANE_KUBERNETES_INSTANCE_NAME"){
            Ok(kubernetes_instance_name) => {kubernetes_instance_name}
            Err(_err) => {
                error!("FATAL: env variable 'STARLANE_KUBERNETES_INSTANCE_NAME' must be set to a valid Starlane Kubernetes resource");
                return Err("FATAL: env variable 'STARLANE_KUBERNETES_INSTANCE_NAME' must be set to a valid Starlane Kubernetes resource".into());
            }
        };

        let namespace = match env::var("NAMESPACE"){
            Ok(namespace) => {namespace}
            Err(_err) => {
                warn!("NAMESPACE environment variable is not set, defaulting to 'default'");
                "default".to_string()
            }
        };

        let starlane_api: Api<crate::star::core::resource::host::kube::Starlane> = Api::namespaced(client.clone(), namespace.as_str() );
        let starlane: crate::star::core::resource::host::kube::Starlane =  match starlane_api.get(kubernetes_instance_name.as_str()).await {
            Ok(starlane) => starlane,
            Err(_err) => {
                let message = format!("FATAL: could not access Kubernetes starlane instance named '{}'", kubernetes_instance_name);
                error!("{}",message);
                return Err(message.into());
            }
        };
        let starlane_meta: ObjectMeta = starlane.metadata.clone();

        let rtn = KubeCore {
            skel: skel,
            client: client,
            namespace: namespace,
            starlane_meta: starlane_meta,
            api_version: starlane.api_version.clone()
        };

        Ok(rtn)
    }
}


#[async_trait]
impl Host for KubeCore {
    async fn assign(
        &self,
        assign: ResourceAssign<AssignResourceStateSrc<DataSet<BinSrc>>>,
    ) -> Result<(), Fail> {


        let provisioners: Api<StarlaneProvisioner> = Api::default_namespaced(self.client.clone() );
        let parts:ResourceKindParts = assign.archetype().kind.into();
        let mut list_params = ListParams::default();
        list_params = list_params.labels(format!("type={}",parts.resource_type).as_str() );
        if let Option::Some(kind) = parts.kind {
            list_params = list_params.labels(format!("kind={}", kind).as_str());
        }
        if let Option::Some(specific) = parts.specific {
            list_params = list_params.labels(format!("vendor={}", specific.vendor.to_string()).as_str());
            list_params = list_params.labels(format!("product={}", specific.product).as_str());
            list_params = list_params.labels(format!("variant={}", specific.variant).as_str());
            list_params = list_params.labels(format!("version={}", specific.version.to_string()).as_str());
        }

        let mut provisioners = provisioners.list(&list_params ).await?;

        //let provisioner:StarlaneProvisioner  = provisioners.items.get_mut(0).ok_or_else(||)?;

        if provisioners.items.is_empty() {
           return Err(Fail::NoProvisioner(assign.stub.archetype.kind.clone()));
        }

        let provisioner:StarlaneProvisioner  = provisioners.items.remove(0);
        let provisioner_name = provisioner.metadata.name.ok_or("expected provisioner to have a name")?;

        let starlane_resource_api: Api<StarlaneResource> = Api::default_namespaced(self.client.clone());
        let mut starlane_resource = StarlaneResource::new(assign.stub.key.clone().to_skewer_case().as_str(), StarlaneResourceSpec::default());
        let starlane_resource_spec: &mut StarlaneResourceSpec = & mut starlane_resource.spec;
        starlane_resource_spec.address = assign.stub.address.to_string();
        starlane_resource_spec.createArgs = Option::None;
        starlane_resource_spec.provisioner = provisioner_name;
        starlane_resource_spec.snakeKey = assign.stub.key.clone().to_snake_case();

        let starlane_resource_meta: &mut ObjectMeta= & mut starlane_resource.metadata;
        let mut owner_ref = OwnerReference::default();
        owner_ref.kind = "Starlane".to_string();
        owner_ref.name = self.starlane_meta.name.as_ref().ok_or("expected Starlane instance to have a Name")?.clone();
        owner_ref.uid = self.starlane_meta.uid.as_ref().ok_or("expected Starlane instance to have a uid")?.clone();
        owner_ref.api_version = self.api_version.clone();
        starlane_resource_meta.owner_references.push(owner_ref);

        starlane_resource_api.create( &PostParams::default(), &starlane_resource ).await?;

        println!("STARLANE RESOURCE CREATED!");

        Ok(())
    }

    async fn has(&self, key: ResourceKey) -> bool {
        todo!()
    }

<<<<<<< HEAD:rust/starlane-core/src/star/core/resource/host/kube.rs
    async fn get(&self, _identifier: ResourceKey ) -> Result<Option<DataSet<BinSrc>>, Fail> {
=======
    async fn get(&self, _identifier: ResourceKey ) -> Result<DataSet<BinSrc>, Fail> {
>>>>>>> f2361a20ec5930eab8327e64fbc6e3b3d95d08d0:rust/starlane-core/src/core/kube.rs
        unimplemented!()
//        self.store.get(identifier).await
    }

<<<<<<< HEAD:rust/starlane-core/src/star/core/resource/host/kube.rs

=======
>>>>>>> f2361a20ec5930eab8327e64fbc6e3b3d95d08d0:rust/starlane-core/src/core/kube.rs

    async fn delete(&self, _identifier: ResourceKey ) -> Result<(), Fail> {
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

#[derive(CustomResource, Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
#[kube(group = "starlane.starlane.io", version = "v1alpha1", kind = "StarlaneProvisioner", namespaced)]
struct StarlaneProvisionerSpec{

}






















 */
