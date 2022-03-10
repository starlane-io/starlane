use std::collections::HashMap;
use std::env;
use std::future::Future;
use std::sync::Arc;
use keycloak::{KeycloakAdmin, KeycloakAdminToken, KeycloakError};
use keycloak::types::{CredentialRepresentation, ProtocolMapperRepresentation, RealmRepresentation, UserRepresentation};
use mesh_portal::version::latest::command::common::StateSrc;
use mesh_portal::version::latest::entity::request::create::Create;
use mesh_portal::version::latest::id::Address;
use mesh_portal::version::latest::resource::ResourceStub;
use mesh_portal_versions::version::v0_0_1::command::common::PropertyMod;
use mesh_portal_versions::version::v0_0_1::entity::request::create::AddressSegmentTemplate;
use serde_json::{json, Value};
use crate::error::Error;
use crate::resource::{ResourceAssign, ResourceType};
use crate::star::core::message::ResourceRegistrationChamber;
use crate::star::core::resource::driver::ResourceCoreDriver;
use crate::star::StarSkel;

pub struct UserBaseKeycloakCoreDriver {
    skel: StarSkel,
    admin: StarlaneKeycloakAdmin
}

impl UserBaseKeycloakCoreDriver {
    pub async fn new(skel: StarSkel) -> Result<Self,Error> {
        let admin = match StarlaneKeycloakAdmin::new().await {
            Ok(admin) => {admin}
            Err(err) => {
                error!("{}",err.to_string());
                return Err(format!("UserBaseKeycloakCoreManager: could not establish an admin connection to Keycloak server: {}",err.to_string()).into() );
            }
        };
        Ok(UserBaseKeycloakCoreDriver {
            skel: skel.clone(),
            admin
        })
    }
}

#[async_trait]
impl ResourceCoreDriver for UserBaseKeycloakCoreDriver {

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

        let registration_email_as_username = assign.stub.properties.get("registration_email_as_username" ).map_or( None, |x|{ Some(x.value=="true") });
        let verify_email= assign.stub.properties.get("registration_email_as_username" ).map_or( None, |x|{ Some(x.value=="true") });

        match self.admin.create_realm_from_address(&assign.stub.address, registration_email_as_username, verify_email ).await
        {
           Err(err) => {
                error!("{}",err.to_string());
                return Err(format!("UserBase<Keyloak>: could not create realm for {}", assign.stub.address.to_string() ).into())
            }
            _ => {}
        }

        Ok(())
    }

    async fn create_child(&self, create: Create ) -> Result<ResourceStub,Error> {
println!("create_child... ");
        let mut create = create;

        let realm = &create.template.address.parent;
        let realm = self.admin.get_realm_from_address(realm).await?;
        let chamber= ResourceRegistrationChamber::new(self.skel.clone());

        let email = create.properties.map.get("email" ).ok_or("UserBase<KeyCloak>: 'email' property is required.")?.set_or("UserBase<KeyCloak>: email property is required.")?;
        let password = match create.properties.map.get("password" ) {
            None => {
                None}
            Some(password) => {
                password.opt()
            }
        };

        match &create.template.address.child_segment_template {
            AddressSegmentTemplate::Exact(username) => {
                if realm.registration_email_as_username.unwrap_or(false) {
                    return Err(format!("UserBase<Keycloak>: realm '{}' requires registration email as username therefore exact address segment '{}' cannot be used. instead try pattern segment: 'user-%' ",create.template.address.parent.to_string(), username).into());
                } else {
                    self.admin.create_user(&create.template.address.parent, email, username.to_string(), password ).await?;
                }
            }
            AddressSegmentTemplate::Pattern(_) => {
                if realm.registration_email_as_username.unwrap_or(false) {
                    let username = create.properties.get("username" ).ok_or(format!("UserBase<Keycloak>: realm '{}' requires property 'username' when creating a pattern address",create.template.address.parent.to_string()))?.set_or(format!("UserBase<Keycloak>: realm '{}' requires property 'username' when creating a pattern address",create.template.address.parent.to_string()))?;
                    self.admin.create_user(&create.template.address.parent, email,username, password ).await?;
                } else {
                    self.admin.create_user(&create.template.address.parent, email.clone(),email, password ).await?;
                }
            }
        }

        chamber.create(&create).await
    }

}




pub struct UserCoreDriver {
    skel: StarSkel,
    admin: StarlaneKeycloakAdmin
}

impl UserCoreDriver {
    pub async fn new(skel: StarSkel) -> Result<Self,Error> {
        println!("New UserKeycloakCoreManager!");
        let admin = match StarlaneKeycloakAdmin::new().await {
            Ok(admin) => {admin}
            Err(err) => {
                error!("{}",err.to_string());
                return Err(format!("UserKeycloakCoreManager: could not establish an admin connection to Keycloak server: {}",err.to_string()).into() );
            }
        };
        println!("Got connection to KeycloakAdmin!");
        Ok(UserCoreDriver {
            skel: skel.clone(),
            admin
        })
    }
}

#[async_trait]
impl ResourceCoreDriver for UserCoreDriver {

    fn resource_type(&self) -> ResourceType {
        ResourceType::User
    }


    async fn assign(
        &mut self,
        assign: ResourceAssign,
    ) -> Result<(), Error> {
        match assign.state {
            StateSrc::Stateless => {
            }
            StateSrc::StatefulDirect(_) => {
                return Err("User<Keycloak> must be stateless".into());
            }
        };

        Ok(())
    }


}

#[derive(Clone)]
pub struct StarlaneKeycloakAdmin {
    admin: Arc<KeycloakAdmin>
}

impl StarlaneKeycloakAdmin {
    pub async fn new(  ) -> Result<Self,Error> {
        let url = std::env::var("STARLANE_KEYCLOAK_URL").map_err(|e|{"User<Keycloak>: environment variable 'STARLANE_KEYCLOAK_URL' not set."})?;
        let password = std::env::var("STARLANE_PASSWORD").map_err(|e|{"User<Keycloak>: environment variable 'STARLANE_PASSWORD' not set."})?;

        let user = "hyperuser".to_string();
        let client = reqwest::Client::new();
        let admin_token = KeycloakAdminToken::acquire(&url, &user, &password, &client).await?;

        eprintln!("{}", json!(admin_token));

        let admin = Arc::new(KeycloakAdmin::new(&url, admin_token, client));
        Ok(Self {
            admin
        })
    }

    fn normalize_realm(realm: &Address) -> String {
        realm.to_string().replace(":","_")
    }


    pub async fn get_realm_from_address(&self, realm: &Address ) -> Result<RealmRepresentation,Error> {
        let realm = Self::normalize_realm(realm);
        Ok(self.admin.realm_get(realm.as_str() ).await?)
    }


    pub async fn delete_realm_from_address(&self, realm: &Address ) -> Result<(),Error> {
        let realm = Self::normalize_realm(realm);
        self.admin.realm_delete(realm.as_str() ).await?;
        Ok(())
    }

    pub async fn create_realm_from_address(&self, realm: &Address, registration_email_as_username: Option<bool>, verify_email: Option<bool> ) -> Result<(),Error> {
        let realm = Self::normalize_realm(realm);
        self.admin
            .post(RealmRepresentation {
                realm: Some(realm.clone().into()),
                enabled: Some(true),
                duplicate_emails_allowed: Some(false),
                registration_email_as_username,
                verify_email,
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
        } ).ok_or(format!("User<Keycloak> could not find client_id '{}'", client_id) )?;


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


    pub async fn create_user(&self, realm: &Address, email: String, username: String, password: Option<String> ) -> Result<(),Error> {
        let realm = Self::normalize_realm(realm);


        let user = UserRepresentation {
            username: Some(username.clone()),
            email: Some(email),
            enabled: Some(true),
            credentials: match password {
                None => None,
                Some(password) => {
                    let creds = CredentialRepresentation {
                        value: Some(password),
                        temporary: Some(false),
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
