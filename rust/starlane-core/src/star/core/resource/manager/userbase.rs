use std::collections::HashMap;
use std::env;
use std::future::Future;
use keycloak::{KeycloakAdmin, KeycloakAdminToken, KeycloakError};
use keycloak::types::{CredentialRepresentation, ProtocolMapperRepresentation, RealmRepresentation, UserRepresentation};
use mesh_portal::version::latest::command::common::StateSrc;
use mesh_portal::version::latest::id::Address;
use serde_json::{json, Value};
use crate::error::Error;
use crate::resource::{ResourceAssign, ResourceType};
use crate::star::core::resource::manager::ResourceManager;
use crate::star::StarSkel;

pub struct UserBaseKeycloakCoreManager {
    skel: StarSkel,
    admin: StarlaneKeycloakAdmin
}

impl UserBaseKeycloakCoreManager {
    pub async fn new(skel: StarSkel) -> Result<Self,Error> {
println!("New UserBaseKeycloakCoreManager!");
        let admin = match StarlaneKeycloakAdmin::new().await {
            Ok(admin) => {admin}
            Err(err) => {
                error!("{}",err.to_string());
                return Err(format!("UserBaseKeycloakCoreManager: could not establish an admin connection to Keycloak server: {}",err.to_string()).into() );
            }
        };
println!("Got connection to KeycloakAdmin!");
        Ok(UserBaseKeycloakCoreManager {
            skel: skel.clone(),
            admin
        })
    }
}

#[async_trait]
impl ResourceManager for UserBaseKeycloakCoreManager {

    fn resource_type(&self) -> ResourceType {
        ResourceType::UserBase
    }


    async fn assign(
        &mut self,
        assign: ResourceAssign,
    ) -> Result<(), Error> {
        match assign.state {
            StateSrc::Stateless => {
            }
            StateSrc::StatefulDirect(_) => {
                return Err("UserBase<Keycloak> must be stateless".into());
            }
        };

        println!("UerBase<Keycloak> Assign");
        match self.admin.create_realm_from_address(&assign.stub.address ).await
        {
           Err(err) => {
                error!("{}",err.to_string());
                return Err(format!("UserBase<Keyloak>: could not create realm for {}", assign.stub.address.to_string() ).into())
            }
            _ => {}
        }

        Ok(())
    }


}

pub struct StarlaneKeycloakAdmin {
    admin: KeycloakAdmin
}

impl StarlaneKeycloakAdmin {
    pub async fn new(  ) -> Result<Self,Error> {
        let url = std::env::var("STARLANE_KEYCLOAK_URL").map_err(|e|{"UserBase<Keycloak>: environment variable 'STARLANE_KEYCLOAK_URL' not set."})?;
        let password = std::env::var("STARLANE_PASSWORD").map_err(|e|{"UserBase<Keycloak>: environment variable 'STARLANE_PASSWORD' not set."})?;

        let user = "hyperuser".to_string();
        let client = reqwest::Client::new();
        let admin_token = KeycloakAdminToken::acquire(&url, &user, &password, &client).await?;

        eprintln!("{}", json!(admin_token));

        let admin = KeycloakAdmin::new(&url, admin_token, client);
        Ok(Self {
            admin
        })
    }

    fn normalize_realm(realm: &Address) -> String {
        realm.to_string().replace(":","_")
    }

    pub async fn delete_realm_from_address(&self, realm: &Address ) -> Result<(),Error> {
        let realm = Self::normalize_realm(realm);
        self.admin.realm_delete(realm.as_str() ).await?;
        Ok(())
    }

    pub async fn create_realm_from_address(&self, realm: &Address) -> Result<(),Error> {
        let realm = Self::normalize_realm(realm);
        self.admin
            .post(RealmRepresentation {
                realm: Some(realm.clone().into()),
                enabled: Some(true),
                ..Default::default()
            })
            .await?;

        let client_id = "${client_admin-cli}";
        let clients = self.admin.realm_clients_get(realm.clone().as_str(), None,None,None,None,None).await?;
        let client_admin_cli_id = clients.into_iter().find_map( |client| {
            if let Some(name) = client.name {
                if client_id == name {
                    client.id.clone()
                } else {
                    None
                }
            } else {
                None
            }
        } ).ok_or(format!("UserBase<Keycloak> could not find client_id '{}'", client_id) )?;


        {
            let mut config = HashMap::new();
            config.insert("userinfo.token.claim".to_string(), Value::String("true".to_string()));
            config.insert("user.attribute".to_string(), Value::String("username".to_string()));
            config.insert("id.token.claim".to_string(), Value::String("true".to_string()));
            config.insert("access.token.claim".to_string(), Value::String("true".to_string()));
            config.insert("claim.name".to_string(), Value::String("preferred_username".to_string()));
            config.insert("jsonType.label".to_string(), Value::String("String".to_string()));
            let username = ProtocolMapperRepresentation {
                config: Some(config),
                name: Some("username".to_string()),
                protocol: Some("openid-connect".to_string()),
                protocol_mapper: Some("oidc-usermodel-property-mapper".to_string()),
                ..Default::default()
            };

            self.admin.realm_clients_with_id_protocol_mappers_models_post(realm.as_str(), client_admin_cli_id.as_str(), username).await?;
        }

        {
            let mut config = HashMap::new();
            config.insert("multivalued".to_string(), Value::String("true".to_string()));
            config.insert("user.attribute".to_string(), Value::String("foo".to_string()));
            config.insert("id.token.claim".to_string(), Value::String("true".to_string()));
            config.insert("access.token.claim".to_string(), Value::String("true".to_string()));
            config.insert("claim.name".to_string(), Value::String("groups".to_string()));
            config.insert("jsonType.label".to_string(), Value::String("String".to_string()));
            let groups = ProtocolMapperRepresentation {
                config: Some(config),
                name: Some("groups".to_string()),
                protocol: Some("openid-connect".to_string()),
                protocol_mapper: Some("oidc-usermodel-property-mapper".to_string()),
                ..Default::default()
            };

            self.admin.realm_clients_with_id_protocol_mappers_models_post(realm.as_str(), client_admin_cli_id.as_str(), groups).await?;
        }

        {
            let mut config = HashMap::new();
            config.insert("multivalued".to_string(), Value::String("true".to_string()));
            config.insert("user.attribute".to_string(), Value::String("foo".to_string()));
            config.insert("access.token.claim".to_string(), Value::String("true".to_string()));
            config.insert("claim.name".to_string(), Value::String("realm_access.roles".to_string()));
            config.insert("jsonType.label".to_string(), Value::String("String".to_string()));
            let roles = ProtocolMapperRepresentation {
                config: Some(config),
                name: Some("realm roles".to_string()),
                protocol: Some("openid-connect".to_string()),
                protocol_mapper: Some("oidc-usermodel-property-mapper".to_string()),
                ..Default::default()
            };

            self.admin.realm_clients_with_id_protocol_mappers_models_post(realm.as_str(), client_admin_cli_id.as_str(), roles).await?;
        }


        Ok(())
    }


    pub async fn create_user(&self, realm: &Address, username: &str, password: Option<&str> ) -> Result<(),Error> {
        let realm = Self::normalize_realm(realm);

        let user = UserRepresentation {
            username: Some(username.to_string()),
            enabled: Some(true),
            credentials: match password {
                None => None,
                Some(password) => {
                    let creds = CredentialRepresentation {
                        secret_data: Some(password.to_string()),
                        type_: Some("password".to_string()),
                        .. Default::default()
                    };
                    Some(vec![creds])
                }
            },
            ..Default::default()
        };
        self.admin.realm_users_post(realm.as_str(), user).await?;
        Ok(())
    }
}
