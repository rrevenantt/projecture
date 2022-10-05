use crate::{
    project, CustomWrapper, DropLeftovers, Marker, Owned, OwningMarker, Projectable,
    ProjectableMarker,
};
use alloc::boxed::Box;
use core::marker::PhantomData;
use core::mem::{align_of, size_of, transmute_copy, ManuallyDrop};
use core::ops::{Deref, DerefMut};

/// Allows type projections to be available in generic contexts
///
/// ```rust
/// use projecture::generic::GenericPointer;
/// use projecture::{generic_ptr, project, ProjectableMarker};
/// trait Foldable<M, F> {
///     fn fold_fields(_self: GenericPointer<Self, M>, folder: &mut F);
/// }
///
/// trait FoldsWith<F> {
/// #    // fn accept<S: SettingsHList>(&mut self, field: F, settings: S);
///     fn accept(&mut self, field: F);
/// }
///
/// # // trait SettingsHList {
/// # //     fn get_setting<T>(&self) -> Option<&T>;
/// # // }
/// #[derive(Default)]
/// struct Test {
/// #    // #[serde::Rename("name")]
/// #    // #[serde::Skip]
/// #    // #[clap::Short]
///     f1: usize,
///     f2: String,
/// }
///
/// impl<M, F> Foldable<M, F> for Test
/// where
///     M: ProjectableMarker<usize> + ProjectableMarker<String> + Clone,
///     F: FoldsWith<<M as ProjectableMarker<usize>>::Output>
///         + FoldsWith<<M as ProjectableMarker<String>>::Output>,
/// {
///     fn fold_fields(_self: GenericPointer<Self, M>, folder: &mut F) {
///         project!(let Self{f1,f2} = _self);
///         folder.accept(f1.into_concrete());
///         folder.accept(f2.into_concrete());
///     }
/// }
///
/// struct Sizes(usize);
/// impl<T> FoldsWith<&T> for Sizes{
///     fn accept(&mut self, field: &T) {
///         self.0 += core::mem::size_of_val(field);
///     }
/// }
///
/// struct Mutator;
/// impl FoldsWith<&mut usize> for Mutator{
///     fn accept(&mut self, field: &mut usize) {
///         *field += 1;
///     }
/// }
/// impl FoldsWith<&mut String> for Mutator{
///     fn accept(&mut self, field: &mut String) {
///         field.push('1');
///     }
/// }
///
/// let mut x = Test::default();
/// let mut  folder = Sizes(0);
/// Test::fold_fields(generic_ptr!(&x),&mut folder);
/// assert_eq!(folder.0,32);
/// Test::fold_fields(generic_ptr!(&mut x),&mut Mutator);
/// Test::fold_fields(generic_ptr!(&mut x),&mut Mutator);
/// assert_eq!(x.f1,2);
/// assert_eq!(x.f2,"11");
/// ```
///
pub struct GenericPointer<T: ?Sized, M> {
    ptr: *mut (),
    ty: PhantomData<*mut T>,
    marker: M,
}

impl<T, M: ProjectableMarker<T>> GenericPointer<T, M> {
    pub fn into_concrete(self) -> M::Output {
        unsafe { self.marker.from_raw(self.ptr as _) }
    }

    #[doc(hidden)]
    pub unsafe fn new(ptr: *mut T, marker: M) -> Self {
        Self {
            ptr: ptr as _,
            ty: PhantomData,
            marker,
        }
    }
}

unsafe impl<T, M> CustomWrapper for GenericPointer<T, M> {
    type Output = GenericPointer<T, M>;
}

unsafe impl<T, M> Projectable for GenericPointer<T, M> {
    type Target = T;
    type Marker = GenericMarker<M>;

    fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
        unsafe { (self.ptr as _, GenericMarker(transmute_copy(&self.marker))) }
    }
}

pub struct GenericMarker<M>(M);

impl<T, M> ProjectableMarker<T> for GenericMarker<M>
where
    M: ProjectableMarker<T> + Clone,
{
    type Output = GenericPointer<T, M>;

    unsafe fn from_raw(&self, raw: *mut T) -> Self::Output {
        GenericPointer {
            ptr: raw as _,
            ty: PhantomData,
            marker: self.0.clone(),
        }
    }
}

/// macro to create [`GenericPointer`] from regular references/pointers
#[macro_export]
macro_rules! generic_ptr {
    ($val:expr) => {
        unsafe {
            let var = core::mem::ManuallyDrop::new($val);
            let var = {
                use $crate::Preprocess;
                core::mem::ManuallyDrop::new((&&&&&var).preprocess())
            };

            let (ptr, marker) = {
                use $crate::Projectable;
                (&&&&&&&*var).get_raw()
            };
            $crate::generic::GenericPointer::new(ptr, marker)
        }
    };
    (let $name:ident = $val:expr) => {
        let var = core::mem::ManuallyDrop::new($val);
        let var = unsafe {
            use $crate::Preprocess;
            core::mem::ManuallyDrop::new((&&&&&var).preprocess())
        };

        let $name = unsafe {
            use $crate::Projectable;
            let (ptr, marker) = (&&&&&&&*var).get_raw();
            $crate::generic::GenericPointer::new(ptr, marker)
        };
    };
    // todo consider place reference type
    ($name:ident[$idx:expr] = $val:expr) => {};
    ($name:ident[$idx:expr]) => {};
}

/// same as [`Deref`] but generic over reference type
pub trait GenericDeref<M>: DerefTarget {
    fn deref_generic(_self: GenericPointer<Self, M>) -> GenericPointer<Self::Target, M>
    where
        M: DropLeftoversOf<Self>;

    // fn deref_generic_into<X>(_self: GenericPointer<Self, M>, into: DropLeftoversGeneric<'_, X, M>)
    // where
    //     X: GenericDeref<M, Target = Self::Target>;
}

pub trait DerefTarget {
    type Target: ?Sized;
}

impl<X, T: ?Sized + DerefTarget> DerefTarget for GenericPointer<T, X> {
    type Target = T::Target;
}

// impl<M, X: ProjectableMarker<T>, T: ?Sized + GenericDeref<M>> GenericDeref<M>
//     for GenericPointer<T, X>
// {
//     fn deref_generic(_self: GenericPointer<Self, M>) -> GenericPointer<Self::Target, M>
//     where
//         M: DropLeftoversOf<Self>,
//     {
//         _self.into_concrete()
//     }
// }

/// Its a separate trait rather than a part of `GenericDeref` to allow `GenericDeref` to be blanket implemented over the marker type
pub trait DropLeftoversOf<X: ?Sized, Idx = DerefOperation>: Sized {
    unsafe fn drop_leftovers(_self: ManuallyDrop<GenericPointer<X, Self>>, idx: Idx) {}
}
pub struct DerefOperation;

#[cfg(feature = "std")]
impl<T> DropLeftoversOf<Box<T>> for OwningMarker<'_, Box<T>> {
    unsafe fn drop_leftovers(
        _self: ManuallyDrop<GenericPointer<Box<T>, Self>>,
        idx: DerefOperation,
    ) {
        let mut b = ManuallyDrop::new(ManuallyDrop::into_inner(_self).into_concrete());

        unsafe {
            crate::DerefOwned::drop_leftovers(&mut b);
        }
    }
}

impl<T> DropLeftoversOf<Owned<T>> for OwningMarker<'_, Owned<T>> {
    unsafe fn drop_leftovers(
        _self: ManuallyDrop<GenericPointer<Owned<T>, Self>>,
        idx: DerefOperation,
    ) {
    }
}

impl<T, I> DropLeftoversOf<T, I> for Marker<&'_ ()> {}
impl<T, I> DropLeftoversOf<T, I> for Marker<&'_ mut ()> {}

pub struct DropLeftoversGeneric<'a, T, M>(
    ManuallyDrop<GenericPointer<T, M>>,
    PhantomData<fn(&'a ()) -> &'a ()>,
);

pub trait DropV2<M> {
    fn drop(_self: ManuallyDrop<GenericPointer<Self, M>>);
    // fn drop(_self: GenericPointer<Self, ManuallyDrop<M>>);
}

/// WIP
#[doc(hidden)]
pub trait FnV2<M, Args> {
    type Output;
    fn call(_self: GenericPointer<Self, M>, args: Args) -> Self::Output;
    // fn call(_self: GenericPointer<Self, M>, args: Args) -> GenericPointer<Self::Output, M>;
    // fn call<M>(_self: GenericPointer<Self, M>, args: Args) -> GenericPointer<Self::Output, M>;
}

/// Same as [`core::ops::Index`] but generic over reference type
pub trait IndexV2<M, I> {
    type Output;
    fn index(_self: GenericPointer<Self, M>, idx: I) -> GenericPointer<Self::Output, M>
    where
        M: DropLeftoversOf<Self, I>;
}
