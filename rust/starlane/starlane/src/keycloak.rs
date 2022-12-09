use crate::err::StarlaneErr;
use cosmic_hyperspace::driver::{
    Driver, DriverCtx, DriverHandler, DriverSkel, HyperDriverFactory, HyperSkel, Item, ItemHandler,
    ItemSkel, ItemSphere,
};
use cosmic_hyperspace::err::HyperErr;
use cosmic_hyperspace::star::HyperStarSkel;
use cosmic_hyperspace::Platform;
use cosmic_space::artifact::ArtRef;
use cosmic_space::config::bind::BindConfig;
use cosmic_space::hyper::HyperSubstance;
use cosmic_space::kind::{BaseKind, Kind, Specific, UserVariant};
use cosmic_space::parse::bind_config;
use cosmic_space::point::Point;
use cosmic_space::selector::{KindSelector, Selector};
use cosmic_space::substance::Substance;
use cosmic_space::util::{log, log_str};
use cosmic_space::wave::exchange::asynch::InCtx;
use keycloak::types::{
    CredentialRepresentation, ProtocolMapperRepresentation, RealmRepresentation, UserRepresentation,
};
use keycloak::{KeycloakAdmin, KeycloakAdminToken, KeycloakError};
use mechtron_host::err::HostErr;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::str::FromStr;
use std::sync::Arc;
use validator::validate_email;
use cosmic_space::HYPER_USERBASE;

lazy_static! {
    static ref KEYCLOAK_BIND_CONFIG: ArtRef<BindConfig> = ArtRef::new(
        Arc::new(auth_bind()),
        Point::from_str("GLOBAL::repo:1.0.0:/bind/keycloak.bind").unwrap()
    );
}

fn auth_bind() -> BindConfig {
    log(bind_config(
        r#"
    Bind(version=1.0.0)
    {
        Route -> {
        }
    }
    "#,
    ))
    .unwrap()
}

pub struct KeycloakDriverFactory;

impl KeycloakDriverFactory {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl<P> HyperDriverFactory<P> for KeycloakDriverFactory
where
    P: Platform,
    P::Err: StarlaneErr,
{
    fn kind(&self) -> KindSelector {
        KindSelector::from_str("<UserBase>").unwrap()
    }

    async fn create(
        &self,
        skel: HyperStarSkel<P>,
        driver_skel: DriverSkel<P>,
        ctx: DriverCtx,
    ) -> Result<Box<dyn Driver<P>>, P::Err> {
        let skel = HyperSkel::new(skel, driver_skel);
        Ok(Box::new(log(KeycloakDriver::new(skel, ctx)
            .await
            .map_err(|e| e.to_space_err()))?))
    }
}

pub struct KeycloakDriver<P>
where
    P: Platform,
{
    skel: HyperSkel<P>,
    ctx: DriverCtx,
    admin: StarlaneKeycloakAdmin<P>,
}

#[handler]
impl<P> KeycloakDriver<P>
where
    P: Platform,
    P::Err: StarlaneErr,
{
    pub async fn new(skel: HyperSkel<P>, ctx: DriverCtx) -> Result<Self, P::Err> {
        let admin = StarlaneKeycloakAdmin::new().await?;
        Ok(Self { skel, ctx, admin })
    }
}

#[async_trait]
impl<P> Driver<P> for KeycloakDriver<P>
where
    P: Platform,
    P::Err: StarlaneErr,
{
    fn kind(&self) -> Kind {
        Kind::UserBase
    }

    async fn item(&self, point: &Point) -> Result<ItemSphere<P>, P::Err> {
        let record = self.skel.driver.locate(point).await?;
        let skel = ItemSkel::new(
            point.clone(),
            record.details.stub.kind,
            self.skel.driver.clone(),
        );
        Ok(ItemSphere::Handler(Box::new(Keycloak::restore(
            skel,
            (),
            (),
        ))))
    }

    async fn handler(&self) -> Box<dyn DriverHandler<P>> {
        Box::new(KeycloakDriverHandler::restore(
            self.skel.clone(),
            self.ctx.clone(),
            self.admin.clone(),
        ))
    }
}

pub struct KeycloakDriverHandler<P>
where
    P: Platform,
{
    skel: HyperSkel<P>,
    ctx: DriverCtx,
    admin: StarlaneKeycloakAdmin<P>,
}

impl<P> KeycloakDriverHandler<P>
where
    P: Platform,
{
    fn restore(skel: HyperSkel<P>, ctx: DriverCtx, admin: StarlaneKeycloakAdmin<P>) -> Self {
        Self { skel, ctx, admin }
    }
}

impl<P> DriverHandler<P> for KeycloakDriverHandler<P>
where
    P: Platform,

    <P as Platform>::Err: StarlaneErr,
{
}

#[handler]
impl<P> KeycloakDriverHandler<P>
where
    P: Platform,
    <P as Platform>::Err: StarlaneErr,
{
    #[route("Hyp<Assign>")]
    async fn assign(&self, ctx: InCtx<'_, HyperSubstance>) -> Result<(), P::Err> {

println!("\tASSIGN OAuth")        ;
        if let HyperSubstance::Assign(assign) = ctx.input {
            self.admin
                .init_realm_for_point(
                    normalize_realm(&assign.details.stub.point),
                    &assign.details.stub.point,
                )
                .await?;
        }
        Ok(())
    }
}

pub struct Keycloak<P>
where
    P: Platform,
{
    skel: ItemSkel<P>,
}

#[handler]
impl<P> Keycloak<P> where P: Platform {}

impl<P> Item<P> for Keycloak<P>
where
    P: Platform,
{
    type Skel = ItemSkel<P>;
    type Ctx = ();
    type State = ();

    fn restore(skel: Self::Skel, ctx: Self::Ctx, state: Self::State) -> Self {
        Self { skel }
    }
}

#[async_trait]
impl<P> ItemHandler<P> for Keycloak<P>
where
    P: Platform,
{
    async fn bind(&self) -> Result<ArtRef<BindConfig>, P::Err> {
        Ok(KEYCLOAK_BIND_CONFIG.clone())
    }
}

lazy_static! {
    static ref USER_BIND_CONFIG: ArtRef<BindConfig> = ArtRef::new(
        Arc::new(user_bind()),
        Point::from_str("GLOBAL::repo:1.0.0:/bind/user.bind").unwrap()
    );
}

fn user_bind() -> BindConfig {
    log(bind_config(
        r#"
    Bind(version=1.0.0)
    {
        Route -> {
        }
    }
    "#,
    ))
    .unwrap()
}

pub struct UserDriverFactory;

impl UserDriverFactory {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl<P> HyperDriverFactory<P> for UserDriverFactory
where
    P: Platform,
{
    fn kind(&self) -> KindSelector {
        KindSelector::from_str("<User>").unwrap()
    }

    async fn create(
        &self,
        skel: HyperStarSkel<P>,
        driver_skel: DriverSkel<P>,
        ctx: DriverCtx,
    ) -> Result<Box<dyn Driver<P>>, P::Err> {
        let skel = HyperSkel::new(skel, driver_skel);
        Ok(Box::new(UserDriver::new(skel, ctx)))
    }
}

pub struct UserDriver<P>
where
    P: Platform,
{
    skel: HyperSkel<P>,
    ctx: DriverCtx,
}

#[handler]
impl<P> UserDriver<P>
where
    P: Platform,
{
    pub fn new(skel: HyperSkel<P>, ctx: DriverCtx) -> Self {
        Self { skel, ctx }
    }
}

#[async_trait]
impl<P> Driver<P> for UserDriver<P>
where
    P: Platform,
{
    fn kind(&self) -> Kind {
        Kind::User
    }

    async fn item(&self, point: &Point) -> Result<ItemSphere<P>, P::Err> {
        let record = self.skel.driver.locate(point).await?;
        let skel = ItemSkel::new(
            point.clone(),
            record.details.stub.kind,
            self.skel.driver.clone(),
        );
        Ok(ItemSphere::Handler(Box::new(User::restore(skel, (), ()))))
    }

    async fn handler(&self) -> Box<dyn DriverHandler<P>> {
        Box::new(UserDriverHandler::restore(
            self.skel.clone(),
            self.ctx.clone(),
        ))
    }
}

pub struct UserDriverHandler<P>
where
    P: Platform,
{
    skel: HyperSkel<P>,
    ctx: DriverCtx,
}

impl<P> UserDriverHandler<P>
where
    P: Platform,
{
    fn restore(skel: HyperSkel<P>, ctx: DriverCtx) -> Self {
        Self { skel, ctx }
    }
}

impl<P> DriverHandler<P> for UserDriverHandler<P> where P: Platform {}

#[handler]
impl<P> UserDriverHandler<P>
where
    P: Platform,
{
    #[route("Hyp<Assign>")]
    async fn assign(&self, ctx: InCtx<'_, HyperSubstance>) -> Result<(), P::Err> {
println!("\tASSIGN USER")        ;
        Ok(())
    }
}

pub struct User<P>
where
    P: Platform,
{
    skel: ItemSkel<P>,
}

#[handler]
impl<P> User<P> where P: Platform {}

impl<P> Item<P> for User<P>
where
    P: Platform,
{
    type Skel = ItemSkel<P>;
    type Ctx = ();
    type State = ();

    fn restore(skel: Self::Skel, ctx: Self::Ctx, state: Self::State) -> Self {
        Self { skel }
    }
}

#[async_trait]
impl<P> ItemHandler<P> for User<P>
where
    P: Platform,
{
    async fn bind(&self) -> Result<ArtRef<BindConfig>, P::Err> {
        Ok(USER_BIND_CONFIG.clone())
    }
}

#[derive(Clone)]
pub struct StarlaneKeycloakAdmin<P>
where
    P: Platform,
{
    pub admin: Arc<KeycloakAdmin>,
    phantom: PhantomData<P>,
}

impl<P> StarlaneKeycloakAdmin<P>
where
    P: Platform,
    P::Err: StarlaneErr,
{
    pub async fn new() -> Result<Self, P::Err> {
        let url = std::env::var("STARLANE_KEYCLOAK_URL")
            .map_err(|e| "UserBase: environment variable 'STARLANE_KEYCLOAK_URL' not set.")?;
        let password = std::env::var("STARLANE_PASSWORD")
            .map_err(|e| "UserBase: environment variable 'STARLANE_PASSWORD' not set.")?;

        let user = "hyperuser".to_string();
        let client = reqwest::Client::new();
println!("keycloak admin url: {} user: {} password: {}", url.to_string(), user.to_string(), password.to_string());
        let admin_token = KeycloakAdminToken::acquire(&url, &user, &password, &client).await?;

        let admin = Arc::new(KeycloakAdmin::new(&url, admin_token, client));
        Ok(Self {
            admin,
            phantom: Default::default(),
        })
    }

    pub async fn get_realm_from_point(&self, realm: &Point) -> Result<RealmRepresentation, P::Err> {
        let realm = normalize_realm(realm);
        Ok(self.admin.realm_get(realm.as_str()).await?)
    }

    pub async fn delete_realm_from_point(&self, realm: &Point) -> Result<(), P::Err> {
        let realm = normalize_realm(realm);
        self.admin.realm_delete(realm.as_str()).await?;
        Ok(())
    }

    pub async fn create_realm_from_point(
        &self,
        realm_point: &Point,
        registration_email_as_username: Option<bool>,
        verify_email: Option<bool>,
        sso_session_max_lifespan: Option<String>,
    ) -> Result<(), P::Err> {
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
        self.init_realm_for_point(realm, &realm_point).await?;
        Ok(())
    }
    pub async fn update_realm_for_point(
        &self,
        realm: String,
        realm_point: &Point,
        registration_email_as_username: Option<bool>,
        verify_email: Option<bool>,
        sso_session_max_lifespan: Option<String>,
    ) -> Result<(), P::Err> {
        let sso_session_max_lifespan = match sso_session_max_lifespan {
            None => None,
            Some(sso_session_max_lifespan) => {
                Some(i32::from_str(sso_session_max_lifespan.as_str())?)
            }
        };

        self.admin
            .realm_put(
                &realm,
                RealmRepresentation {
                    realm: Some(realm.clone().into()),
                    enabled: Some(true),
                    duplicate_emails_allowed: Some(false),
                    registration_email_as_username,
                    verify_email,
                    sso_session_max_lifespan,
                    ..Default::default()
                },
            )
            .await?;
        Ok(())
    }

    pub async fn init_realm_for_point(
        &self,
        realm: String,
        realm_point: &Point,
    ) -> Result<(), P::Err> {
println!("Init Realm for Point: {}", realm);
        let client_id = "${client_admin-cli}";


        let clients = log_err(self
            .admin
            .realm_clients_get(realm.clone().as_str(), None, None, None, None, None, None)
            .await)?;
println!("Got Realm Clients");
        let client_admin_cli_id = clients
            .into_iter()
            .find_map(|client| {
                if let Some(name) = client.name {
                    if client_id == name {
                        client.id.clone()
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .ok_or(format!(
                "User<Keycloak> could not find client_id '{}'",
                client_id
            ))?;

        {
            let mut config = HashMap::new();
            config.insert(
                "userinfo.token.claim".to_string(),
                Value::String("true".to_string()),
            );
            config.insert(
                "user.attribute".to_string(),
                Value::String("username".to_string()),
            );
            config.insert(
                "id.token.claim".to_string(),
                Value::String("true".to_string()),
            );
            config.insert(
                "access.token.claim".to_string(),
                Value::String("true".to_string()),
            );
            config.insert(
                "claim.name".to_string(),
                Value::String("preferred_username".to_string()),
            );
            config.insert(
                "jsonType.label".to_string(),
                Value::String("String".to_string()),
            );
            let username = ProtocolMapperRepresentation {
                config: Some(config),
                name: Some("username".to_string()),
                protocol: Some("openid-connect".to_string()),
                protocol_mapper: Some("oidc-usermodel-property-mapper".to_string()),
                ..Default::default()
            };
println!("GOT HERE");
            log_err(self.admin
                .realm_clients_with_id_protocol_mappers_models_post(
                    realm.as_str(),
                    client_admin_cli_id.as_str(),
                    username,
                )
                .await);
        }
println!("AND HERE");
        {
            let mut config = HashMap::new();
            config.insert(
                "userinfo.token.claim".to_string(),
                Value::String("true".to_string()),
            );
            config.insert(
                "id.token.claim".to_string(),
                Value::String("true".to_string()),
            );
            config.insert(
                "access.token.claim".to_string(),
                Value::String("true".to_string()),
            );
            config.insert(
                "claim.name".to_string(),
                Value::String("userbase_ref".to_string()),
            );
            config.insert(
                "claim.value".to_string(),
                Value::String(realm_point.to_string()),
            );
            config.insert(
                "jsonType.label".to_string(),
                Value::String("String".to_string()),
            );
            let userbase_ref = ProtocolMapperRepresentation {
                config: Some(config),
                name: Some("userbase_ref".to_string()),
                protocol: Some("openid-connect".to_string()),
                protocol_mapper: Some("oidc-hardcoded-claim-mapper".to_string()),
                ..Default::default()
            };

            self.admin
                .realm_clients_with_id_protocol_mappers_models_post(
                    realm.as_str(),
                    client_admin_cli_id.as_str(),
                    userbase_ref,
                )
                .await;
        }

        {
            let mut config = HashMap::new();
            config.insert("multivalued".to_string(), Value::String("true".to_string()));
            config.insert(
                "user.attribute".to_string(),
                Value::String("foo".to_string()),
            );
            config.insert(
                "id.token.claim".to_string(),
                Value::String("true".to_string()),
            );
            config.insert(
                "access.token.claim".to_string(),
                Value::String("true".to_string()),
            );
            config.insert(
                "claim.name".to_string(),
                Value::String("groups".to_string()),
            );
            config.insert(
                "jsonType.label".to_string(),
                Value::String("String".to_string()),
            );
            let groups = ProtocolMapperRepresentation {
                config: Some(config),
                name: Some("groups".to_string()),
                protocol: Some("openid-connect".to_string()),
                protocol_mapper: Some("oidc-usermodel-property-mapper".to_string()),
                ..Default::default()
            };

            self.admin
                .realm_clients_with_id_protocol_mappers_models_post(
                    realm.as_str(),
                    client_admin_cli_id.as_str(),
                    groups,
                )
                .await;
        }

        {
            let mut config = HashMap::new();
            config.insert("multivalued".to_string(), Value::String("true".to_string()));
            config.insert(
                "user.attribute".to_string(),
                Value::String("foo".to_string()),
            );
            config.insert(
                "access.token.claim".to_string(),
                Value::String("true".to_string()),
            );
            config.insert(
                "claim.name".to_string(),
                Value::String("realm_access.roles".to_string()),
            );
            config.insert(
                "jsonType.label".to_string(),
                Value::String("String".to_string()),
            );
            let roles = ProtocolMapperRepresentation {
                config: Some(config),
                name: Some("realm roles".to_string()),
                protocol: Some("openid-connect".to_string()),
                protocol_mapper: Some("oidc-usermodel-property-mapper".to_string()),
                ..Default::default()
            };

            self.admin
                .realm_clients_with_id_protocol_mappers_models_post(
                    realm.as_str(),
                    client_admin_cli_id.as_str(),
                    roles,
                )
                .await;
        }

println!("KEYCLOAK INIT COMPLETE");
        Ok(())
    }

    pub async fn select_all(
        &self,
        realm: &Point,
        first: i32,
        max: i32,
    ) -> Result<Vec<UserRepresentation>, P::Err> {
        let realm = normalize_realm(realm);
        Ok(self
            .admin
            .realm_users_get(
                realm.as_str(),
                Some(true),
                None,
                None,
                None,
                None,
                Some(first),
                None,
                None,
                None,
                None,
                Some(max),
                None,
                None,
                None,
            )
            .await?)
    }

    pub async fn select_by_username(
        &self,
        realm: &Point,
        username: String,
    ) -> Result<Vec<UserRepresentation>, P::Err> {
        let realm = normalize_realm(realm);
        Ok(self
            .admin
            .realm_users_get(
                realm.as_str(),
                Some(true),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(username),
                None,
            )
            .await?)
    }

    pub async fn select_by_email(
        &self,
        realm: &Point,
        email: String,
    ) -> Result<Vec<UserRepresentation>, P::Err> {
        let realm = normalize_realm(realm);
        Ok(self
            .admin
            .realm_users_get(
                realm.as_str(),
                Some(true),
                Some(email),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await?)
    }

    pub async fn reset_password(
        &self,
        realm: &Point,
        email: String,
        password: String,
    ) -> Result<(), P::Err> {
        if !validate_email(&email) {
            return Err(format!("invalid email '{}'", email).into());
        }
        let mut users = self.select_by_email(realm, email.clone()).await?;
        if users.is_empty() {
            return Err(format!("could not find email '{}'", email).into());
        } else if users.len() > 1 {
            return Err(format!("duplicate accounts for email '{}'", email).into());
        }

        let mut user = users.remove(0);
        let id = user.id.ok_or("user id must be set")?;
        let cred = CredentialRepresentation {
            value: Some(password.clone()),
            temporary: Some(false),
            type_: Some("password".to_string()),
            ..Default::default()
        };

        let realm = normalize_realm(realm);
        self.admin
            .realm_users_with_id_reset_password_put(realm.as_str(), id.as_str(), cred)
            .await?;
        Ok(())
    }

    pub async fn add_user_attributes(
        &self,
        realm: &Point,
        username: String,
        attributes: HashMap<String, Value>,
    ) -> Result<(), P::Err> {
        let users = self.select_by_username(realm, username).await?;

        for mut user in users {
            let realm = normalize_realm(realm);
            let mut attributes = attributes.clone();
            match user.attributes {
                None => {}
                Some(mut old_attributes) => {
                    for (key, value) in old_attributes {
                        if !attributes.contains_key(&key) {
                            attributes.insert(key, value.into());
                        }
                    }
                }
            }

            user.attributes = Some(attributes.into());

            self.admin
                .realm_users_with_id_put(
                    realm.as_str(),
                    user.id.as_ref().ok_or("expected user id")?.clone().as_str(),
                    user,
                )
                .await?;
        }
        Ok(())
    }

    pub async fn create_user(
        &self,
        realm: &Point,
        email: String,
        username: Option<String>,
        password: Option<String>,
        point: &Point,
    ) -> Result<(), P::Err> {
        let realm = normalize_realm(realm);

        let mut attributes = HashMap::new();
        attributes.insert("point".to_string(), Value::String(point.to_string()));

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
                        ..Default::default()
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
    Point::hyperuser() == *point
}

pub fn is_hyper_userbase(point: &Point) -> bool {
println!("Point::hyper_userbase() == *point : {} == {} {}",Point::hyper_userbase().to_string(), point.to_string(),   (Point::hyper_userbase() == *point).to_string());
    Point::hyper_userbase() == *point
}

fn normalize_realm(realm: &Point) -> String {
    if is_hyper_userbase(realm) {
        "master".to_string()
    } else {
        realm.to_string().replace(":", "_")
    }
}

pub fn log_err<R>(result: Result<R,KeycloakError>) -> Result<R,KeycloakError> {
    if let Err(err) = &result {
       match err {
           KeycloakError::ReqwestFailure(r) => {
               println!("\tREQUEST FAILURE: {}", r.to_string());
           }
           KeycloakError::HttpFailure { status, body, text } => {

               println!("\tHttpFailure {}",text);
           }
       }
    }

    result
}

#[cfg(test)]
pub mod zoinks{
    use cosmic_space::point::Point;
    use crate::keycloak::StarlaneKeycloakAdmin;
    use crate::Starlane;

    #[test]
    pub fn test() {
        println!("hello test");
        assert!(true)
    }

    #[test]
    pub fn test_admin() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build().unwrap();
        runtime.block_on(async move {
            let admin: StarlaneKeycloakAdmin<Starlane> = StarlaneKeycloakAdmin::new().await.unwrap();
            admin.init_realm_for_point("master".to_string(), &Point::hyper_userbase()).await.unwrap();

            println!("done");
        });

    }

}
