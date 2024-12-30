/// Abstract definition of `Modes`.
///
/// Some implementations of [foundation::Foundation],[foundation::Dependency] and
/// [foundation::Provider] may need to defined different behaviors based on how a resource is being
/// used.`Modes` can be used when the configuration for one of its use cases requires only a subset
/// of another.
///
/// Example: Say you have created a new common [foundation::Dependency] and [foundation::Provider]'s
/// for a `PostgresCluster` and a `Database` instance provider.  Postgres has three use cases and
/// therefore three distinct `Modes`
///
/// ## Mode use cases:
/// * [Mode::Create] -> Provides all details to create and supply a [foundation::Foundation]
///   managed `Postgres Cluster` instance including  download an artifact, install an executable,
///   initialize(setup) including: assigning credentials, creating and mapping local persistent
///   storage, configure a custom port.
///
/// * [Mode::Start] -> After [foundation::Foundation]  Creates the Postgres Cluster--or--determines
///   that a Postgres Cluster has already been created it needs to `Start` the Service instance.
///   Important details for [Mode::Start] are: path of the Postgres Cluster executable and
///   ability to probe the health and availability of the Cluster Instance.
///
/// * [Mode::Connect] -> If the Dependency is already available or freshly created and started ...
///   then all that is needed is to access the cluster via a connection pool. [Mode::Connect] provides
///   the same Credential properties that it used from [Mode::Create].  In only makes sense
///   to configure and define common properties and behaviors across all potential use cases.
///
/// ## Mode Definition = Property+Feature+Functionality
/// The Mode `Definition` encapsulates the set of Properties for the [config::DependencyConfig],
/// and the available `Features` and `Functionalities` that [foundation::Foundation] must support
/// for when operating in that mode.
///
/// ## Titrated Subsets
/// Sequential Modes configure Subsets of the Prior Mode's Definitions.   Note that in our
/// PostgresCluster Mode example [Mode::Create] the union of all Postgres Cluster Dependency
/// Definitions and [Mode::Create] supplies the most minimal subset.
///
/// ## Not Every Mode is Available in every case!
/// The Purpose of modes is to supply the [foundation::Foundation] cascading approaches to
/// achieving the Dependency's Ready state which--in the case of the Postgres Cluster Dependency--
/// is for the [foundation::Foundation] to provide the [platform::Platform] with a database
/// connection pool. And to accomplish the goal not all modes are required depending upon
/// the desired setup.
///
/// Consider again our Postgres example this time it's a scenario where we want to use
/// a `PostgresCluster` that is *already* available, or perhaps stringent requirements require
/// a special `PostgresCluster` setup that isn't supported by any [foundation::Foundation]
/// implementation... In those cases the [foundation::config::DependencyConfig] only needs be
/// configured for [Mode::Connect] and details like: mounting persistent storage, port assignment
/// credential setup, etc... do not need to be furnished to `Starlane`  In this case [Mode::Create]
/// should not be created and therefore will not be available
///
/// ## Modes for Security
/// In some cases it may not be desirable to supply any agent with sensitive information
/// or features therefor some configured Modes may be illegal for an agent to access or execute.
/// Oh... Security for Modes is NOT implemented at the time of this writing, and I'm not sure
/// exactly how it will be implemented. I mention the as-of-yet vaporous security mechanism because
/// an imported prerequisite for any security implementation is a means to create bounds around
/// a resources definition.  Modes are used to define all that is required for [foundation::Foundation]
/// to accomplish an aspect of the *total* resources definitions... Exactly what our future security
/// uh... *thingy* will need to comprehend in order to someday keep out the riffraff.
///
/// ## Mode Ordering
///
/// [Mode] ordering goes from the largest definition to the smallest, however, it is important
/// to understand that the [foundation::Dependency] goal is pursued trying mode's in *reverse* order.
/// The reverse order may seem unintuitive because you can't [Mode::Connect] to the Cluster before it goes through
/// [Mode::Create] and [Mode::Start] is run. To understand the reverse order rational consider
/// every time [foundation::Foundation] is started it makes no assumptions about the present
/// state of the infrastructure it manages.  [foundation::Foundation] attempts to get a database
/// connection by trying modes from least to most... it tries each mode in turn until its goal
/// is reached or if the final available mode is attempted and fails.
///```
///  pub enum Mode {
///     Connect,
///     Start,
///      Create
///  }
///  ```
/// So, here is the sequence that [foundation::Foundation] follows on startup:
///
/// * [Mode::Connect] -> Try to connect to the database
/// * [Mode::Start] -> This mode may be capable of probing the health of the Postgres Cluster
///   by a custom foundation implementation ... for instance if we are running the
///   [foundation::implementation::docker_daemon_foundation], which provides infrastructure
///   on a local development computer via `Docker` the probe would take the form of checking
///   if the docker daemon is running and healthy, etc.  If the probe determines that the
///   Postgres Docker instance is not running then it attempts to start it (but does so without
///   doing a `docker pull "some-postgres-images:latest"`
/// * [Mode::Create] -> Checks if the Postgres Docker image artifact is downloaded, installed and
///   initialized and then performs the necessary actions to download, install and initialize as needed.
///   IF [Mode::Create] succeeds... the [foundation::Foundation] will then reverse its mode traversal

use crate::base;
use base::config;
use base::foundation;
use base::platform;

