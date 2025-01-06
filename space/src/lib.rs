#![allow(warnings)]

use crate::point::Point;
use once_cell::sync::Lazy;
use std::str::FromStr;

// so macros will work
extern crate self as starlane_space;

#[allow(missing_docs, unused_imports, warnings)]
pub(crate) mod lib {
    mod core {
        #[cfg(not(feature = "std"))]
        pub use core::*;
        #[cfg(feature = "std")]
        pub use std::*;
    }

    pub use self::core::{f32, f64};
    pub use self::core::{i16, i32, i64, i8, isize};
    pub use self::core::{iter, num, ptr, str};
    pub use self::core::{u16, u32, u64, u8, usize};

    #[cfg(any(feature = "std", feature = "alloc"))]
    pub use self::core::{cmp, mem, slice};

    pub use self::core::cell::{Cell, RefCell};
    pub use self::core::clone;
    pub use self::core::cmp::Reverse;
    pub use self::core::convert;
    pub use self::core::default;
    pub use self::core::fmt::{self, Debug, Display, Write as FmtWrite};
    pub use self::core::marker::{self, PhantomData};
    pub use self::core::num::Wrapping;
    pub use self::core::ops::{Bound, Range, RangeFrom, RangeInclusive, RangeTo};
    pub use self::core::option;
    pub use self::core::result;
    pub use self::core::time::Duration;

    #[cfg(all(feature = "alloc", not(feature = "std")))]
    pub use alloc::borrow::{Cow, ToOwned};
    #[cfg(feature = "std")]
    pub use std::borrow::{Cow, ToOwned};

    #[cfg(all(feature = "alloc", not(feature = "std")))]
    pub use alloc::string::{String, ToString};
    #[cfg(feature = "std")]
    pub use std::string::{String, ToString};

    #[cfg(all(feature = "alloc", not(feature = "std")))]
    pub use alloc::vec::Vec;
    #[cfg(feature = "std")]
    pub use std::vec::Vec;

    #[cfg(all(feature = "alloc", not(feature = "std")))]
    pub use alloc::boxed::Box;
    #[cfg(feature = "std")]
    pub use std::boxed::Box;

    #[cfg(all(feature = "rc", feature = "alloc", not(feature = "std")))]
    pub use alloc::rc::{Rc, Weak as RcWeak};
    #[cfg(all(feature = "rc", feature = "std"))]
    pub use std::rc::{Rc, Weak as RcWeak};

    #[cfg(all(feature = "rc", feature = "alloc", not(feature = "std")))]
    pub use alloc::sync::{Arc, Weak as ArcWeak};
    #[cfg(all(feature = "rc", feature = "std"))]
    pub use std::sync::{Arc, Weak as ArcWeak};

    #[cfg(all(feature = "alloc", not(feature = "std")))]
    pub use alloc::collections::{BTreeMap, BTreeSet, BinaryHeap, LinkedList, VecDeque};
    #[cfg(feature = "std")]
    pub use std::collections::{BTreeMap, BTreeSet, BinaryHeap, LinkedList, VecDeque};

    #[cfg(all(not(no_core_cstr), not(feature = "std")))]
    pub use self::core::ffi::CStr;
    #[cfg(feature = "std")]
    pub use std::ffi::CStr;

    #[cfg(all(not(no_core_cstr), feature = "alloc", not(feature = "std")))]
    pub use alloc::ffi::CString;
    #[cfg(feature = "std")]
    pub use std::ffi::CString;

    #[cfg(all(not(no_core_net), not(feature = "std")))]
    pub use self::core::net;
    #[cfg(feature = "std")]
    pub use std::net;

    #[cfg(feature = "std")]
    pub use std::error;

    #[cfg(feature = "std")]
    pub use std::collections::{HashMap, HashSet};
    #[cfg(feature = "std")]
    pub use std::ffi::{OsStr, OsString};
    #[cfg(feature = "std")]
    pub use std::hash::{BuildHasher, Hash};
    #[cfg(feature = "std")]
    pub use std::io::Write;
    #[cfg(feature = "std")]
    pub use std::path::{Path, PathBuf};
    #[cfg(feature = "std")]
    pub use std::sync::{Mutex, RwLock};
    #[cfg(feature = "std")]
    pub use std::time::{SystemTime, UNIX_EPOCH};

    #[cfg(all(feature = "std", no_target_has_atomic, not(no_std_atomic)))]
    pub use std::sync::atomic::{
        AtomicBool, AtomicI16, AtomicI32, AtomicI8, AtomicIsize, AtomicU16, AtomicU32, AtomicU8,
        AtomicUsize, Ordering,
    };
    #[cfg(all(feature = "std", no_target_has_atomic, not(no_std_atomic64)))]
    pub use std::sync::atomic::{AtomicI64, AtomicU64};

    #[cfg(all(feature = "std", not(no_target_has_atomic)))]
    pub use std::sync::atomic::Ordering;
    #[cfg(all(feature = "std", not(no_target_has_atomic), target_has_atomic = "8"))]
    pub use std::sync::atomic::{AtomicBool, AtomicI8, AtomicU8};
    #[cfg(all(feature = "std", not(no_target_has_atomic), target_has_atomic = "16"))]
    pub use std::sync::atomic::{AtomicI16, AtomicU16};
    #[cfg(all(feature = "std", not(no_target_has_atomic), target_has_atomic = "32"))]
    pub use std::sync::atomic::{AtomicI32, AtomicU32};
    #[cfg(all(feature = "std", not(no_target_has_atomic), target_has_atomic = "64"))]
    pub use std::sync::atomic::{AtomicI64, AtomicU64};
    #[cfg(all(feature = "std", not(no_target_has_atomic), target_has_atomic = "ptr"))]
    pub use std::sync::atomic::{AtomicIsize, AtomicUsize};

    #[cfg(not(no_core_num_saturating))]
    pub use self::core::num::Saturating;
}
pub mod artifact;
pub mod asynch;
pub mod command;
pub mod config;
pub mod err;
pub mod fail;
pub mod frame;
pub mod hyper;
pub mod kind;
pub mod parse;
pub mod particle;
pub mod wave;

#[cfg(feature = "kind2")]
pub mod kind2;

pub mod loc;
pub mod log;
pub mod path;
pub mod point;
pub mod security;
pub mod selector;
pub mod settings;
pub mod substance;
pub mod util;
pub mod wasm;

pub mod prelude;
pub mod progress;

pub mod status;

/// `types` mod is a work in progress for the proposed new type system
/// its having some compile problems and isn't as-of-yet used by
/// anything so makes sense to disable it for a while, so I can focus
/// on getting `CI/CD` working
//#[cfg(feature = "types2")]
pub mod types;

#[cfg(test)]
pub mod test;

pub static VERSION: Lazy<semver::Version> =
    Lazy::new(|| semver::Version::from_str(env!("CARGO_PKG_VERSION").trim()).unwrap());

pub static HYPERUSER: Lazy<Point> =
    Lazy::new(|| Point::from_str("hyperspace:users:hyperuser").expect("point"));
pub static ANONYMOUS: Lazy<Point> =
    Lazy::new(|| Point::from_str("hyperspace:users:anonymous").expect("point"));
