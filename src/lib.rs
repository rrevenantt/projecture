#![no_std]
#![doc = include_str!("../README.md")]
#![cfg_attr(feature = "nightly", feature(dispatch_from_dyn))]
#![cfg_attr(feature = "nightly", feature(unsize))]
#![cfg_attr(feature = "nightly", feature(arbitrary_self_types))]
#![cfg_attr(feature = "nightly", feature(dropck_eyepatch))]

#[cfg(feature = "std")]
extern crate alloc;

#[cfg(feature = "std")]
use alloc::boxed::Box;
use core::cell::{Cell, UnsafeCell};
use core::convert::Infallible;
use core::marker::{PhantomData, PhantomPinned};
use core::mem::{transmute_copy, ManuallyDrop, MaybeUninit};
use core::ops::{Deref, DerefMut};
use core::pin::Pin;
use core::ptr;
use core::ptr::{drop_in_place, null_mut, NonNull};

/// Support for atomic types projection
#[cfg(feature = "atomic")]
pub mod atomic;

// #[doc(inline)]
pub use pin::*;
/// Pin projection support
pub mod pin;

// #[doc(hidden)]
// pub use memoffset::*;

pub use option::OptionMarker;
mod option;
mod refcell;

pub mod generic;

// helper to wrap `T` `&T` and `&mut T` to prevent conflicting implementations when doing autoderef specialization
#[doc(hidden)]
pub unsafe trait Preprocess {
    type Output;
    fn preprocess(&self) -> Self::Output;
}

// ///Implement this on your reference type that you want to work with this crate (like `Pin` or `std::cell:Ref`)
// pub unsafe trait MarkerNonOwned {}

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

// unsafe impl<T: MarkerNonOwned> Preprocess for &ManuallyDrop<T> {
//     // unsafe impl<T: Reborrow> Preprocess for &ManuallyDrop<T> {
//     // type Output = T::Reborrowed;
//     type Output = T;
//
//     fn preprocess(&self) -> Self::Output {
//         unsafe { transmute_copy(self) }
//     }
// }

// wrapper to prevent overlapping implementations
#[doc(hidden)]
#[repr(transparent)]
pub struct Helper<T>(T);
impl<T: Deref> Deref for Helper<T> {
    type Target = T::Target;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}
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

///Trait to, if necessary, transparently wrap type to prevent conflicting implementations
pub unsafe trait CustomWrapper {
    /// `Self` but wrapped in `#[repr(transparent)]` wrapper,
    /// or just `Self` if there is no problems with conflicting implementations
    type Output;
}
unsafe impl<T: CustomWrapper> Preprocess for &&&ManuallyDrop<T> {
    type Output = T::Output;

    fn preprocess(&self) -> Self::Output {
        unsafe { transmute_copy(***self) }
    }
}
unsafe impl<'a, T> Preprocess for &&&&'a ManuallyDrop<T>
where
    &'a T: CustomWrapper,
{
    type Output = <&'a T as CustomWrapper>::Output;

    fn preprocess(&self) -> Self::Output {
        unsafe { transmute_copy(****self) }
    }
}
unsafe impl<'a, 'b, T> Preprocess for &&&&'a &'b ManuallyDrop<T>
where
    &'a &'b T: CustomWrapper,
{
    type Output = <&'a &'b T as CustomWrapper>::Output;

    fn preprocess(&self) -> Self::Output {
        unsafe { transmute_copy(*****self) }
    }
}

//-----------------
// ///Implement this if you want your pointer type to be reborrowed on `->` operation in `project` macro
// pub unsafe trait Reborrow<'a> {
//     type Reborrowed;
//     fn reborrow(&'a mut self) -> Self::Reborrowed;
// }
// unsafe impl<'a, T: 'a> Reborrow<'a> for &mut T {
//     type Reborrowed = &'a mut T;
//
//     fn reborrow(&'a mut self) -> Self::Reborrowed {
//         &mut *self
//     }
// }
// unsafe impl<'a, T: 'a> Reborrow<'a> for &T {
//     type Reborrowed = &'a T;
//
//     fn reborrow(&'a mut self) -> Self::Reborrowed {
//         &mut *self
//     }
// }
// unsafe impl<'a, T: 'a> Reborrow<'a> for Pin<&mut T> {
//     type Reborrowed = Pin<&'a mut T>;
//
//     fn reborrow(&'a mut self) -> Self::Reborrowed {
//         self.as_mut()
//     }
// }
// unsafe impl<'a, T: 'a> Reborrow<'a> for Pin<&T> {
//     type Reborrowed = Pin<&'a T>;
//
//     fn reborrow(&'a mut self) -> Self::Reborrowed {
//         self.as_ref()
//     }
// }
// #[doc(hidden)]
// pub unsafe trait DoReborrow {
//     type Reborrowed;
//     unsafe fn do_reborrow(&self) -> Self::Reborrowed;
// }
// unsafe impl<'a, T> DoReborrow for ManuallyDrop<&'a mut T> {
//     type Reborrowed = &'a mut T;
//
//     unsafe fn do_reborrow(&self) -> Self::Reborrowed {
//         transmute_copy(self)
//     }
// }
// unsafe impl<'a, T: Reborrow<'a>> DoReborrow for &ManuallyDrop<&'a mut T> {
//     type Reborrowed = T::Reborrowed;
//
//     unsafe fn do_reborrow(&self) -> Self::Reborrowed {
//         transmute_copy(*self)
//     }
// }
//-----------------
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

/// Trait to wrap raw pointer to a field with a type that corresponds to a projection being done.
///
/// Marker also must have an inherent empty `pub fn check(&self){}` function which is used to check that `project!`
/// macro works on concrete types, and not on generics, you can look at [`Marker::check`] as an example.
pub trait ProjectableMarker<T: ?Sized> {
    /// Wrapped pointer type
    type Output;
    /// Wraps raw pointer to a field with a type that corresponds to a projection being done
    unsafe fn from_raw(&self, raw: *mut T) -> Self::Output;

    #[doc(hidden)]
    unsafe fn from_raw_option(&self, raw: Option<*mut T>) -> Self::Output {
        self.from_raw(raw.unwrap())
    }
}

/// Implement it if your projection can meaningfully project through a deref operation
pub unsafe trait DerefProjectable {
    type Target: ?Sized;
    type Marker;

    fn deref_raw(&self) -> (*mut Self::Target, Self::Marker);
    #[doc(hidden)]
    fn maybe_deref_raw(&self) -> (Option<*mut Self::Target>, Self::Marker) {
        let (a, b) = self.deref_raw();
        (Some(a), b)
    }
}

// #[doc(hidden)]
/// Marker type for the projections used in this crate.
/// You can use that if you need to reuse existing projections.
// #[derive(Copy,Clone)]
pub struct Marker<T>(PhantomData<T>);
impl<T> Marker<T> {
    pub fn new() -> Self {
        Self(PhantomData)
    }

    pub fn check(&self) {}
}
impl<T> Clone for Marker<T> {
    fn clone(&self) -> Self {
        Marker::new()
    }
}

#[doc(hidden)]
pub trait AmbiguityCheck {
    fn check(&self) -> usize {
        unreachable!()
    }
}
impl<T: ?Sized> AmbiguityCheck for T {}

// #[doc(hidden)]
// pub struct DerefMarkerWrapper<T>(PhantomData<T>);
// impl<T> DerefMarkerWrapper<T> {
//     #[doc(hidden)]
//     pub unsafe fn new(_value: &T) -> Self {
//         Self(PhantomData)
//     }
// }

#[repr(transparent)]
pub struct MaybeDerefProjectable<T>(ManuallyDrop<T>);
impl<T> MaybeDerefProjectable<T> {
    pub fn new(from: T) -> Self {
        Self(ManuallyDrop::new(from))
    }
}

unsafe impl<T: Projectable> DerefProjectable for MaybeDerefProjectable<T> {
    type Target = T::Target;
    type Marker = T::Marker;

    fn deref_raw(&self) -> (*mut Self::Target, Self::Marker) {
        self.0.get_raw()
    }
}
unsafe impl<'a, T, Target, Marker> DerefProjectable for &'a MaybeDerefProjectable<T>
where
    &'a T: Projectable<Target = Target, Marker = Marker>,
{
    type Target = Target;
    type Marker = Marker;

    fn deref_raw(&self) -> (*mut Self::Target, Self::Marker) {
        (&*self.0).get_raw()
    }
}
unsafe impl<'a, 'b, T, Target, Marker> DerefProjectable for &'a &'b MaybeDerefProjectable<T>
where
    &'a &'b T: Projectable<Target = Target, Marker = Marker>,
{
    type Target = Target;
    type Marker = Marker;

    fn deref_raw(&self) -> (*mut Self::Target, Self::Marker) {
        unsafe { &*(self as *const _ as *const &&T) }.get_raw()
    }
}
unsafe impl<'a, 'b, 'c, T, Target, Marker> DerefProjectable for &'a &'b &'c MaybeDerefProjectable<T>
where
    &'a &'b &'c T: Projectable<Target = Target, Marker = Marker>,
{
    type Target = Target;
    type Marker = Marker;

    fn deref_raw(&self) -> (*mut Self::Target, Self::Marker) {
        unsafe { &*(self as *const _ as *const &&&T) }.get_raw()
    }
}

unsafe impl<T: DerefProjectable> DerefProjectable for &&&&MaybeDerefProjectable<T> {
    type Target = T::Target;
    type Marker = T::Marker;

    fn deref_raw(&self) -> (*mut Self::Target, Self::Marker) {
        self.0.deref_raw()
    }
}

unsafe impl<'a, T> DerefProjectable for &&&&&'a MaybeDerefProjectable<T>
where
    &'a T: DerefProjectable,
{
    type Target = <&'a T as DerefProjectable>::Target;
    type Marker = <&'a T as DerefProjectable>::Marker;

    fn deref_raw(&self) -> (*mut Self::Target, Self::Marker) {
        (&*self.0).deref_raw()
    }
}

//--------------
unsafe impl<T> Projectable for Owned<T> {
    type Target = T;
    type Marker = Marker<()>;

    fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
        (self as *const Self as *mut Self as *mut T, Marker::new())
    }
}
impl<T> ProjectableMarker<T> for Marker<()> {
    type Output = T;

    unsafe fn from_raw(&self, raw: *mut T) -> Self::Output {
        ptr::read_unaligned(raw as *const T as *const _)
    }
}

unsafe impl<ToCheck, NotPacked> SupportsPacked
    for &(*mut ToCheck, &Marker<()>, PhantomData<NotPacked>)
{
    type Result = NotPacked;
}

/// Version of `Deref` trait that allows moving out `Self::Target` from `Self` by value.
pub trait DerefOwned: DerefMut {
    /// Drops what's left of `Self` when `Self::Target` was moved out
    ///
    /// Safety requirements: must not be called twice on the same instance,
    /// implementation must not access `Self::Target`
    unsafe fn drop_leftovers(_leftovers: &mut ManuallyDrop<Self>) {}
    // /// Safety requirements: must not be called twice on the same instance
    // unsafe fn move_out_target(md: &mut ManuallyDrop<Self>) -> Self::Target
    // where
    //     Self::Target: Sized,
    // {
    //     /// need to go through `deref_mut` because we need to ensure that we have unique access to `Self::Target`
    //     /// otherwise it would be possible to move out from `Rc`
    //     ptr::read(&*(**md).deref_mut())
    // }

    fn deref_owned(self) -> Self::Target
    where
        Self: Sized,
        Self::Target: Sized,
    {
        let mut x = DropLeftovers::new(self);
        unsafe { ManuallyDrop::new(x.deref_as_owning()).0.read() }
    }
}

impl<T> DerefOwned for Pin<T>
where
    T::Target: Unpin,
    T: DerefOwned,
{
    unsafe fn drop_leftovers(_leftovers: &mut ManuallyDrop<Self>) {
        T::drop_leftovers(unsafe { &mut *(_leftovers as *mut _ as *mut ManuallyDrop<T>) })
    }
}

#[cfg(feature = "std")]
impl<T: ?Sized> DerefOwned for Box<T> {
    unsafe fn drop_leftovers(leftovers: &mut ManuallyDrop<Self>) {
        ManuallyDrop::drop(&mut *(leftovers as *mut _ as *mut ManuallyDrop<Box<ManuallyDrop<T>>>))
    }
}

#[doc(hidden)]
pub struct OwnedDropMarker<T: DerefOwned>(*const ManuallyDrop<T>);
// impl<T: DerefOwned> OwnedDropMarker<T> {
//     pub fn check() {}
// }
impl<'a, T: DerefOwned> Drop for OwnedDropMarker<T> {
    fn drop(&mut self) {
        let mut temp = unsafe { ptr::read(self.0) };
        unsafe { T::drop_leftovers(&mut temp) }
    }
}

unsafe impl<T: DerefOwned> DerefProjectable for Owned<T> {
    type Target = T::Target;
    type Marker = OwnedDropMarker<T>;

    fn deref_raw(&self) -> (*mut Self::Target, Self::Marker) {
        let ptr = unsafe { &**self.0 } as *const _ as _;
        (ptr, OwnedDropMarker(&self.0))
    }
}

impl<X: DerefOwned, T> ProjectableMarker<T> for OwnedDropMarker<X> {
    type Output = T;

    unsafe fn from_raw(&self, raw: *mut T) -> Self::Output {
        ptr::read(raw as *const T)
    }
}

//---------------------
unsafe impl<'a, T> Projectable for Helper<&'a mut T> {
    type Target = T;
    type Marker = Marker<&'a mut ()>;

    fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
        (
            unsafe { ptr::read(self as *const _ as *const *mut Self::Target) },
            Marker::new(),
        )
    }
}
impl<'a, T: 'a> ProjectableMarker<T> for Marker<&'a mut ()> {
    type Output = &'a mut T;

    unsafe fn from_raw(&self, raw: *mut T) -> Self::Output {
        &mut *raw
    }
}
unsafe impl<'a, T: DerefMut> DerefProjectable for &Helper<&'a mut T> {
    type Target = T::Target;
    type Marker = Marker<&'a mut ()>;

    fn deref_raw(&self) -> (*mut Self::Target, Self::Marker) {
        (
            unsafe { transmute_copy::<_, &mut T>(*self) }.deref_mut(),
            Marker::new(),
        )
    }
}
unsafe impl<'a, T: DerefMut> DerefProjectable for Helper<&'a mut Pin<T>> {
    type Target = T::Target;
    type Marker = PinMarker<Marker<&'a mut ()>>;

    fn deref_raw(&self) -> (*mut Self::Target, Self::Marker) {
        (
            unsafe {
                transmute_copy::<_, Self>(self)
                    .0
                    .as_mut()
                    .get_unchecked_mut()
            },
            PinMarker(Marker::new()),
        )
    }
}

// impl<'a, T: DerefMut + 'a> ProjectableMarker<Pin<T>> for &DerefMarkerWrapper<Marker<&'a mut ()>> {
//     type Output = Pin<&'a mut T::Target>;
//
//     unsafe fn from_raw(&self, raw: *mut Pin<T>) -> Self::Output {
//         (&mut *raw).as_mut() // todo idea, deref should happen in finalize
//     }
// }

//---------------------
unsafe impl<'a, T> Projectable for Helper<&'a T> {
    type Target = T;
    type Marker = Marker<&'a ()>;

    fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
        (self.0 as *const _ as _, Marker::new())
    }
}
impl<'a, T: 'a> ProjectableMarker<T> for Marker<&'a ()> {
    type Output = &'a T;

    unsafe fn from_raw(&self, raw: *mut T) -> Self::Output {
        &*raw
    }
}
unsafe impl<'a, T: Deref> DerefProjectable for Helper<&'a T> {
    type Target = T::Target;
    type Marker = Marker<&'a ()>;

    fn deref_raw(&self) -> (*mut Self::Target, Self::Marker) {
        (self.0.deref() as *const T::Target as _, Marker::new())
    }
}
unsafe impl<'a, T: Deref> DerefProjectable for &Helper<&'a Pin<T>> {
    type Target = T::Target;
    type Marker = PinMarker<Marker<&'a ()>>;

    fn deref_raw(&self) -> (*mut Self::Target, Self::Marker) {
        (
            self.0.as_ref().get_ref() as *const _ as _,
            PinMarker(Marker::new()),
        )
    }
}
// impl<'a, T: Deref + 'a> ProjectableMarker<T> for DerefMarkerWrapper<Marker<&'a ()>> {
//     type Output = &'a T::Target;
//
//     unsafe fn from_raw(&self, raw: *mut T) -> Self::Output {
//         &**raw
//     }
// }
//---------------------
unsafe impl<'a, T> Projectable for &Helper<&'a Cell<T>> {
    type Target = T;
    type Marker = Marker<&'a Cell<()>>;

    fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
        (unsafe { transmute_copy(*self) }, Marker::new())
    }
}
impl<'a, T: 'a> ProjectableMarker<T> for Marker<&'a Cell<()>> {
    type Output = &'a Cell<T>;

    unsafe fn from_raw(&self, raw: *mut T) -> Self::Output {
        &*(raw as *mut Cell<T>)
    }
}

unsafe impl<'a, T> Projectable for &Helper<&'a mut Cell<T>> {
    type Target = T;
    type Marker = Marker<&'a mut ()>;

    fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
        (unsafe { transmute_copy(*self) }, Marker::new())
    }
}
//---------------------
unsafe impl<'a, T> Projectable for &Helper<&'a mut MaybeUninit<T>> {
    type Target = T;
    type Marker = Marker<&'a mut MaybeUninit<()>>;

    fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
        (
            unsafe { ptr::read(*self as *const _ as *const *mut Self::Target) },
            Marker::new(),
        )
    }
}
impl<'a, T: 'a> ProjectableMarker<T> for Marker<&'a mut MaybeUninit<()>> {
    type Output = &'a mut MaybeUninit<T>;

    unsafe fn from_raw(&self, raw: *mut T) -> Self::Output {
        &mut *(raw as *mut MaybeUninit<T>)
    }
}
//---------------------
unsafe impl<T> CustomWrapper for *mut T {
    type Output = Self;
}
unsafe impl<T> Projectable for *mut T {
    type Target = T;
    type Marker = Marker<*mut ()>;

    fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
        (*self, Marker::new())
    }
}
impl<T> ProjectableMarker<T> for Marker<*mut ()> {
    type Output = *mut T;

    unsafe fn from_raw(&self, raw: *mut T) -> Self::Output {
        raw
    }
}

//---------------------
unsafe impl<T> CustomWrapper for *const T {
    type Output = Self;
}
unsafe impl<T> Projectable for *const T {
    type Target = T;
    type Marker = Marker<*const ()>;

    fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
        (*self as _, Marker::new())
    }
}
impl<T> ProjectableMarker<T> for Marker<*const ()> {
    type Output = *const T;

    unsafe fn from_raw(&self, raw: *mut T) -> Self::Output {
        raw as _
    }
}

//---------------------
unsafe impl<T> CustomWrapper for NonNull<T> {
    type Output = Self;
}
unsafe impl<T> Projectable for NonNull<T> {
    type Target = T;
    type Marker = Marker<NonNull<()>>;

    fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
        (self.as_ptr(), Marker::new())
    }
}
impl<T> ProjectableMarker<T> for Marker<NonNull<()>> {
    type Output = NonNull<T>;

    unsafe fn from_raw(&self, raw: *mut T) -> Self::Output {
        NonNull::new_unchecked(raw)
    }
}

//---------------------
/// Implement that if you need to do some kind of post processing like unwrap something
/// or panic if some soundness requirements are not satisfied
pub trait FinalizeProjection {
    type Output;
    unsafe fn finalize(&self) -> Self::Output;
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
//----------------

#[doc(hidden)]
pub trait CheckNoDeref {
    type Result;
    fn check_deref(&self) -> Self::Result;
}

impl<T> CheckNoDeref for *mut T {
    type Result = *mut T;
    fn check_deref(&self) -> Self::Result {
        *self
    }
}

impl<T: Deref> CheckNoDeref for &*mut T {
    type Result = *mut Infallible;

    fn check_deref(&self) -> *mut Infallible {
        panic!("can't go through deref here, use more explicit syntax")
    }
}

//----------------

/// Implement this only if your projection can work with `#[repr(packed)]` structs.
pub unsafe trait SupportsPacked {
    type Result;
    fn select(&self) -> *mut Self::Result {
        NonNull::dangling().as_ptr()
    }
}

// todo figure out how to make that implementable by downstream crates
unsafe impl<ToCheck, NotPacked, M> SupportsPacked for (*mut ToCheck, M, PhantomData<NotPacked>) {
    type Result = ToCheck;
}
unsafe impl<ToCheck, NotPacked> SupportsPacked
    for &(*mut ToCheck, &Marker<*mut ()>, PhantomData<NotPacked>)
{
    type Result = NotPacked;
}
unsafe impl<ToCheck, NotPacked> SupportsPacked
    for &(*mut ToCheck, &Marker<*const ()>, PhantomData<NotPacked>)
{
    type Result = NotPacked;
}
unsafe impl<ToCheck, NotPacked> SupportsPacked
    for &(*mut ToCheck, &Marker<NonNull<()>>, PhantomData<NotPacked>)
{
    type Result = NotPacked;
}

/// Macro to do all kinds of projections
///
/// Has two modes:
///  - `let` syntax very similar to regular rust's `let <pattern> = <expr>`.
///    Basically it is exactly the same but also has additional support for deref patterns and does not yet supports bindings via `@`.
///  - single field projection `project!(<variable> -> <field>)` or `project!((<expression>) -> <field>)`.
///     Basically same as doing `let` option with one field, but this one is an expression while `let` one is a statement.
///     Also this variant additionally tries to do an implicit deref projection if possible.
///     Note though that you will get an error if inner type of projection implements `Deref`.
///     This is caused by the fact that Rust's `.` operator(which is used by this macro) can go through an implicit deref call
///     which would ruin all unsafe logic.
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
///     let mut cell_ref = &cell;
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
///     project!(let Foo { x: x, y: ref mut y } = foo);
///     let x: usize = x;
///     let y: &mut usize = y;
///
///     let mut foo = Foo { x: 0, y: 0 };
///     project!(let Foo { x, y } = &mut foo);
///     *x = 1;
///     *y = 1;
///
///     let mut mu = MaybeUninit::<Foo>::uninit();
///     project!(let Foo { x:x, y:y } = &mut mu);
///     let x: &mut MaybeUninit<usize> = x;
///     let y: &mut MaybeUninit<usize> = y;
///
///     struct Bar(usize, usize);
///     let mut foo = Bar(1, 2);
///     project! { let Bar(x,y) = &mut foo }
///     project! { let Bar{ 0: x, 1: y } = &mut foo }
///     let mut foo_mut = &mut foo;
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
///     project!( let Foo{ x: * x, y: * ref mut y }  = foo );
///     let x: usize = x;
///     let y: &mut usize = y;
/// ```
/// Also `Pin` projection:
/// ```rust
/// #    use std::marker::PhantomPinned;
/// #    use std::pin::Pin;
/// #    use projecture::{project, Unpinned,PinProjectable};
///     #[macro_rules_attribute::derive(PinProjectable!)]
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
/// `Option` projection, which also works together with other projections
/// ```rust
/// # use std::marker::PhantomPinned;
/// # use std::pin::Pin;
/// # use projecture::{project,PinProjectable};
/// #[macro_rules_attribute::derive(Default,PinProjectable!)]
/// struct Foo{
///     x: Box<usize>,
///     y: usize,
///     p: PhantomPinned
/// }
/// let mut arg = Some(Foo::default());
/// project!(let Foo { x: *x, y } = arg.as_mut());
/// let x: Option<&mut usize> = x;
/// let y: Option<&mut usize> = y;
///
/// project!(let Foo { x, y } = arg);
/// let x: Option<Box<usize>> = x;
/// let y: Option<usize> = y;
///
/// # // todo
/// # // fn test_pin(arg: Option<Pin<&mut Foo>>){
/// # //     project!(let Foo { p, ..} = arg );
/// # //     let p: Option<Pin<&mut PhantomPinned>> = p;
/// # // }
/// ```
/// `Ref`/`RefMut` projection
/// ```rust
/// # use std::cell::{Ref, RefCell, RefMut};
/// # use projecture::project;
/// #[derive(Default)]
/// struct Foo(String,String);
/// let arg = RefCell::new(Foo::default());
/// project!(let Foo(x,_) = arg.borrow());
/// let x: Ref<String> = x;
/// # drop(x);
///
/// project!(let Foo(x, ..) = arg.borrow_mut());
/// let x: RefMut<String> = x;
/// ```
/// Raw pointer projection (`*const T`, `*mut T`, `NonNull<T>`).
/// Note that it is safe because it behaves like a [`pointer::wrapping_offset`](https://doc.rust-lang.org/std/primitive.pointer.html#method.wrapping_offset).
/// If you want unsafe [`pointer::offset`](https://doc.rust-lang.org/std/primitive.pointer.html#method.offset) like behavior you can still enable that by doing `.add(0)` on a resulting pointer
/// ```rust
/// use std::ptr::NonNull;
/// use projecture::project;
/// #[repr(C,packed)]
/// struct Packed(u8,usize);
/// let mut x = Packed(1,2);
/// let ptr = &x as *const Packed;
/// let val = unsafe { project!(ptr -> 1).read_unaligned() };
/// assert_eq!(val,2);
///
/// let ptr:Option<NonNull<Packed>> = NonNull::new(&mut x);
/// let field_ptr: Option<NonNull<usize>> = project!(ptr -> 1);
/// let val = field_ptr.map(|ptr|unsafe { ptr.as_ptr().read_unaligned() } );
/// assert_eq!(val,Some(2));
/// ```
#[macro_export]
macro_rules! project {
    // ( { $($field:ident),+  } = $target:expr) => {};
    (let $struct:ident { $($fields:tt)+ } = $val: expr) => {
        let var = core::mem::ManuallyDrop::new($val);
        let var = {
            use $crate::Preprocess;
            core::mem::ManuallyDrop::new((&&&&&var).preprocess())
        };

        let (ptr,marker) = {
            use $crate::Projectable;
            (&&&&&&& *var).get_raw()
        };
        // if false{
        //     use $crate::AmbiguityCheck;
        //     let _:() = marker.check();
        //     // let $struct { .. } = unsafe { &*ptr };
        // }
        $crate::project_struct_fields! { [ptr marker $struct] [] $($fields)+ }
        drop(marker);
    };
    (let $struct:ident ( $($fields:tt)+ ) = $val: expr) => {
        let var = core::mem::ManuallyDrop::new($val);
        let var = {
            use $crate::Preprocess;
            core::mem::ManuallyDrop::new((&&&&&var).preprocess())
        };
        let (ptr,marker) = {
            use $crate::Projectable;
            (&&&&&&& *var).get_raw()
        };
        // if false{
        //     use $crate::AmbiguityCheck;
        //     let _:() = marker.check();
        //     // let $struct { .. } = unsafe{ &*ptr };
        // }

        $crate::project_tuple_fields! { [ptr marker $struct] [] [] $($fields)+ }
        drop(marker);
    };
    (let * $($tail:tt)+) => {
        $crate::project_deref!{ [] $($tail)+ }
    };
    // why the f `let _ = x;` does not drop `x` ?!!
    // and at the same time `let _ = Foo;` does drop `Foo` ... , like wtf?!!
    (let _ = $val:expr) => {
        drop($val);
    };
    (let $pat:pat = $val:expr) => {
        let $pat = $val;
    };
    ($var:ident ) => { $var };
    ( $var:ident -> $($tail:tt)+) => { $crate::project! { ($var) -> $($tail)+ } };
    (( $var:expr ) -> $method:tt ($($args:tt)*) $($tail:tt)*) => {
          //todo
    };
    (( $var:expr ) -> $field:tt $($tail:tt)*) => {
        {
            $crate::project_deref!( ? [ var ] = $var);
            // use $crate::DoReborrow;
            let var = core::mem::ManuallyDrop::new(var);
            // let var = unsafe { core::mem::ManuallyDrop::new((&&var).do_reborrow()) };
            let var = {
                use $crate::Preprocess;
                core::mem::ManuallyDrop::new((&&&&&var).preprocess())
            };

            let (ptr,marker) = {
                use $crate::Projectable;
                (&&&&&&& *var).get_raw()
            };
            let ptr = {
                // use $crate::AmbiguityCheck;
                // let _:() = marker.check();
                use $crate::CheckNoDeref;
                // check that (*ptr).field would not go through a deref
                (&&ptr).check_deref()
            };

            $crate::project_field_inner! { [ptr marker] { $field } : temp_name }
            $crate::project!( temp_name $($tail)*)
        }
    };

}

#[doc(hidden)]
#[macro_export]
macro_rules! project_deref {
    ( ? [$($parsed:tt)*] = $($tail:tt)* ) => {
        let var = core::mem::ManuallyDrop::new($($tail)*);
        let var = {
            use $crate::Preprocess;
            (&&&&&var).preprocess()
        };
        let (ptr,marker) = {
            use $crate::DerefProjectable;
            (&&&&&& $crate::MaybeDerefProjectable::new(var)).deref_raw()
        };
        #[allow(unused_mut)]
        let mut result = unsafe {
            use $crate::{ProjectableMarker,Finalizer};
            let tmp = core::mem::ManuallyDrop::new(marker.from_raw(ptr));
            (&&&&& tmp).call_finalize()
        };
        drop(marker);
        $crate::project!(let $($parsed)* = result);
    };

    ( [$($parsed:tt)*] = $($tail:tt)* ) => {
        let var = core::mem::ManuallyDrop::new($($tail)*);
        let var = {
            use $crate::Preprocess;
            core::mem::ManuallyDrop::new((&&&&&var).preprocess())
        };
        let (ptr,marker) = {
            use $crate::DerefProjectable;
            (&&&&&&& *var).deref_raw()
        };
        #[allow(unused_mut)]
        let mut result = unsafe {
            use $crate::{ProjectableMarker,Finalizer};
            let tmp = core::mem::ManuallyDrop::new(marker.from_raw(ptr));
            (&&&&& tmp).call_finalize()
        };
        drop(marker);
        $crate::project!(let $($parsed)* = result);
    };
    ([$($parsed:tt)*] $token:tt $($tail:tt)*) => {
        $crate::project_deref!{ [$($parsed)* $token]  $($tail)* }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! project_tuple_fields {
    ([$ptr:ident $marker:ident $struct:ident] [$($idx:tt)*] [$($pattern:tt)*] , $($tail:tt)* ) => {
        $crate::project_field_inner! { [$ptr $marker $struct] { $($idx)* } : $($pattern)* }
        $crate::project_tuple_fields! { [$ptr $marker $struct] [$($idx)* !] [] $($tail)* }
    };
    ([$ptr:ident $marker:ident $struct:ident] [$($idx:tt)*] [] ) => {};
    ([$ptr:ident $marker:ident $struct:ident] [$($idx:tt)*] [] .. ) => {};

    ([$ptr:ident $marker:ident $struct:ident] [$($idx:tt)*] [$($pattern:tt)*]  $next:tt $($tail:tt)* ) => {
        $crate::project_tuple_fields! { [$ptr $marker $struct] [$($idx)*] [$($pattern)* $next] $($tail)*  }
    };
    ([$ptr:ident $marker:ident $struct:ident] [$($idx:tt)*] [$($pattern:tt)*] ) => {
        $crate::project_field_inner! { [$ptr $marker $struct] { $($idx)* } : $($pattern)* }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! project_struct_fields {
    ([$ptr:ident $marker:ident $struct:ident] [$name:tt $($pattern:tt)*] , $($tail:tt)* ) => {
        $crate::project_field_inner! { [$ptr $marker $struct] { $name } $($pattern)* }
        $crate::project_struct_fields! { [$ptr $marker $struct]  [] $($tail)* }
    };
    ([$ptr:ident $marker:ident $struct:ident] [] ) => {};
    ([$ptr:ident $marker:ident $struct:ident] [] ..) => {};
    ([$ptr:ident $marker:ident $struct:ident] [$($pattern:tt)*] $next:tt $($tail:tt)* ) => {
        $crate::project_struct_fields! { [$ptr $marker $struct] [$($pattern)* $next] $($tail)*  }
    };
    ([$ptr:ident $marker:ident $struct:ident] [$name:tt $($pattern:tt)*] ) => {
        $crate::project_field_inner! { [$ptr $marker $struct] { $name } $($pattern)* }
    };
}

/// ```rust,compile_fail
/// use projecture::project;
/// #[repr(packed)]
/// struct Test{
///     x:u8,
///     y: usize,
/// }
/// fn test(arg:&Test){
///     project!(let Test { x, y } = arg);
/// }
/// ```
///
/// ```rust,compile_fail
///     use projecture::project;
///     use core::cell::Cell;
///     struct Foo(usize, usize);
///     let tmp = Cell::new(Box::new(Foo(1, 2)));
///     let x = project!((&tmp) -> 1);
/// ```
///
/// ```rust,compile_fail
/// use std::marker::PhantomPinned;
/// use std::pin::Pin;
/// use projecture::project;
/// struct Foo(usize,PhantomPinned);
/// impl Drop for Foo{
///     fn drop(&mut self) {}
/// }
/// fn test(arg: Pin<&mut Foo>){
///     let _ = project!(arg -> usize);
/// }
/// ```
#[doc(hidden)]
#[macro_export]
macro_rules! not_packed {
    ($name:ident $field:ident) => {
        struct $name {
            $field: (),
        }
    };
    ($name:ident $($field:tt)* ) => {
        struct $name((), (), (), (), (), (), (), (), (), (), ());
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! project_field_inner {
    ( [$($args:tt)*] { } : $($pattern:tt)* ) => {
        $crate::project_field_inner! { [$($args)*] { 0 } : $($pattern)* }
    };
    ( [$($args:tt)*] { ! } : $($pattern:tt)* ) => {
        $crate::project_field_inner! { [$($args)*] { 1 } : $($pattern)* }
    };
    ( [$($args:tt)*] { !! } : $($pattern:tt)* ) => {
        $crate::project_field_inner! { [$($args)*] { 2 } : $($pattern)* }
    };
    ( [$($args:tt)*] { !!! } : $($pattern:tt)* ) => {
        $crate::project_field_inner! { [$($args)*] { 3 } : $($pattern)* }
    };
    ( [$($args:tt)*] { !!!! } : $($pattern:tt)* ) => {
        $crate::project_field_inner! { [$($args)*] { 4 } : $($pattern)* }
    };
    ( [$($args:tt)*] { !!!!! } : $($pattern:tt)* ) => {
        $crate::project_field_inner! { [$($args)*] { 5 } : $($pattern)* }
    };
    ( [$($args:tt)*] { !!!!!! } : $($pattern:tt)* ) => {
        $crate::project_field_inner! { [$($args)*] { 6 } : $($pattern)* }
    };
    ( [$($args:tt)*] { !!!!!!! } : $($pattern:tt)* ) => {
        $crate::project_field_inner! { [$($args)*] { 7 } : $($pattern)* }
    };
    ( [$($args:tt)*] { !!!!!!!! } : $($pattern:tt)* ) => {
        $crate::project_field_inner! { [$($args)*] { 8 } : $($pattern)* }
    };
    ( [$($args:tt)*] { !!!!!!!!! } : $($pattern:tt)* ) => {
        $crate::project_field_inner! { [$($args)*] { 9 } : $($pattern)* }
    };
    ( [$($args:tt)*] { !!!!!!!!!! } : $($pattern:tt)* ) => {
        $crate::project_field_inner! { [$($args)*] { 10 } : $($pattern)* }
    };
    ( [$ptr:tt $marker:ident $type:ident] { $field:tt } $($pattern:tt)* ) => {
        if false {
            let $type { $field : _ , .. } = unsafe { &*$ptr };
        }
        $crate::project_field_inner! { [$ptr $marker] { $field } $($pattern)* }
    };
    ( [$ptr:tt $marker:ident] { $field:tt } : $($pattern:tt)* ) => {
        #[allow(unused_mut)]
        let mut tmp = unsafe {
            use $crate::{ProjectableMarker,Finalizer,SupportsPacked};
            // check for #[packed] struct
            #[forbid(unaligned_references)]
            #[allow(dead_code)]
            if false{
                $crate::not_packed! { Foo $field }
                let check_ptr = ( &&($ptr, &$marker, core::marker::PhantomData::<Foo>) ).select();
                let _ = &(*check_ptr). $field;
            }
            fn create_uninit<T>(_ptr: *mut T) -> core::mem::MaybeUninit<T>{ core::mem::MaybeUninit::uninit() }
            let mut mu = create_uninit($ptr);
            let mu_ptr = mu.as_mut_ptr();
            let mu_field_ptr = core::ptr::addr_of_mut!((*mu_ptr). $field );
            let offset = (mu_field_ptr as *mut u8).offset_from(mu_ptr as *mut u8);
            fn do_offset<T,U>(ptr:*mut T, _field_ptr_type: *mut U, offset: isize) -> *mut U{
                (ptr as *mut u8).wrapping_offset(offset) as *mut U
            }
            let field_ptr = do_offset($ptr, mu_field_ptr, offset);
            let tmp = core::mem::ManuallyDrop::new($marker.from_raw(field_ptr));
            (&&&&& tmp).call_finalize()
        };
        $crate::project!(let $($pattern)* = tmp);

    };
    ( [$ptr:tt $marker:ident] { $field:ident } ) => { $crate::project_field_inner! { [$ptr $marker] { $field } : $field } };
}

/// Keeps track of stuff that was left of `T` when we moved out `T::Target` from it.
pub struct DropLeftovers<'a, T: DerefOwned>(
    // workaround to not require #[may_dangle] on 'a
    DropLeftoversInner<T>,
    PhantomData<fn(&'a ()) -> &'a ()>,
);
struct DropLeftoversInner<T: DerefOwned>(ManuallyDrop<T>);
impl<T: DerefOwned> Drop for DropLeftoversInner<T> {
    fn drop(&mut self) {
        unsafe { T::drop_leftovers(&mut self.0) }
    }
}

impl<'a, T: DerefOwned> DropLeftovers<'a, T> {
    pub fn new(val: T) -> Self {
        Self(DropLeftoversInner(ManuallyDrop::new(val)), PhantomData)
    }

    pub fn deref_as_owning(&'a mut self) -> OwningRef<'a, T::Target> {
        OwningRef((*self.0 .0).deref_mut(), PhantomData)
    }
}

#[cfg_attr(
    feature = "nightly",
    doc = "
```rust
#![feature(arbitrary_self_types)]
# use std::marker::PhantomData;
# use std::mem::ManuallyDrop;
# use projecture::{DerefOwned, DropLeftovers, OwningRef};
trait Trait{
    fn test(self: OwningRef<'_,Self>);
}

impl Trait for String{
    fn test(self: OwningRef<'_, Self>) {
        assert_eq!(\"test\", self.deref_owned());
    }
}

let x = Box::new(String::from(\"test\")) as Box<dyn Trait>;
DropLeftovers::new(x).deref_as_owning().test()
```
"
)]
pub struct OwningRef<'a, T: 'a + ?Sized>(*mut T, PhantomData<fn(&'a ()) -> &'a ()>);

impl<T: ?Sized> Drop for OwningRef<'_, T> {
    fn drop(&mut self) {
        unsafe { drop_in_place(self.0) }
    }
}

impl<'a, T: ?Sized + 'a> Deref for OwningRef<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0 }
    }
}
impl<'a, T: ?Sized + 'a> DerefMut for OwningRef<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.0 }
    }
}
impl<'a, T: 'a> DerefOwned for OwningRef<'a, T> {}

#[cfg(feature = "nightly")]
impl<'a, T: ?Sized, U: ?Sized> core::ops::DispatchFromDyn<OwningRef<'a, U>> for OwningRef<'a, T> where
    T: core::marker::Unsize<U>
{
}

pub struct OwningMarker<'a, T>(&'a mut ManuallyDrop<T>);
impl<'a, T> ProjectableMarker<T> for OwningMarker<'a, T> {
    type Output = T;

    unsafe fn from_raw(&self, raw: *mut T) -> Self::Output {
        ptr::read(raw as *const T)
    }
}

//todo:
// pattern matching/enums
// foldable trait
//

#[cfg(any())]
mod experiments;
