//! ## Projecture
//!
//! **This is in a proof of concept state and also internally uses a lot of not yet battle-tested unsafe code
//! so use it on your own risk meanwhile if you are good with unsafe rust i would appreciate a soundness review**
//!
//! Allows to do an arbitrary projections without procedural macros, and as such does not have additional
//! requirements on target struct, so in comparison to other crates that do similar things
//! if target struct is located in external crate that crate does not have to explicitly add a support such projection.
//!
//! Although as of now this crate has a one serious drawback - it can't support enums.
//!
//! #### Currently can do following type of projections
//!  - Destructuring projection (more powerful then what you can do in standard rust with regular `let <pattern>`)
//!  - Reference projection (more powerful then what you can do in standard rust with regular `let <pattern>`)
//!  - Mutable reference projection (more powerful then what you can do in standard rust with regular `let <pattern>`)
//!  - Pin projection
//!  - Cell projection
//!  - MaybeUninit projection
//!
//! See [`project`]! macro for usage examples.
//!
//! Also allows dependent crates to define their own projections via traits.
//!
//!
//! MSRV: 1.51

use std::cell::Cell;
use std::marker::PhantomData;
use std::mem::{transmute_copy, ManuallyDrop, MaybeUninit};
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::{mem, ptr};

#[doc(hidden)]
pub unsafe trait Preprocess {
    type Output;
    fn preprocess(&self) -> Self::Output;
}

///Implement this on your reference type that you want to work with this crate (like `Pin` or `std::cell:Ref`)
pub unsafe trait MarkerNonOwned {}

// wrapper to prevent overlapping implementations
#[doc(hidden)]
#[repr(transparent)]
pub struct Owned<T>(ManuallyDrop<T>);
unsafe impl<T> Preprocess for ManuallyDrop<T> {
    type Output = Owned<T>;

    fn preprocess(&self) -> Self::Output {
        unsafe { transmute_copy(self) }
    }
}

unsafe impl<T: MarkerNonOwned> Preprocess for &ManuallyDrop<T> {
    type Output = T;

    fn preprocess(&self) -> Self::Output {
        unsafe { transmute_copy(self) }
    }
}

// wrapper to prevent overlapping implementations
#[doc(hidden)]
#[repr(transparent)]
pub struct Helper<T>(T);
unsafe impl<'a, T> Preprocess for &&ManuallyDrop<&'a T> {
    type Output = Helper<&'a T>;

    fn preprocess(&self) -> Self::Output {
        unsafe { transmute_copy(**self) }
    }
}

unsafe impl<'a, T> Preprocess for &&ManuallyDrop<&'a mut T> {
    type Output = Helper<&'a mut T>;

    fn preprocess(&self) -> Self::Output {
        unsafe { transmute_copy(**self) }
    }
}

///Implement this if there is a user-defined field wrapper type
pub unsafe trait CustomWrapper {
    /// `Self` but wrapped in `#[repr(transparent)` wrapper
    type Output;
}
unsafe impl<T: CustomWrapper> Preprocess for &&&ManuallyDrop<T> {
    type Output = T::Output;

    fn preprocess(&self) -> Self::Output {
        unsafe { transmute_copy(self) }
    }
}

// --------------

/// Trait to get raw pointer to underlying struct
pub unsafe trait Projectable {
    /// Inner type to which projection will be applied
    type Target;
    /// Marker type to track information about the type of projection being done
    /// Should implement `ProjectableMarker`
    type Marker;

    /// Get raw pointer to underlying struct
    fn get_raw(&self) -> (*mut Self::Target, Self::Marker);
}

/// Trait to wrap raw pointer to a field with a type that corresponds to a projection being done
pub trait ProjectableMarker<T> {
    /// Wrapped pointer type
    type Output;
    /// Wraps raw pointer to a field with a type that corresponds to a projection being done
    unsafe fn from_raw(&self, raw: *mut T) -> Self::Output;
}

// todo make dereference also user overridable
#[doc(hidden)]
pub struct DerefMarkerWrapper<T>(PhantomData<T>);
impl<T> DerefMarkerWrapper<T> {
    #[doc(hidden)]
    pub unsafe fn new(value: &T) -> Self {
        Self(PhantomData)
    }
}

#[doc(hidden)]
pub struct MaybeDerefMarkerWrapper<'a, T>(&'a T);
impl<'a, T> MaybeDerefMarkerWrapper<'a, T> {
    #[doc(hidden)]
    pub unsafe fn new(value: &'a T) -> Self {
        Self(value)
    }
}
impl<T, M: ProjectableMarker<T>> ProjectableMarker<T> for MaybeDerefMarkerWrapper<'_, M> {
    type Output = M::Output;

    unsafe fn from_raw(&self, raw: *mut T) -> Self::Output {
        self.0.from_raw(raw)
    }
}
impl<T, M> ProjectableMarker<T> for &MaybeDerefMarkerWrapper<'_, M>
where
    DerefMarkerWrapper<M>: ProjectableMarker<T>,
{
    type Output = <DerefMarkerWrapper<M> as ProjectableMarker<T>>::Output;

    unsafe fn from_raw(&self, raw: *mut T) -> Self::Output {
        DerefMarkerWrapper::new(self.0).from_raw(raw)
    }
}

//--------------
unsafe impl<T> Projectable for Owned<T> {
    type Target = T;
    type Marker = [(); 0];

    fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
        (self as *const Self as *mut Self as *mut T, [])
    }
}
impl<T> ProjectableMarker<T> for [(); 0] {
    type Output = T;

    unsafe fn from_raw(&self, raw: *mut T) -> Self::Output {
        ptr::read(raw as *const T as *const _)
    }
}
// pub trait DerefOwned: DerefMut {
//     type WithManuallyDrop: DerefMut<Target = ManuallyDrop<Self::Target>>;
//     fn into_md(self) -> Self::WithManuallyDrop;
// }
// impl<T> DerefOwned for Box<T> {
//     type WithManuallyDrop = Box<ManuallyDrop<T>>;
//
//     fn into_md(self) -> Self::WithManuallyDrop {
//         unsafe { mem::transmute(self) }
//     }
// }
/// Implement this if your type can be unwrapped on a dereference operation when doing
/// destructuring projection
pub trait Unwrap {
    type Target;
    fn unwrap(self) -> Self::Target;
}
impl<T> Unwrap for Box<T> {
    type Target = T;

    fn unwrap(self) -> Self::Target {
        *self
    }
}
impl<T: Unwrap> ProjectableMarker<T> for DerefMarkerWrapper<[(); 0]> {
    type Output = T::Target;

    unsafe fn from_raw(&self, raw: *mut T) -> Self::Output {
        ptr::read(raw as *const T).unwrap()
    }
}

//---------------------
unsafe impl<'a, T> Projectable for Helper<&'a mut T> {
    type Target = T;
    type Marker = &'a mut ();

    fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
        (
            unsafe { ptr::read(self as *const _ as *const *mut Self::Target) },
            Box::leak(Box::new(())),
        )
    }
}
impl<'a, T: 'a> ProjectableMarker<T> for &'a mut () {
    type Output = &'a mut T;

    unsafe fn from_raw(&self, raw: *mut T) -> Self::Output {
        &mut *raw
    }
}
impl<'a, T: DerefMut + 'a> ProjectableMarker<T> for DerefMarkerWrapper<&'a mut ()> {
    type Output = &'a mut T::Target;

    unsafe fn from_raw(&self, raw: *mut T) -> Self::Output {
        &mut **raw
    }
}

//---------------------
unsafe impl<'a, T> Projectable for Helper<&'a T> {
    type Target = T;
    type Marker = &'a ();

    fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
        (self.0 as *const _ as _, Box::leak(Box::new(())))
    }
}
impl<'a, T: 'a> ProjectableMarker<T> for &'a () {
    type Output = &'a T;

    unsafe fn from_raw(&self, raw: *mut T) -> Self::Output {
        &*raw
    }
}
impl<'a, T: Deref + 'a> ProjectableMarker<T> for DerefMarkerWrapper<&'a ()> {
    type Output = &'a T::Target;

    unsafe fn from_raw(&self, raw: *mut T) -> Self::Output {
        &**raw
    }
}

//---------------------
unsafe impl<'a, T> Projectable for &Helper<&'a Cell<T>> {
    type Target = T;
    type Marker = &'a Cell<()>;

    fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
        (*self as *const _ as _, Box::leak(Box::new(Cell::new(()))))
    }
}
impl<'a, T: 'a> ProjectableMarker<T> for &'a Cell<()> {
    type Output = &'a Cell<T>;

    unsafe fn from_raw(&self, raw: *mut T) -> Self::Output {
        &*(raw as *mut Cell<T>)
    }
}
// unimplemented because it would be unsound
// impl<'a, T: 'a> ProjectableMarker<T> for DerefMarkerWrapper<&'a Cell<()>> {

//---------------------
unsafe impl<'a, T> Projectable for &Helper<&'a mut MaybeUninit<T>> {
    type Target = T;
    type Marker = &'a mut MaybeUninit<()>;

    fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
        (
            unsafe { ptr::read(self as *const _ as *const *mut Self::Target) },
            Box::leak(Box::new(MaybeUninit::new(()))),
        )
    }
}
impl<'a, T: 'a> ProjectableMarker<T> for &'a mut MaybeUninit<()> {
    type Output = &'a mut MaybeUninit<T>;

    unsafe fn from_raw(&self, raw: *mut T) -> Self::Output {
        &mut *(raw as *mut MaybeUninit<T>)
    }
}
// unimplemented because it would be unsound
//impl<'a, T: 'a> ProjectableMarker<T> for &'a mut MaybeUninit<()> {

//---------------------
// unfortunately raw pointer projections are unsound
// unsafe impl<T> MarkerNonOwned for *mut T {}
// unsafe impl<T> Projectable for *mut T {
//     type Target = T;
//     type Marker = *mut ();
//
//     fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
//         (*self, &() as *const () as _)
//     }
// }
// impl<T> ProjectableMarker<T> for *mut () {
//     type Output = *mut T;
//
//     unsafe fn from_raw(&self, raw: *mut T) -> Self::Output {
//         raw
//     }
// }
//
// //---------------------
// unsafe impl<T> MarkerNonOwned for *const T {}
// unsafe impl<T> Projectable for *const T {
//     type Target = T;
//     type Marker = *const ();
//
//     fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
//         (*self as _, &())
//     }
// }
// impl<T> ProjectableMarker<T> for *const () {
//     type Output = *const T;
//
//     unsafe fn from_raw(&self, raw: *const T) -> Self::Output {
//         raw
//     }
// }

//---------------------
unsafe impl<T> MarkerNonOwned for Pin<T> {}
macro_rules! impl_pin {
    ($($maybe_mut:tt)?) => {
         unsafe impl<'a, T> Projectable for Pin<&'a $($maybe_mut)? T> {
            type Target = T;
            type Marker = Pin<&'a $($maybe_mut)? ()>;

            fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
                (
                    unsafe { ptr::read(self as *const _ as *const *mut Self::Target) },
                    Pin::new(Box::leak(Box::new(()))),
                )
            }
        }
        impl<'a, T: 'a> ProjectableMarker<T> for Pin<&'a $($maybe_mut)? ()> {
            type Output = Pin<&'a $($maybe_mut)? T>;

            unsafe fn from_raw(&self, raw: *mut T) -> Self::Output {
                Pin::new_unchecked(&$($maybe_mut)? *raw)
            }
        }
        impl<'a, T: Unpin + DerefMut + 'a> ProjectableMarker<T> for DerefMarkerWrapper<Pin<&'a $($maybe_mut)? ()>> {
            type Output = &'a $($maybe_mut)? T::Target;

            unsafe fn from_raw(&self, raw: *mut T) -> Self::Output {
                &$($maybe_mut)? **raw
            }
        }

        #[allow(drop_bounds)]
        unsafe impl<'a, T: Drop> Projectable for &Pin<&'a $($maybe_mut)? T> {
            type Target = T;
            type Marker = Pin<&'a $($maybe_mut)? ()>;

            fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
                panic!("struct must also implement PinDrop")
            }
        }
        #[allow(drop_bounds)]
        unsafe impl<'a, T: Drop + PinDrop> Projectable for &&Pin<&'a $($maybe_mut)? T> {
            type Target = T;
            type Marker = Pin<&'a $($maybe_mut)? ()>;

            fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
                (
                    unsafe { ptr::read(self as *const _ as *const *mut Self::Target) },
                    Pin::new(Box::leak(Box::new(()))),
                )
            }
        }
        #[cfg(feature = "pin-project")]
        unsafe impl<'a, T: pin_project::__private::PinnedDrop> Projectable for &&&Pin<&'a $($maybe_mut)? T> {
            type Target = T;
            type Marker = Pin<&'a $($maybe_mut)? ()>;

            fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
                panic!("this crate is incompatible with pin-project when both are used on the same struct")
            }
        }

    };
}

impl_pin! {}
impl_pin! { mut }

/// for Pin projection to work soundly if struct wants to implement custom Drop it needs to
/// always go through `Pin<&mut Self>`. So `Drop` implementation must directly delegate to `PinDrop`.
/// You can use [`pin_drop`] macro to do that without `unsafe`
pub trait PinDrop {
    unsafe fn drop(self: Pin<&mut Self>);
}

//todo
#[macro_export]
macro_rules! pin_drop {
    (impl PinDrop for $name:ident {}) => {};
}

//---------------------
/// Implement that if you need to do some kind of post processing
pub trait FinalizeProjection {
    type Output;
    unsafe fn finalize(&self) -> Self::Output;
}
impl<'a, T: Unpin> FinalizeProjection for Pin<&'a mut T> {
    type Output = &'a mut T;

    unsafe fn finalize(&self) -> Self::Output {
        transmute_copy(self)
    }
}
impl<'a, T> FinalizeProjection for &Pin<&'a mut Unpinned<T>> {
    type Output = &'a mut T;

    unsafe fn finalize(&self) -> Self::Output {
        transmute_copy(*self)
    }
}
//---------------------
#[doc(hidden)]
pub trait Finalizer {
    type Output;
    unsafe fn call_finalize(&self) -> Self::Output;
}
impl<T> Finalizer for ManuallyDrop<T> {
    type Output = T;

    unsafe fn call_finalize(&self) -> Self::Output {
        transmute_copy(self)
    }
}
impl<T: FinalizeProjection> Finalizer for &ManuallyDrop<T> {
    type Output = T::Output;

    unsafe fn call_finalize(&self) -> Self::Output {
        self.finalize()
    }
}
impl<'a, T> Finalizer for &&'a ManuallyDrop<T>
where
    &'a T: FinalizeProjection,
{
    type Output = <&'a T as FinalizeProjection>::Output;

    unsafe fn call_finalize(&self) -> Self::Output {
        transmute_copy::<_, &T>(*self).finalize()
    }
}
impl<'a, 'b, T> Finalizer for &&'a &'b ManuallyDrop<T>
where
    &'a &'b T: FinalizeProjection,
{
    type Output = <&'a &'b T as FinalizeProjection>::Output;

    unsafe fn call_finalize(&self) -> Self::Output {
        transmute_copy::<_, &&T>(*self).finalize()
    }
}
impl<'a, 'b, 'c, T> Finalizer for &&'a &'b &'c ManuallyDrop<T>
where
    &'a &'b &'c T: FinalizeProjection,
{
    type Output = <&'a &'b &'c T as FinalizeProjection>::Output;

    unsafe fn call_finalize(&self) -> Self::Output {
        transmute_copy::<_, &&&T>(*self).finalize()
    }
}

//---------------------
/// Transparent wrapper to indicate that a type should not be pin projected.
/// It will not be p
#[repr(transparent)]
pub struct Unpinned<T>(T);
impl<T> Unpin for Unpinned<T> {}
impl<T> Deref for Unpinned<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<T> DerefMut for Unpinned<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Macro to do all kinds of projections
///
/// Has two modes:
///  - `let` syntax very similar to regular rust's `let <pattern> = <expr>`, see examples below.
///  - single field projection `project!(<variable> -> <field>)` or `project!((<expression>) -> <field>)`
///
/// ```rust
/// #   use std::cell::Cell;
/// #   use std::mem::MaybeUninit;
/// #   use projecture::project;
///     struct Foo {
///         x: usize,
///         y: usize,
///     }
///     let cell = Cell::new(Foo { x: 0, y: 0 });
///     let cell_ref = &cell;
///     // project only one field
///     project!(cell_ref -> x).set(1);
///     // do a full let destructuring matching
///     project!(let Foo { x, y } = &cell);
///     x.set(1);
///     y.set(1);
///
///     let foo = Foo { x: 0, y: 0 };
///     project!(let Foo { x, y } = &foo);
///     let x: &usize = x;
///     let y: &usize = y;
///
///     project!(let Foo { x, y } = foo);
///     let x: usize = x;
///     let y: usize = y;
///
///     let mut foo = Foo { x: 0, y: 0 };
///     project!(let Foo { x, y } = &mut foo);
///     *x = 1;
///     *y = 1;
///
///     let mut cell = MaybeUninit::<Foo>::uninit();
///     project!(let Foo { x:x, y:y } = &mut cell);
///     x.write(1);
///     y.write(1);
///
///     struct Bar(usize, usize);
///     let mut foo = Bar(1, 2);
///     project! { let Bar(x,y) = &mut foo }
///     project! { let Bar{ 0: x, 1: y } = &mut foo }
///     let foo_mut = &mut foo;
///     *project!(foo_mut -> 0) = 1;
///     *project!((&mut foo) -> 0) = 1;
/// ```
/// It supports dereferencing during pattern matching
/// ```rust
/// #    use projecture::project;
///     struct Foo {
///         x: Box<usize>,
///         y: Box<usize>,
///     }
///     let mut foo = Foo {
///         x: Box::new(0),
///         y: Box::new(0),
///     };
///     project!( let Foo{ x: *x, y: *y }  = &mut foo );
///     let x: &mut usize = x;
///     project!( let Foo{ x: *x, y: *y }  = foo );
///     let x: usize = x;
/// ```
/// Also `Pin` projection:
/// ```rust
/// #    use std::marker::PhantomPinned;
/// #    use std::pin::Pin;
/// #    use projecture::{project, Unpinned};
///     struct Foo<T, U: Unpin, V> {
///         a: usize,
///         b: T,
///         c: U,
///         d: Unpinned<V>,
///         e: PhantomPinned,
///     }
///
///     fn test<T, U: Unpin, V>(foo: Pin<&mut Foo<T, U, V>>) {
///         project!(let Foo{ a,b,c,d,e } = foo);
///         let a: &mut usize = a;
///         let b: Pin<&mut T> = b;
///         let c: &mut U = c;
///         let d: &mut V = d;
///         let e: Pin<&mut PhantomPinned> = e;
///     }
/// ```
#[macro_export]
macro_rules! project {
    // ( { $($field:ident),+  } = $target:expr) => {};
    (let $struct:ident { $($fields:tt)+ } = $val: expr) => {
        let var = core::mem::ManuallyDrop::new($val);
        let var = {
            use $crate::Preprocess;
            (&&&var).preprocess()
        };

        let (ptr,marker) = {
            use $crate::Projectable;
            (&&&&&&& var).get_raw()
        };
        if false{
            let $struct { .. } = unsafe { &*ptr };
        }
        $crate::project_fields_inner! { [ptr marker] $($fields)+ }
    };
    (let $struct:ident ( $($fields:tt)+ ) = $val: expr) => {
        let var = core::mem::ManuallyDrop::new($val);
        let var = {
            use $crate::Preprocess;
            (&&&var).preprocess()
        };
        let (ptr,marker) = {
            use $crate::Projectable;
            (&&&&&&& var).get_raw()
        };
        if false{
            let $struct { .. } = unsafe{ &*ptr };
        }

        $crate::project_tuple_fields! { [ptr marker] [] [] $($fields)+ }
    };
    ( $var:ident -> $field:tt) => { $crate::project! { ($var) -> $field} };
    (( $var:expr ) -> $field:tt) => {
        {
            let var = core::mem::ManuallyDrop::new($var);
            let var = {
                use $crate::Preprocess;
                (&&&var).preprocess()
            };

            let (ptr,marker) = {
                use $crate::Projectable;
                (&&&&&&& var).get_raw()
            };
            $crate::project_fields_inner! { [ptr marker] $field : temp_name }
            temp_name
        }
    };

}

#[doc(hidden)]
#[macro_export]
macro_rules! project_tuple_fields {
    ([$ptr:ident $marker:ident] [$($idx:tt)*] [$($pattern:tt)*] , $($tail:tt)* ) => {
        $crate::project_fields_inner! { [$ptr $marker] { $($idx)* } : $($pattern)* }
        $crate::project_tuple_fields! { [$ptr $marker] [$($idx)* !] [] $($tail)* }
    };
    ([$ptr:ident $marker:ident] [$($idx:tt)*] [$($pattern:tt)*]  $next:tt $($tail:tt)* ) => {
        $crate::project_tuple_fields! { [$ptr $marker] [$($idx)*] [$($pattern)* $next] $($tail)*  }
    };

    ([$ptr:ident $marker:ident] [$($idx:tt)*] [$($pattern:tt)*] ) => {
        $crate::project_fields_inner! { [$ptr $marker] { $($idx)* } : $($pattern)* }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! field_addr {
    ((*$ptr:ident).) => {
        core::ptr::addr_of_mut!((*$ptr).1)
    };
    ((*$ptr:ident).!) => {
        core::ptr::addr_of_mut!((*$ptr).1)
    };
    ((*$ptr:ident).!!) => {
        core::ptr::addr_of_mut!((*$ptr).2)
    };
    ((*$ptr:ident).!!!) => {
        core::ptr::addr_of_mut!((*$ptr).3)
    };
    ((*$ptr:ident).!!!!) => {
        core::ptr::addr_of_mut!((*$ptr).4)
    };
    ((*$ptr:ident).!!!!!) => {
        core::ptr::addr_of_mut!((*$ptr).5)
    };
    ((*$ptr:ident).!!!!!!) => {
        core::ptr::addr_of_mut!((*$ptr).6)
    };
    ((*$ptr:ident).!!!!!!!) => {
        core::ptr::addr_of_mut!((*$ptr).7)
    };
    ((*$ptr:ident).!!!!!!!!) => {
        core::ptr::addr_of_mut!((*$ptr).8)
    };
    ((*$ptr:ident).!!!!!!!!!) => {
        core::ptr::addr_of_mut!((*$ptr).9)
    };
    ((*$ptr:ident).!!!!!!!!!!) => {
        core::ptr::addr_of_mut!((*$ptr).10)
    };
    ((*$ptr:ident).$field:tt) => {
        core::ptr::addr_of_mut!((*$ptr).$field)
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! project_fields_inner {
    ( [$ptr:tt $marker:ident] $field:tt : $new_name:ident) => { $crate::project_fields_inner! { [$ptr $marker] $field : $new_name , } };
    ( [$ptr:tt $marker:ident] $field:tt : * $new_name:ident) => { $crate::project_fields_inner! {[$ptr $marker] $field : * $new_name , }};
    // ( [$ptr:tt $marker:ident] $field:tt : *? $new_name:ident) => { $crate::project_fields_inner! {[$ptr $marker] $field : *? $new_name , }};
    ( [$ptr:tt $marker:ident] { $($field:tt)* } : $new_name:ident , $($tail:tt)*) => {
        let $new_name = unsafe {
            use $crate::{ProjectableMarker,Finalizer};
            let tmp = core::mem::ManuallyDrop::new($marker.from_raw($crate::field_addr!((*$ptr). $($field)* )));
            (&&&& tmp).call_finalize()
        };
        $crate::project_fields_inner!{ [$ptr $marker] $($tail)* }
    };
    ( [$ptr:tt $marker:ident] { $($field:tt)* } : * $new_name:ident , $($tail:tt)*) => {
        let $new_name = unsafe {
            use $crate::{ProjectableMarker,Finalizer};
            let deref_marker = $crate::DerefMarkerWrapper::new(&$marker);
            let tmp = core::mem::ManuallyDrop::new(deref_marker.from_raw($crate::field_addr!((*$ptr). $($field)* )));
            (&&&& tmp).call_finalize()
        };
        $crate::project_fields_inner!{ [$ptr $marker] $($tail)* }
    };
    // ( [$ptr:tt $marker:ident] { $($field:tt)* } : *? $new_name:ident , $($tail:tt)*) => {
    //     let $new_name = unsafe {
    //         use $crate::{ProjectableMarker,Finalizer};
    //         let deref_marker = $crate::MaybeDerefMarkerWrapper::new(&$marker);
    //         let tmp = core::mem::ManuallyDrop::new((&&deref_marker).from_raw($crate::field_addr!((*$ptr). $($field)* )));
    //         (&&&& tmp).call_finalize()
    //     };
    //     $crate::project_fields_inner!{ [$ptr $marker] $($tail)* }
    // };
    ( [$ptr:tt $marker:ident] $field:tt : $($tail:tt)* ) => { $crate::project_fields_inner! {[$ptr $marker] { $field } : $($tail)* }};
    ( [$ptr:tt $marker:ident] * $field:tt $($tail:tt)*) => { $crate::project_fields_inner! { [$ptr $marker] $field : * $field $($tail)* } };
    ( [$ptr:tt $marker:ident] $field:tt $($tail:tt)*) => { $crate::project_fields_inner! { [$ptr $marker] $field : $field $($tail)* } };
    ( [$ptr:tt $marker:ident] ) => {};
}

//todo:
// reborrow on ->
// option projection
// pattern matching
// nested projection
