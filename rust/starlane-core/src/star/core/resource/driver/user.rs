use std::collections::HashMap;
use std::convert::TryInto;
use std::env;
use std::future::Future;
use std::str::FromStr;
use std::sync::Arc;
use alcoholic_jwt::{JWKS, token_kid};
use http::StatusCode;
use keycloak::{KeycloakAdmin, KeycloakAdminToken, KeycloakError};
use keycloak::types::{CredentialRepresentation, ProtocolMapperRepresentation, RealmRepresentation, UserRepresentation};
use mesh_portal::version::latest::command::common::StateSrc;
use mesh_portal::version::latest::entity::request::get::{Get, GetOp};
use mesh_portal::version::latest::entity::request::{Method, Rc};
use mesh_portal::version::latest::entity::request::create::{Create, PointSegFactory};
use mesh_portal::version::latest::entity::request::select::Select;
use mesh_portal::version::latest::entity::request::set::Set;
use mesh_portal::version::latest::entity::response::ResponseCore;
use mesh_portal::version::latest::http::HttpMethod;
use mesh_portal::version::latest::id::Point;
use mesh_portal::version::latest::messaging::{Request, Response};
use mesh_portal::version::latest::particle::Stub;
use mesh_portal::version::latest::payload::Payload;
use mesh_portal_versions::version::v0_0_1::parse::skewer_or_snake;
use nom::AsBytes;
use nom::combinator::all_consuming;
use nom_supreme::final_parser::final_parser;
use serde_json::{json, Value};
use validator::validate_email;
use crate::error::Error;
use crate::particle::{Kind, KindBase, ParticleAssign};
use crate::particle::property::{AddressPattern, AnythingPattern, BoolPattern, EmailPattern, PropertiesConfig, PropertyPattern, PropertyPermit, PropertySource};
use crate::registry::{match_kind};
use crate::star::core::resource::driver::ParticleCoreDriver;
use crate::star::StarSkel;




#[derive(Clone)]
pub struct UsernamePattern {

}

impl PropertyPattern for UsernamePattern {
    fn is_match(&self, value: &String) -> Result<(), Error> {
        match all_consuming(skewer_or_snake)(value.as_str()) {
            Ok(_) => {}
            Err(err) => {
                return Err(format!("illegal username '{}'. username must be all lowercase alphanumerical with '-' and '_' allowed.", value).into());
            }
        }
        Ok(())
    }
}


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
impl ParticleCoreDriver for UserBaseKeycloakCoreDriver {

    fn kind(&self) -> KindBase {
        KindBase::UserBase
    }



    async fn assign(
        &mut self,
        assign: ParticleAssign,
    ) -> Result<(), Error> {
        match assign.state {
            StateSrc::None=> {
            }
            StateSrc::Payload(_) => {
                return Err("UserBase<Keycloak> must be stateless".into());
            }
        };


        if is_hyper_userbase(&assign.details.stub.point)
        {
            let sso_session_max_lifespan = assign.details.properties.get("sso-session-max-lifespan" ).ok_or("cannot get value for required property 'sso-session-max-lifespan'")?.value.clone();
            self.admin.init_realm_for_point("master".to_string(), &assign.details.stub.point ).await?;
            match self.admin.update_realm_for_point("master".to_string(), &assign.details.stub.point, Some(false), Some(false), Some(sso_session_max_lifespan)).await
            {
                Err(err) => {
                    error!("{}",err.to_string());
                    return Err(format!("UserBase<Keyloak>: could not update master realm for {}", assign.details.stub.point.to_string()).into())
                }
                _ => {}
            }
        }
        else
        {
            let registration_email_as_username = assign.details.properties.get("registration-email-as-username" ).map_or(None, |x|{ Some(x.value=="true") });
            let verify_email= assign.details.properties.get("verify-email" ).map_or(None, |x|{ Some(x.value=="true") });
            let sso_session_max_lifespan = assign.details.properties.get("sso-session-max-lifespan" ).ok_or("cannot get value for required property 'sso-session-max-lifespan'")?.value.clone();

            match self.admin.create_realm_from_point(&assign.details.stub.point, registration_email_as_username, verify_email, Some(sso_session_max_lifespan)).await
            {
                Err(err) => {
                    error!("{}",err.to_string());
                    return Err(format!("UserBase<Keyloak>: could not create realm for {}", assign.details.stub.point.to_string()).into())
                }
                _ => {}
            }
        }

        Ok(())
    }

    async fn handle_request(&self, request: Request ) -> Response {
println!("USers handle reqeust...");
        match &request.core.method{
            Method::Cmd(rc) => {
               // request.clone().payload_result(self.particle_command(request.to.clone(), rc.clone() ).await)
                request.not_found()
            }
            Method::Http(_) => {
println!("handle HTTP: {}", request.core.uri.to_string());
                self.handle_http(request).await
            }
            Method::Msg(_) => {
                self.handle_msg(request).await
            }
        }
    }


    async fn particle_command(&self, to: Point, rc: Rc) -> Result<Payload,Error> {
        unimplemented!()
/*        match rc {
            Rc::Create(create) => { self.create_child(to, create).await }
            Rc::Set(set) => { self.set_child(to,set).await }
            Rc::Get(get) => { self.get_child(to,get).await }
            Rc::Select(select) => { self.select_child(to,select).await }
            _ => { unimplemented!() }
        }

 */
    }

}

impl UserBaseKeycloakCoreDriver{

    fn keycloak_url() -> Result<String,Error> {
        Ok(std::env::var("STARLANE_KEYCLOAK_URL").map_err(|e|{"User<Keycloak>: environment variable 'STARLANE_KEYCLOAK_URL' not set."})?)
    }

    async fn handle_msg( &self, request: Request ) -> Response {

        if let Method::Msg(method) =&request.core.method{
println!("UserBase<Keycloak> ... Handle Message action: {}", method.to_string());
            match method.as_str() {
                "GetJwks" => request.clone().payload_result(self.handle_get_jwks(&request).await),
                _ => {
                    request.status(404)
                }
            }
        } else {
            request.status(404)
        }
    }

    async fn handle_get_jwks( &self, request: &Request ) -> Result<Payload,Error>
    {
        let client = reqwest::Client::new();
        let realm = normalize_realm(&request.to);
        let url = Self::keycloak_url()?;
        let jwks= client
            .get(&format!(
                "{}/auth/realms/{}/protocol/openid-connect/certs",
                url, realm
            ))
            .send()
            .await?;
        let jwks = jwks.text().await?;

        Ok(Payload::Text(jwks))

    }


    async fn handle_http( &self, request: Request ) -> Response
    {
println!("UserBaseKeycloakCoreDriver: handle_http");
        if let Method::Http(method) =&request.core.method{
            match method {
                &HttpMethod::Post=> {
                    match &request.core.uri.path() {
                        &"/login" => request.clone().result(self.handle_login(&request).await),
                        &"/introspect" => request.clone().payload_result(self.handle_introspect_token(&request).await),
                        &"/refresh-token" => request.clone().payload_result(self.handle_refresh_token(&request).await),
                        _ => {
                            request.status(404)
                        }
                    }
                }
                _ => {request.status(404)}
            }
        } else {
            request.status(404)
        }
    }

    async fn handle_login( &self, request: &Request ) -> Result<ResponseCore,Error>{
println!("handle_login");
        let multipart: Vec<(String,String)> = serde_urlencoded::from_bytes(request.core.body.clone().to_bin()?.as_bytes() )?;
        let mut map = HashMap::new();
        for (key,value) in multipart {
            map.insert(key,value);
        }
        map.get("username").ok_or("username required")?;
        map.get("password").ok_or("password required")?;
        map.insert( "client_id".to_string(), "admin-cli".to_string() );
        map.insert( "grant_type".to_string(), "password".to_string() );

        let client = reqwest::Client::new();
        let realm = normalize_realm(&request.to);
        let url = Self::keycloak_url()?;
        let response = client
            .post(&format!(
                "{}/auth/realms/{}/protocol/openid-connect/token",
                url, realm
            ))
            .form(&map)
            .send()
            .await?;
        let response = ResponseCore {
            status: response.status(),
            headers: response.headers().clone(),
            body: Payload::Bin(Arc::new(response.bytes().await?.to_vec()))
        };
        Ok(response)
    }

    async fn handle_introspect_token( &self, request: &Request ) -> Result<Payload,Error>{
        let token: String = request.core.body.clone().try_into()?;
        let realm = normalize_realm(&request.to);
        let url = Self::keycloak_url()?;
        let url= format!("{}/auth/realms/{}/protocol/openid-connect/certs",url,realm);

        let client = reqwest::Client::new();
        let response = client.get(url ).send().await?;
        let jwks = response.text().await?;
        println!("jwks: {}", jwks);
        let jwks: JWKS = serde_json::from_str(jwks.as_str())?;

        let kid = token_kid(token.as_str())?.ok_or("expected token kid")?;

        let jwk = jwks.find(&kid).ok_or(format!("expected to find kid: {}", kid))?;

        let valid_jwt = alcoholic_jwt::validate( token.as_str(), jwk, vec![] )?;

        println!("valid_jwt: {}",valid_jwt.claims.to_string());


        Ok(Payload::Empty)
    }


    /*
    Method: POST
    URL: https://keycloak.example.com/auth/realms/myrealm/protocol/openid-connect/token
    Body type: x-www-form-urlencoded
    Form fields:
    client_id : <my-client-name>
    grant_type : refresh_token
    refresh_token: <my-refresh-token>
     */

    async fn handle_refresh_token( &self, request: &Request ) -> Result<Payload,Error>{
println!("handle_refresh_token...");
        let token: String = request.core.body.clone().try_into()?;
println!("received refresh token: {}", token );
        let client = reqwest::Client::new();
        let realm = normalize_realm(&request.to);
        let url = Self::keycloak_url()?;
        let response = client
            .post(&format!(
                "{}/auth/realms/{}/protocol/openid-connect/token",
                url, realm
            ))
            .form(&json!({
                "refresh_token": token,
                "client_id": "admin-cli",
                "grant_type": "refresh_token"
            }))
            .send()
            .await?;
        match &response.status().as_u16()
        {
            200 => {
                let response = response.text().await?;
                Ok(Payload::Bin(Arc::new(response.as_bytes().to_vec())))
            }
            other => {
                println!("{}",other);
                Err(Error::with_status(other.clone(), "could not refresh token" ))
            }
        }
    }

//    {{keycloak_url}}/admin/realms/{{realm}}/users/{{userId}}/logout



    async fn get_child(&self, to: Point, mut get: Get) -> Result<Payload,Error> {

        match &mut get.op {
            GetOp::State => {
                return Err("<User> is stateless".into());
            }
            GetOp::Properties(properties) => {
                if properties.contains(&"password".to_string() ) {
                    return Err("<User> cannot return 'password' property".into());
                }
            }
        }

        self.skel.registry_api.get(&get).await?;

        Ok(Payload::Empty)
    }


    async fn set_child(&self, to: Point, mut set: Set) -> Result<Payload,Error> {
        let record = self.skel.registry_api.locate(&set.point).await?;
        let password = match set.properties.map.remove("password") {
            None => {
                None
            }
            Some(password) => {
                password.opt()
            }
        };

        if set.properties.map.get("email" ).is_some() {
            return Err("UserBase<Keycloak>: 'email' property is immutable".into());
        }

        if set.properties.map.get("username" ).is_some() {
            return Err("UserBase<Keycloak>: 'username' property is immutable".into());
        }

        let email = record.details.properties.get("email").ok_or("User missing 'email' property")?.value.clone();

        if password.is_some() {
            self.admin.reset_password(&to, email, password.unwrap() ).await?;
        }

        Ok(Payload::Empty)
    }

    async fn create_child(&self, to: Point, create: Create ) -> Result<Payload,Error> {
        let mut create = create;

        let kind = match_kind(&create.template.kind)?;

        if kind != Kind::User {
            return Err("UserBase<KeyCloak> can only have <User> type for children".into());
        }

        let realm = &create.template.point.parent;
        let realm = self.admin.get_realm_from_point(realm).await?;

        let email = match create.properties.map.get("email") {
            None => None,
            Some(email) => {
                email.opt()
            }
        };
        let password = match create.properties.map.remove("password") {
            None => {
                None
            }
            Some(password) => {
                password.opt()
            }
        };

        let stub = match &create.template.point.child_segment_template {
            PointSegFactory::Exact(username) => {
                if realm.registration_email_as_username.unwrap_or(false) {
                    return Err(format!("UserBase<Keycloak>: realm '{}' requires registration email as username therefore exact point segment '{}' cannot be used. instead try pattern segment: 'user-%' ", create.template.point.parent.to_string(), username).into());
                } else {
                    let stub = self.skel.registry_api.create(&create).await?.stub;
                    if !is_hyperuser(&stub.point) {
                        self.admin.create_user(&create.template.point.parent, email.ok_or("'email' is required.")?, Some(username.to_string()), password, &stub.point).await?;
                    } else {
                        let mut attributes = HashMap::new();
                        attributes.insert( "point".to_string(), Value::String(stub.point.to_string()));
                        self.admin.add_user_attributes(&create.template.point.parent, username.to_string(),attributes ).await?;
                    }
                    stub
                }
            }
            PointSegFactory::Pattern(_) => {
                if realm.registration_email_as_username.unwrap_or(false) {
                    let stub = self.skel.registry_api.create(&create).await?.stub;
                    self.admin.create_user(&create.template.point.parent, email.ok_or("'email' is required.")?, None, password, &stub.point).await?;
                    stub
                } else {
                    let username = create.properties.get("username").ok_or(format!("UserBase<Keycloak>: realm '{}' requires property 'username' when creating a pattern point", create.template.point.parent.to_string()))?.set_or(format!("UserBase<Keycloak>: realm '{}' requires property 'username' when creating a pattern point", create.template.point.parent.to_string()))?.to_lowercase();
                    let stub = self.skel.registry_api.create(&create).await?.stub;
                    self.admin.create_user(&create.template.point.parent, email.ok_or("'email' is required")?, Some(username), password, &stub.point).await?;
                    stub
                }
            }
        };

       Ok(Payload::Stub(stub))
     }


    async fn select_child(&self, to: Point, select: Select) -> Result<Payload,Error> {
        self.skel.registry_api.cmd_select(&select).await
/*        let mut first = 0;
        let max = 100;
        let mut rtn = vec![];
        loop {
            let users = self.admin.select_all(&to, first, max).await?;
            if users.is_empty() {
                break;
            }
            for user in users {
                let point = user.attributes.ok_or("expected user to have attributes set")?.get("point").ok_or("expected 'point' attribute to be set")?.to_string();
                let point = Address::from_str(point.to_string().as_str() )?;
                if select.
                let record = self.skel.resource_locator_api.locate(point).await?;
                rtn.push(record.stub);
            }
        }

        Ok(Payload::Empty)

 */
    }

}





pub struct UserCoreDriver {
    skel: StarSkel,
    admin: StarlaneKeycloakAdmin
}

impl UserCoreDriver {
    pub async fn new(skel: StarSkel) -> Result<Self,Error> {
        let admin = match StarlaneKeycloakAdmin::new().await {
            Ok(admin) => {admin}
            Err(err) => {
                error!("{}",err.to_string());
                return Err(format!("UserKeycloakCoreManager: could not establish an admin connection to Keycloak server: {}",err.to_string()).into() );
            }
        };
        Ok(UserCoreDriver {
            skel: skel.clone(),
            admin
        })
    }
}

#[async_trait]
impl ParticleCoreDriver for UserCoreDriver {

    fn kind(&self) -> KindBase {
        KindBase::User
    }


    async fn assign(
        &mut self,
        assign: ParticleAssign,
    ) -> Result<(), Error> {
        match assign.state {
            StateSrc::None => {
            }
            StateSrc::Payload(_) => {
                return Err("User must be stateless".into());
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

        let admin = Arc::new(KeycloakAdmin::new(&url, admin_token, client));
        Ok(Self {
            admin
        })
    }



    pub async fn get_realm_from_point(&self, realm: &Point) -> Result<RealmRepresentation,Error> {
        let realm = normalize_realm(realm);
        Ok(self.admin.realm_get(realm.as_str() ).await?)
    }


    pub async fn delete_realm_from_point(&self, realm: &Point) -> Result<(),Error> {
        let realm = normalize_realm(realm);
        self.admin.realm_delete(realm.as_str() ).await?;
        Ok(())
    }

    pub async fn create_realm_from_point(&self, realm_point: &Point, registration_email_as_username: Option<bool>, verify_email: Option<bool>, sso_session_max_lifespan: Option<String> ) -> Result<(),Error> {
        let realm = normalize_realm(realm_point);
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
        self.init_realm_for_point(realm,&realm_point).await?;
        Ok(())
    }
    pub async fn update_realm_for_point(&self, realm: String, realm_point: &Point, registration_email_as_username: Option<bool>, verify_email: Option<bool>, sso_session_max_lifespan: Option<String> ) -> Result<(),Error> {

        let sso_session_max_lifespan = match sso_session_max_lifespan {
            None => None,
            Some(sso_session_max_lifespan) => Some(i32::from_str(sso_session_max_lifespan.as_str())?)
        };

        self.admin
            .realm_put(&realm, RealmRepresentation {
                realm: Some(realm.clone().into()),
                enabled: Some(true),
                duplicate_emails_allowed: Some(false),
                registration_email_as_username,
                verify_email,
                sso_session_max_lifespan,
                ..Default::default()
            })
            .await?;
        Ok(())
    }

    pub async fn init_realm_for_point(&self, realm: String, realm_point: &Point) -> Result<(),Error> {
        let client_id = "${client_admin-cli}"; let clients = self.admin.realm_clients_get(realm.clone().as_str(), None,None,None,None,None).await?; let client_admin_cli_id = clients.into_iter().find_map( |client| {
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
            config.insert("userinfo.token.claim".to_string(), Value::String("true".to_string()));
            config.insert("id.token.claim".to_string(), Value::String("true".to_string()));
            config.insert("access.token.claim".to_string(), Value::String("true".to_string()));
            config.insert("claim.name".to_string(), Value::String("userbase_ref".to_string()));
            config.insert("claim.value".to_string(), Value::String(realm_point.to_string()));
            config.insert("jsonType.label".to_string(), Value::String("String".to_string()));
            let userbase_ref= ProtocolMapperRepresentation {
                config: Some(config),
                name: Some("userbase_ref".to_string()),
                protocol: Some("openid-connect".to_string()),
                protocol_mapper: Some("oidc-hardcoded-claim-mapper".to_string()),
                ..Default::default()
            };

            self.admin.realm_clients_with_id_protocol_mappers_models_post(realm.as_str(), client_admin_cli_id.as_str(), userbase_ref).await?;
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



    pub async fn select_all(&self, realm: &Point, first: i32, max: i32) -> Result<Vec<UserRepresentation>,Error> {
        let realm = normalize_realm(realm);
        Ok(self.admin.realm_users_get(realm.as_str(), Some(true),None, None, None,None,Some(first),None,None,None, None, Some(max),None,None).await?)
    }


    pub async fn select_by_username(&self, realm: &Point, username: String) -> Result<Vec<UserRepresentation>,Error> {
        let realm = normalize_realm(realm);
        Ok(self.admin.realm_users_get(realm.as_str(), Some(true),None, None, None,None,None,None,None,None, None, None,None,Some(username)).await?)
    }


    pub async fn select_by_email(&self, realm: &Point, email: String) -> Result<Vec<UserRepresentation>,Error> {
        let realm = normalize_realm(realm);
        Ok(self.admin.realm_users_get(realm.as_str(), Some(true),Some(email), None, None,None,None,None,None,None, None, None,None,None).await?)
    }

    pub async fn reset_password(&self, realm: &Point, email: String, password: String ) -> Result<(),Error> {
        if !validate_email(&email) {
            return Err(format!("invalid email '{}'",email).into());
        }
        let mut users = self.select_by_email(realm, email.clone()).await?;
        if users.is_empty() {
            return Err(format!("could not find email '{}'",email).into());
        } else  if users.len() > 1 {
            return Err(format!("duplicate accounts for email '{}'",email).into());
        }

        let mut user = users.remove(0);
        let id = user.id.ok_or("user id must be set")?;
        let cred = CredentialRepresentation {
            value: Some(password.clone()),
            temporary: Some(false),
            type_: Some("password".to_string()),
            .. Default::default()
        };

        let realm = normalize_realm(realm);
        self.admin.realm_users_with_id_reset_password_put(realm.as_str(), id.as_str(), cred).await?;
        Ok(())
    }

    pub async fn add_user_attributes(&self, realm: &Point, username: String, attributes: HashMap<String,Value>) -> Result<(),Error> {

        let users = self.select_by_username(realm,username).await?;

        for mut user in users {
            let realm = normalize_realm(realm);
            let mut attributes = attributes.clone();
            match user.attributes {
                None => {}
                Some(mut old_attributes) => {
                    for (key,value) in old_attributes {
                        if !attributes.contains_key(&key) {
                            attributes.insert(key,value);
                        }
                    }
                }
            }

            user.attributes = Some(attributes);

            self.admin.realm_users_with_id_put(realm.as_str(), user.id.as_ref().ok_or("expected user id")?.clone().as_str(), user).await?;
        }
        Ok(())
    }

    pub async fn create_user(&self, realm: &Point, email: String, username: Option<String>, password: Option<String>, point: &Point) -> Result<(),Error> {
        let realm = normalize_realm(realm);

        let mut attributes = HashMap::new();
        attributes.insert( "point".to_string(), Value::String(point.to_string()) );

        let user = UserRepresentation {
            username: username,
            email: Some(email),
            enabled: Some(true),
            attributes: Some(attributes),
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

pub fn is_hyperuser(point: &Point) -> bool {
    point.to_string().as_str() == "hyperspace:users:hyperuser"
}

pub fn is_hyper_userbase(point: &Point) -> bool {
    point.to_string().as_str() == "hyperspace:users"
}

fn normalize_realm(realm: &Point) -> String {
    if is_hyper_userbase(realm) {
        "master".to_string()
    } else {
        realm.to_string().replace(":", "_")
    }
}
