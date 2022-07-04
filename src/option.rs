use crate::{
    CustomWrapper, DerefProjectable, FinalizeProjection, Helper, Owned, Projectable,
    ProjectableMarker, SupportsPacked,
};
use core::marker::PhantomData;
use core::mem::transmute_copy;
use core::ptr::null_mut;

unsafe impl<T> CustomWrapper for Option<T> {
    type Output = Option<T>;
}
unsafe impl<'a, T> CustomWrapper for &Option<&'a T> {
    type Output = Option<Helper<&'a T>>;
}
unsafe impl<'a, T> CustomWrapper for &Option<&'a mut T> {
    type Output = Option<Helper<&'a mut T>>;
}
unsafe impl<T: CustomWrapper> CustomWrapper for &&Option<T> {
    type Output = Option<T::Output>;
}
unsafe impl<T> Projectable for Option<T> {
    type Target = T;
    type Marker = Option<<Owned<T> as Projectable>::Marker>;

    fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
        (
            unsafe { core::mem::transmute(self.as_ref()) },
            self.as_ref().map(|_| []),
        )
    }
}

// unsafe impl<'a, T> Projectable for &Option<T>
// where
//     Helper<T>: Projectable,
// {
//     type Target = <Helper<T> as Projectable>::Target;
//     type Marker = Option<<Helper<T> as Projectable>::Marker>;
//
//     fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
//         if let Some(x) = self {
//             let (raw, marker) = unsafe { &*(x as *const T as *const Helper<T>) }.get_raw();
//             (raw as _, Some(marker))
//         } else {
//             (null_mut(), None)
//         }
//     }
// }
unsafe impl<T> Projectable for &&Option<T>
where
    T: Projectable,
{
    type Target = T::Target;
    type Marker = Option<T::Marker>;

    fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
        if let Some(x) = self {
            let (raw, marker) = x.get_raw();
            (raw as _, Some(marker))
        } else {
            (null_mut(), None)
        }
    }
}
unsafe impl<'a, T> Projectable for &&&'a Option<T>
where
    &'a T: Projectable,
{
    type Target = <&'a T as Projectable>::Target;
    type Marker = Option<<&'a T as Projectable>::Marker>;

    fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
        if let Some(x) = self {
            let (raw, marker) = x.get_raw();
            (raw as _, Some(marker))
        } else {
            (null_mut(), None)
        }
    }
}

impl<T, M: ProjectableMarker<T>> ProjectableMarker<T> for Option<M> {
    type Output = Option<M::Output>;

    unsafe fn from_raw(&self, raw: *mut T) -> Self::Output {
        self.as_ref().map(|m| m.from_raw(raw))
    }
}

impl<T: FinalizeProjection> FinalizeProjection for Option<T> {
    type Output = Option<T::Output>;

    unsafe fn finalize(&self) -> Self::Output {
        self.as_ref().map(|x| x.finalize())
    }
}

// todo make more general Option flattening
impl<T> FinalizeProjection for &Option<Option<T>> {
    type Output = Option<T>;

    unsafe fn finalize(&self) -> Self::Output {
        if let Some(Some(x)) = self {
            Some(transmute_copy(x))
        } else {
            None
        }
    }
}

unsafe impl<T: DerefProjectable> DerefProjectable for Option<T>
where
    // temporary limitation
    T::Target: Sized,
{
    type Target = T::Target;
    type Marker = Option<T::Marker>;

    fn deref_raw(&self) -> (*mut Self::Target, Self::Marker) {
        if let Some(x) = self {
            let (raw, marker) = x.deref_raw();
            (raw, Some(marker))
        } else {
            (null_mut(), None)
        }
    }
}
unsafe impl<'a, T, Target, Marker> DerefProjectable for &'a Option<T>
where
    &'a T: DerefProjectable<Target = Target, Marker = Marker>,
{
    type Target = Target;
    type Marker = Option<Marker>;

    fn deref_raw(&self) -> (*mut Self::Target, Self::Marker) {
        if let Some(x) = self {
            let (raw, marker) = x.deref_raw();
            (raw, Some(marker))
        } else {
            (null_mut(), None)
        }
    }
}

unsafe impl<'a, 'b, ToCheck, NotPacked, M> SupportsPacked
    for &'a (*mut ToCheck, &'b Option<M>, PhantomData<NotPacked>)
where
    &'a (*mut ToCheck, &'b M, PhantomData<NotPacked>): SupportsPacked<Result = NotPacked>,
{
    type Result = NotPacked;
}
