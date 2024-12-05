use downcast_rs::{impl_downcast, DowncastSync};

pub trait PartialConfig: DowncastSync { }

impl_downcast!(sync PartialConfig);
