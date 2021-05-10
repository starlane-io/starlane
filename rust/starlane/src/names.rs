
use crate::artifact::Artifact;
use crate::artifact::ArtifactId;
use crate::artifact::ArtifactKind;
use crate::artifact::Name;
use crate::app::AppKind;
use crate::actor::ActorKindExt;
use crate::actor::ActorKind;

lazy_static!
{
    pub static ref TEST_APP_KIND: AppKind = AppKind::from("starlane.io:starlane:core:test:/test/test_app").unwrap();
    pub static ref TEST_ACTOR_KIND: ActorKind = ActorKind::Actor(ActorKindExt::from("starlane.io:starlane:core:test:/test/test_actor").unwrap());

    pub static ref TEST_APP_CONFIG_ARTIFACT: Artifact = Artifact
    {
                    id: ArtifactId::from("starlane.io:starlane:core:test:1.0.0:/test/test_app.yaml").unwrap(),
                    kind: ArtifactKind::AppConfig(AppKind::from("starlane.io:starlane:core:test:/test/test_app").unwrap())
    };

    pub static ref TEST_ACTOR_CONFIG_ARTIFACT: Artifact = Artifact
    {
                    id: ArtifactId::from("starlane.io:starlane:core:test:1.0.0:/test/test_actor.yaml").unwrap(),
                    kind: ArtifactKind::ActorConfig(ActorKind::Actor(ActorKindExt::from("starlane.io:starlane:core:test:/test/test_actor").unwrap()))
    };
}