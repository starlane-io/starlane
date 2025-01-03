//! defined here are the final touches to the [starlane_base] strata: [Platform] and [Foundation].
//! The benefit of the base strata is make starlane as extensible and abstract as possible,
//! however, this implementation is doing thing that seem in defiance of the base strata's
//! architectural approach. *THIS* [base](self) was coded in haste to create a `Poof of Concept`
//! for `Starlane` ... the implementation is meant to act as a facade for a limited enumeration
//! of use cases.
//!
//! The plan to slowly bring this [base](self) implementation into full compliance over multiple
//! releases as the support infrastructure matures.

use starlane_base::Foundation;
use starlane_base::Platform;

pub mod platform;
pub mod foundation;