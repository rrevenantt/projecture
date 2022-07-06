use super::*;
use core::pin::Pin;

// unsafe impl<T> MarkerNonOwned for Pin<T> {}
unsafe impl<T> CustomWrapper for Pin<T> {
    type Output = Pin<T>;
}
macro_rules! impl_pin {
    ($($maybe_mut:tt)?) => {
        unsafe impl<'a, T> Projectable for Pin<&'a $($maybe_mut)? T> {
           type Target = T;
           type Marker = Marker<Pin<&'a $($maybe_mut)? ()>>;

           fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
               (
                   unsafe { transmute_copy(self) },
                   Marker::new()
               )
           }
        }
        #[allow(drop_bounds)]
        unsafe impl<'a, T: Drop> Projectable for &Pin<&'a $($maybe_mut)? T> {
            type Target = T;
            type Marker = Marker<Pin<&'a $($maybe_mut)? ()>>;

            fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
                panic!("struct must also implement PinDrop")
            }
        }
        #[allow(drop_bounds)]
        unsafe impl<'a, T: Drop + PinDrop> Projectable for &&Pin<&'a $($maybe_mut)? T> {
            type Target = T;
            type Marker = Marker<Pin<&'a $($maybe_mut)? ()>>;

            fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
                (
                    unsafe { transmute_copy(**self) },
                    Marker::new()
                )
            }
        }
        #[cfg(feature = "pin-project")]
        unsafe impl<'a, T: pin_project::__private::PinnedDrop> Projectable for &&&Pin<&'a $($maybe_mut)? T> {
            type Target = T;
            type Marker = Marker<Pin<&'a $($maybe_mut)? ()>>;

            fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
                panic!("this crate is incompatible with pin-project when both are used on the same struct")
            }
        }
        // maybe it should also be a panic
        unsafe impl<'a, T: Unpin> Projectable for &&&&Pin<&'a $($maybe_mut)? T> {
           type Target = T;
           type Marker = Marker<&'a $($maybe_mut)? ()>;

           fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
               (
                   unsafe { transmute_copy(****self) },
                   Marker::new()
               )
           }
        }


        impl<'a, T: 'a> ProjectableMarker<T> for Marker<Pin<&'a $($maybe_mut)? ()>> {
            type Output = Pin<&'a $($maybe_mut)? T>;

            unsafe fn from_raw(&self, raw: *mut T) -> Self::Output {
                Pin::new_unchecked(&$($maybe_mut)? *raw)
            }
        }
        impl<'a, T: Unpin + DerefMut + 'a> ProjectableMarker<T> for DerefMarkerWrapper<Marker<Pin<&'a $($maybe_mut)? ()>>> {
            type Output = &'a $($maybe_mut)? T::Target;

            unsafe fn from_raw(&self, raw: *mut T) -> Self::Output {
                &$($maybe_mut)? **raw
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

impl_pin! {}
impl_pin! { mut }

//---------------------
/// Transparent wrapper to indicate that a type should not be pin projected.
/// It will be removed after projection
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

/// For Pin projection to work soundly if struct wants to implement custom Drop it needs to
/// always go through `Pin<&mut Self>`. So `Drop` implementation must directly delegate to `PinDrop`.
/// Similar to what `pin_project::pinned_drop` is doing but without proc macros.
/// You can use [`pin_drop_delegate`] macro to implement such delegating drop without `unsafe`
pub trait PinDrop {
    /// Implementation of drop for pinned struct.
    /// `marker` parameter exists only to prevent calling this function from safe code.
    fn drop(self: Pin<&mut Self>, marker: DropMarker);
}

/// Marker to prevent calling [`PinDrop::drop`] from safe code
pub struct DropMarker(());
impl DropMarker {
    #[doc(hidden)]
    pub unsafe fn new() -> Self {
        DropMarker(())
    }
}

/// ```rust
/// # use std::fmt::Debug;
/// # use std::marker::PhantomPinned;
/// # use projecture::{DropMarker, pin_drop_delegate, PinDrop};
/// # use projecture::project;
/// # use core::pin::Pin;
/// trait Trait<T> {}
/// struct Foo<'a,T: Trait<usize>>(&'a T, PhantomPinned) where T:Debug;
///
/// pin_drop_delegate!{ Foo<'a, T: Trait<usize>> where T: Debug }
///
/// impl<'a, T: Trait<usize>> PinDrop for Foo<'a, T> where T: Debug{
///     fn drop(self: Pin<&mut Self>, _marker: DropMarker){
///         project!(let Foo(x,_) = self);
///         println!("{:?}",x);
///     }
/// }
/// ```
#[macro_export]
macro_rules! pin_drop_delegate {
    ([ ! $($generics:tt)*] [$($type:tt)*] [] < $($tail:tt)* ) => { $crate::pin_drop_delegate!{[ !! $($generics)* <] [ $($type)*] [] $($tail)*} };
    ([ ! $($generics:tt)*] [$($type:tt)*] [] << $($tail:tt)* ) => { $crate::pin_drop_delegate!{[ !! $($generics)* <] [ $($type)*] [] < $($tail)*} };
    ([ !! $($generics:tt)*] [$($type:tt)*] [] > $($tail:tt)* ) => { $crate::pin_drop_delegate!{[ ! $($generics)* >] [ $($type)*] [] $($tail)*} };
    ([ !! $($generics:tt)*] [$($type:tt)*] [] >> $($tail:tt)* ) => { $crate::pin_drop_delegate!{[ ! $($generics)* >] [ $($type)*] [] > $($tail)*} };
    ([ !! $($generics:tt)*] [$($type:tt)*] [] $token:tt $($tail:tt)* ) => { $crate::pin_drop_delegate!{[!! $($generics)* $token] [$($type)*] [] $($tail)*} };

    ([ ! $($generics:tt)*] [$($type:tt)*] [] , $generic:tt $($tail:tt)* ) => { $crate::pin_drop_delegate!{[ ! $($generics)* , $generic] [ $($type)* , $generic] [] $($tail)*} };
    ([ ! $($generics:tt)*] [$($type:tt)*] [] > ) => { $crate::pin_drop_delegate!{ [ $($generics)*] [ $($type)* > ] [] } };
    ([ ! $($generics:tt)*] [$($type:tt)*] [] > where $($tail:tt)* ) => { $crate::pin_drop_delegate!{[ $($generics)*] [ $($type)* >] [$($tail)*]  } };
    ([ ! $($generics:tt)*] [$($type:tt)*] [] $token:tt $($tail:tt)* ) => { $crate::pin_drop_delegate!{[! $($generics)* $token] [$($type)*] [] $($tail)*} };

    ([$($generics:tt)*] [! $($type:tt)*] [] < $generic:tt $($tail:tt)* ) => { $crate::pin_drop_delegate!{ [ ! $($generics)* $generic] [$($type)* < $generic] [] $($tail)*} };
    ([$($generics:tt)*] [! $($type:tt)*] [] $name_part:tt $($tail:tt)* ) => { $crate::pin_drop_delegate!{ [ $($generics)* ] [! $($type)* $name_part] [] $($tail)*} };

    ([$($generics:tt)*] [$($type:tt)+] [$($where:tt)*] ) => {
        impl<$($generics)*> core::ops::Drop for $($type)+ where $($where)*{
            fn drop(&mut self){
                unsafe{ $crate::PinDrop::drop(core::pin::Pin::new_unchecked(self), $crate::DropMarker::new()) }
            }
        }
    };

    ( $($tail:tt)* ) => { $crate::pin_drop_delegate!{ [] [!] [] $($tail)* } };

}
