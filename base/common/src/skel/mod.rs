/// skel is a `stubbed` reference implementation from which elements should be
/// copied to create a starting point for adding to the `base strata`
///
/// # `Examples`
/// * [Provider] implementations for `Starlane` to add `utilization` and `control`
///   capabilities to an external entity.
///
/// * [Foundation] implementations to expand the environments that `Starlane` can control
///
/// * [base] examples for extending [ProviderKind]'s [Provider] the trait definitions
///   that are common to a [ProviderKind]'s [Platform] and [Foundation]
///
/// * [Partial] examples for adding functionality that spans over multiple [Provider] and/or
///   [Foundation] implementations

use crate::foundation::Foundation;
use crate::platform::prelude::Platform;
use crate::partial::Partial;

pub mod base;
pub mod foundation;

pub mod partial;
mod platform;
