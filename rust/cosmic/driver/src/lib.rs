use cosmic_space::err::SpaceErr;
use cosmic_space::kind3::Kind;

pub struct DriverSkel {

}

pub trait Driver {
   type Skel;
   type State;
   fn kind(&self) -> Kind;
}

pub trait DriverLifecycle {
   fn init(&self) -> Result<(),SpaceErr> {
      Ok(())
   }
}

