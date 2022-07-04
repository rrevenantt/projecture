//! ```rust
//! # use core::sync::atomic::Ordering;
//! # use std::sync::Mutex;
//! # use atomic::Atomic;
//! # use projecture::atomic::NonAtomicWindow;
//! # use projecture::project;
//! struct Foo{
//!     f1:f64,
//!     f2:f64,
//!     f3:NonAtomicWindow<Mutex<String>>
//! }
//!
//! let x = Atomic::new(Foo{f1:0.0,f2:0.0 ,f3: NonAtomicWindow(Mutex::new("".to_string()))});
//! project!(let Foo { f1, f2, f3 } = &x);
//! f1.store(1.0,Ordering::Relaxed);
//! f2.store(1.0,Ordering::Relaxed);
//! let f3:&Mutex<String> = f3;
//! ```
//!
//!

use crate::{CustomWrapper, FinalizeProjection, Marker, Projectable, ProjectableMarker};
use atomic::Atomic;
use core::mem::transmute_copy;

unsafe impl<'a, T> CustomWrapper for &'a Atomic<T> {
    type Output = Whatever<&'a Atomic<T>>;
}

// transparent wrapper to work around orphan rules
#[repr(transparent)]
pub struct Whatever<T>(T);

unsafe impl<'a, T> Projectable for Whatever<&'a Atomic<T>> {
    type Target = T;
    type Marker = Marker<&'a Atomic<()>>;

    fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
        (self.0 as *const _ as *const T as _, Marker::new())
    }
}

impl<'a, T: 'a> ProjectableMarker<T> for Marker<&'a Atomic<()>> {
    type Output = &'a Atomic<T>;

    unsafe fn from_raw(&self, raw: *mut T) -> Self::Output {
        &*(raw as *mut Atomic<T>)
    }
}

#[repr(transparent)]
pub struct NonAtomicWindow<T>(pub T);

impl<'a, T> FinalizeProjection for &&'a Atomic<NonAtomicWindow<T>> {
    type Output = &'a T;

    unsafe fn finalize(&self) -> Self::Output {
        transmute_copy(*self)
    }
}
