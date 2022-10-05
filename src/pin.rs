use super::*;
use core::pin::Pin;

// unsafe impl<T> MarkerNonOwned for Pin<T> {}
unsafe impl<T> CustomWrapper for Pin<T> {
    type Output = Pin<T>;
}
unsafe impl<'a, T> CustomWrapper for &Pin<&'a T> {
    type Output = Pin<Helper<&'a T>>;
}
unsafe impl<'a, T> CustomWrapper for &Pin<&'a mut T> {
    type Output = Pin<Helper<&'a mut T>>;
}
unsafe impl<T: CustomWrapper> CustomWrapper for &&Pin<T> {
    type Output = Pin<T::Output>;
}

macro_rules! impl_pin {
    ($($maybe_mut:tt)?) => {
        unsafe impl<'a, T:PinProjectable> Projectable for Pin<&'a $($maybe_mut)? T> {
           type Target = T;
           type Marker = Marker<Pin<&'a $($maybe_mut)? ()>>;

           fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
               (
                   unsafe { transmute_copy(self) },
                   Marker::new()
               )
           }
        }
        // #[allow(drop_bounds)]
        // unsafe impl<'a, T: Drop> Projectable for &Pin<&'a $($maybe_mut)? T> {
        //     type Target = T;
        //     type Marker = core::convert::Infallible;
        //
        //     fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
        //         panic!("struct must also implement PinDrop")
        //     }
        // }
        // #[allow(drop_bounds)]
        // unsafe impl<'a, T: Drop + PinDrop> Projectable for &&Pin<&'a $($maybe_mut)? T> {
        //     type Target = T;
        //     type Marker = Marker<Pin<&'a $($maybe_mut)? ()>>;
        //
        //     fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
        //         (
        //             unsafe { transmute_copy(**self) },
        //             Marker::new()
        //         )
        //     }
        // }
        // #[cfg(feature = "pin-project")]
        // unsafe impl<'a, T: pin_project::__private::PinnedDrop> Projectable for &&&Pin<&'a $($maybe_mut)? T> {
        //     type Target = T;
        //     type Marker = Marker<Pin<&'a $($maybe_mut)? ()>>;
        //
        //     fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
        //         panic!("this crate is incompatible with pin-project when both are used on the same struct")
        //     }
        // }
        // // maybe it should also be a panic
        // unsafe impl<'a, T: Unpin> Projectable for &&&&Pin<&'a $($maybe_mut)? T> {
        //    type Target = T;
        //    type Marker = Marker<&'a $($maybe_mut)? ()>;
        //
        //    fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
        //        (
        //            unsafe { transmute_copy(****self) },
        //            Marker::new()
        //        )
        //    }
        // }


        impl<'a, T: 'a> ProjectableMarker<T> for Marker<Pin<&'a $($maybe_mut)? ()>> {
            type Output = Pin<&'a $($maybe_mut)? T>;

            unsafe fn from_raw(&self, raw: *mut T) -> Self::Output {
                Pin::new_unchecked(&$($maybe_mut)? *raw)
            }
        }

        impl<'a, T: Unpin> FinalizeProjection for Pin<&'a $($maybe_mut)? T> {
            type Output = &'a $($maybe_mut)? T;

            unsafe fn finalize(&self) -> Self::Output {
                transmute_copy(self)
            }
        }
        impl<'a, T> FinalizeProjection for &Pin<&'a $($maybe_mut)? Unpinned<T>> {
            type Output = &'a $($maybe_mut)? T;

            unsafe fn finalize(&self) -> Self::Output {
                transmute_copy(*self)
            }
        }

    };
}

// impl_pin! {}
// impl_pin! { mut }
unsafe impl<'a, P: Deref<Target = T> + Projectable<Target = T>, T: PinProjectable> Projectable
    for Pin<P>
{
    type Target = T;
    type Marker = PinMarker<P::Marker>;

    fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
        let res = unsafe { transmute_copy::<_, P>(self) }.get_raw();
        (res.0, PinMarker(res.1))
    }
}
pub struct PinMarker<T>(pub T);

impl<'a, T: 'a, P: ProjectableMarker<T>> ProjectableMarker<T> for PinMarker<P>
where
    P::Output: Deref,
{
    type Output = Pin<P::Output>;

    unsafe fn from_raw(&self, raw: *mut T) -> Self::Output {
        Pin::new_unchecked(self.0.from_raw(raw))
    }
}

impl<'a, P> FinalizeProjection for Pin<P>
where
    P: Deref,
    P::Target: Unpin,
{
    type Output = P;

    unsafe fn finalize(&self) -> Self::Output {
        transmute_copy(self)
    }
}
impl<'a, T> FinalizeProjection for &Pin<&'a Unpinned<T>> {
    type Output = &'a T;

    unsafe fn finalize(&self) -> Self::Output {
        transmute_copy(*self)
    }
}
impl<'a, T> FinalizeProjection for &Pin<&'a mut Unpinned<T>> {
    type Output = &'a mut T;

    unsafe fn finalize(&self) -> Self::Output {
        transmute_copy(*self)
    }
}
impl<'a, T> FinalizeProjection for &Pin<OwningRef<'a, Unpinned<T>>> {
    type Output = OwningRef<'a, T>;

    unsafe fn finalize(&self) -> Self::Output {
        transmute_copy(*self)
    }
}

//---------------------
/// Transparent wrapper to indicate that a type should not be pin projected.
/// It will be removed after projection.
#[repr(transparent)]
pub struct Unpinned<T>(pub T);
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
impl<T> DerefOwned for Unpinned<T> {}

// pub struct Pinned<T>(T);

#[doc(hidden)]
pub struct PinnedMarker<'a>(&'a (), PhantomPinned);

/// Unfortunately `Unpin` is a safe trait ( whyyyy .... (╯︵╰,)  pin projection would have "just worked" would it be unsafe),
/// so we need some way to indicate that the type does not have incorrect `Unpin` implementation.
/// Implemented by [`pin_projectable`] macro.
pub unsafe trait PinProjectable {}

/// For Pin projection to work soundly if struct wants to implement custom Drop it needs to
/// always go through `Pin<&mut Self>`. So `Drop` implementation must directly delegate to `PinDrop`.
/// Similar to what `pin_project::pinned_drop` is doing but without proc macros.
/// You should use [`pin_projectable`] macro to implement such delegating drop without `unsafe`.
/// `PinDrop` implementation just like `Drop` one should have *exactly* same bounds as the struct itself, otherwise
/// delegation will not work.
pub trait PinDrop {
    /// Implementation of drop for pinned struct.
    fn drop(_self: CallGuard<Pin<&mut Self>>);
}

/// Trivial wrapper to make functions(like [`PinDrop::drop`]) safe to implement but unsafe to call.
pub struct CallGuard<T>(T);
impl<T> CallGuard<T> {
    /// Safety requirements: see function that requires that
    pub unsafe fn new(from: T) -> Self {
        CallGuard(from)
    }
    pub fn into_inner(self) -> T {
        self.0
    }
}

/// Macro to delegate to `PinDrop`.
///
/// ```rust
/// # use std::fmt::Debug;
/// # use std::marker::PhantomPinned;
/// # use projecture::{pin_projectable, PinDrop, CallGuard};
/// # use projecture::project;
/// # use core::pin::Pin;
/// trait Trait<T> {}
/// struct Foo<'a,T: Trait<usize>>(&'a T, PhantomPinned) where T:Debug;
///
/// pin_projectable!{ Foo<'a, T: Trait<usize>> where T: Debug }
///
/// impl<'a, T: Trait<usize>> PinDrop for Foo<'a, T> where T: Debug{
///     fn drop(_self: CallGuard<Pin<&mut Self>>){
///         project!(let Foo(x,_) = _self.into_inner());
///         println!("{:?}",x);
///     }
/// }
/// ```
#[macro_export]
macro_rules! pin_projectable {
    ([ ! $($generics:tt)*] [$($type:tt)*] [] < $($tail:tt)* ) => { $crate::pin_projectable!{[ !! $($generics)* <] [ $($type)*] [] $($tail)*} };
    ([ ! $($generics:tt)*] [$($type:tt)*] [] << $($tail:tt)* ) => { $crate::pin_projectable!{[ !! $($generics)* <] [ $($type)*] [] < $($tail)*} };
    ([ !! $($generics:tt)*] [$($type:tt)*] [] > $($tail:tt)* ) => { $crate::pin_projectable!{[ ! $($generics)* >] [ $($type)*] [] $($tail)*} };
    ([ !! $($generics:tt)*] [$($type:tt)*] [] >> $($tail:tt)* ) => { $crate::pin_projectable!{[ ! $($generics)* >] [ $($type)*] [] > $($tail)*} };
    ([ !! $($generics:tt)*] [$($type:tt)*] [] $token:tt $($tail:tt)* ) => { $crate::pin_projectable!{[!! $($generics)* $token] [$($type)*] [] $($tail)*} };

    ([ ! $($generics:tt)*] [$($type:tt)*] [] , $generic:tt $($tail:tt)* ) => { $crate::pin_projectable!{[ ! $($generics)* , $generic] [ $($type)* , $generic] [] $($tail)*} };
    ([ ! $($generics:tt)*] [$($type:tt)*] [] > ) => { $crate::pin_projectable!{ [ $($generics)*] [ $($type)* > ] [] } };
    ([ ! $($generics:tt)*] [$($type:tt)*] [] > where $($tail:tt)* ) => { $crate::pin_projectable!{[ $($generics)*] [ $($type)* >] [$($tail)*]  } };
    ([ ! $($generics:tt)*] [$($type:tt)*] [] $token:tt $($tail:tt)* ) => { $crate::pin_projectable!{[! $($generics)* $token] [$($type)*] [] $($tail)*} };

    ([$($generics:tt)*] [! $($type:tt)*] [] < $generic:tt $($tail:tt)* ) => { $crate::pin_projectable!{ [ ! $($generics)* $generic] [$($type)* < $generic] [] $($tail)*} };
    ([$($generics:tt)*] [! $($type:tt)*] [] $name_part:tt $($tail:tt)* ) => { $crate::pin_projectable!{ [ $($generics)* ] [! $($type)* $name_part] [] $($tail)*} };
    ([$($generics:tt)*] [! $($type:tt)*] [] ) => { $crate::pin_projectable!{ [ $($generics)* ] [ $($type)* ] [] } };

    ([$($generics:tt)*] [$($type:tt)+] [$($where:tt)*] ) => {
        impl<'__inner,$($generics)*> core::marker::Unpin for $($type)+ where $crate::pin::PinnedMarker<'__inner>:Unpin,$($where)*{}
        unsafe impl<$($generics)*> $crate::pin::PinProjectable for $($type)+ where $($where)*{}

        impl<$($generics)*> core::ops::Drop for $($type)+ where $($where)*{
            fn drop(&mut self){
                unsafe {
                    let mut helper = &mut *(self as *mut _ as *mut $crate::Helper<$($type)+>);
                    use $crate::pin::PinDropDelegator;
                    (&mut &mut helper).delegate()
                }
            }
        }
    };

    ( $($tail:tt)* ) => { $crate::pin_projectable!{ [] [!] [] $($tail)* } };

}

/// Version of [`pin_projectable`] to work as derive macro with [`macro_rules_attribute`](https://docs.rs/macro_rules_attribute)
/// ```rust
/// use projecture::PinProjectable;
/// use macro_rules_attribute::derive;
/// #[derive(PinProjectable!)]
/// struct Foo(Box<usize>);
/// ```
#[macro_export]
#[allow(non_snake_case)]
macro_rules! PinProjectable {
    ( [struct $($head:tt)*] { $($inner:tt)* } ) => {
        $crate::pin_projectable!{ $($head)* }
    };
    ( [enum   $($head:tt)*] { $($inner:tt)* } ) => {
        $crate::pin_projectable!{ $($head)* }
    };
    ( [struct $($head:tt)*] ($($inner:tt)*) ; ) => {
        $crate::pin_projectable!{ $($head)* }
    };
    ( [enum   $($head:tt)*] ($($inner:tt)*) ; ) => {
        $crate::pin_projectable!{ $($head)* }
    };
    ( [$($head:tt)*] $token:tt $($tail:tt)* ) => { $crate::PinProjectable!{ [$($head)* $token] $($tail)* } };
    ( $($tail:tt)* ) => { $crate::PinProjectable!{ [] $($tail)* } };
}

#[doc(hidden)]
pub unsafe trait PinDropDelegator {
    unsafe fn delegate(&mut self);
}

unsafe impl<T> PinDropDelegator for &mut Helper<T> {
    unsafe fn delegate(&mut self) {}
}

unsafe impl<T: PinDrop> PinDropDelegator for Helper<T> {
    unsafe fn delegate(&mut self) {
        PinDrop::drop(CallGuard::new(Pin::new_unchecked(&mut self.0)))
    }
}

// pub type Identity<'hrtb, T> = <T as IdentityIgnoring<'hrtb>>::ItSelf;
// // where
// pub trait IdentityIgnoring<'__> {
//     type ItSelf: ?Sized;
// }
// impl<T: ?Sized> IdentityIgnoring<'_> for T {
//     type ItSelf = Self;
// }
