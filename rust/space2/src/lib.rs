#![cfg_attr(not(feature = "std"), no_std)]
#![cfg(feature = "alloc")]
#![macro_use]
extern crate alloc;

#[cfg_attr(not(feature = "std"), no_std)]
#[cfg(feature = "alloc")]
#[feature(new_range_api)]
extern crate core;




/// Lib module to re-export everything needed from `std` or `core`/`alloc`. This is how `serde` does
/// it, albeit there it is not public.
pub mod lib {
    /// `std` facade allowing `std`/`core` to be interchangeable. Reexports `alloc` crate optionally,
    /// as well as `core` or `std`
    #[cfg(not(feature = "std"))]
    /// internal std exports for no_std compatibility
    pub mod std {
        #[doc(hidden)]
        #[cfg(not(feature = "alloc"))]
        pub use core::borrow;

        #[cfg(feature = "alloc")]
        #[doc(hidden)]
        pub use alloc::{borrow, boxed, string, vec,sync };

        #[doc(hidden)]
        pub use core::{cmp, convert, fmt, iter, mem, num, ops, option, result, slice, str};

        /// internal reproduction of std prelude
        #[doc(hidden)]
        pub mod prelude {
            pub use core::prelude as v1;
        }
    }

    #[cfg(feature = "std")]
    /// internal std exports for no_std compatibility
    pub mod std {
        #[doc(hidden)]
        pub use std::{
            alloc, borrow, boxed, cmp, collections, convert, fmt, hash, iter, mem, num, ops, option,
            result, slice, str, string, vec,
        };

        /// internal reproduction of std prelude
        #[doc(hidden)]
        pub mod prelude {
            pub use std::prelude as v1;
        }
    }
}









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

use crate::lib::std::string::String;
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
