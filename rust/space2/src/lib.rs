#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]



#![feature(new_range_api)]
extern crate core;
#[macro_use]
extern crate alloc;

pub mod space;


#[cfg(test)]
pub mod test;

use core::panic::PanicInfo;

#[cfg(not(test))]
#[panic_handler]
fn panic(_panic: &PanicInfo<'_>) -> ! {
    loop {}
}




/*


extern crate alloc;

use alloc::string::String;
use core::ops::Deref;

#[macro_use]
macro_rules! cfg_not_std {
    ($($item:item)*) => {
        $( #[cfg(not(feature = "std"))] $item )*
    }
}

macro_rules! cfg_std {
    ($($item:item)*) => {
        $( #[cfg(feature = "std")] $item )*
    }
}
cfg_not_std! {

use core::panic::PanicInfo;

    #[panic_handler]
    fn panic(_panic: &PanicInfo<'_>) -> ! {
        loop {}
    }

}


#[cfg(test)]
pub mod test {
    #[test]
    fn test() {

    }
}

*/

pub(crate) use core::error::Error as RustErr;
