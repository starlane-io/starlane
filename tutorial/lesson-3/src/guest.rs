use cosmic_space::particle::Details;
use std::sync::Arc;
use mechtron::err::GuestErr;
use crate::platform::MyPlatform;

#[no_mangle]
pub extern "C" fn mechtron_guest(details: Details) -> Result<Arc<dyn mechtron::Guest>, GuestErr> {
    Ok(Arc::new(mechtron::guest::Guest::new(
        details,
        MyPlatform::new(),
    )?))
}
