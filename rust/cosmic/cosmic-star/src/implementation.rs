use cosmic_api::id::StarSub;
use cosmic_driver::DriverFactory;
use crate::driver::DriversBuilder;

pub trait Implementation {
    fn drivers_builder( &self, kind: &StarSub ) -> DriversBuilder;
}