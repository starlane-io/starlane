
use crate::artifact::Artifact;
use crate::artifact::ArtifactId;
use crate::artifact::ArtifactKind;
use crate::artifact::Name;
use crate::app::AppSpecific;
use crate::actor::ActorSpecific;
use crate::actor::ActorKind;

lazy_static!
{
    pub static ref TEST_APP_SPEC: AppSpecific = AppSpecific::from("starlane.io:starlane:core:test:/test/test_app").unwrap();
    pub static ref TEST_ACTOR_SPEC: ActorSpecific = ActorSpecific::from("starlane.io:starlane:core:test:/test/test_actor").unwrap();

    pub static ref TEST_APP_CONFIG_ARTIFACT: Artifact = Artifact
    {
                    id: ArtifactId::from("starlane.io:starlane:core:test:1.0.0:/test/test_app.yaml").unwrap(),
                    kind: ArtifactKind::AppConfig(AppSpecific::from("starlane.io:starlane:core:test:/test/test_app").unwrap())
    };

    pub static ref TEST_ACTOR_CONFIG_ARTIFACT: Artifact = Artifact
    {
                    id: ArtifactId::from("starlane.io:starlane:core:test:1.0.0:/test/test_actor.yaml").unwrap(),
                    kind: ArtifactKind::ActorConfig(ActorKind::Actor(ActorSpecific::from("starlane.io:starlane:core:test:/test/test_actor").unwrap()))
    };
}