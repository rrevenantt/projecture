use crate::{
    CustomWrapper, DerefProjectable, FinalizeProjection, Helper, Marker, Owned, Projectable,
    ProjectableMarker, SupportsPacked,
};
use core::marker::PhantomData;
use core::mem::transmute_copy;
use core::ptr::null_mut;

unsafe impl<T> CustomWrapper for Option<T> {
    type Output = Option<Owned<T>>;
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
unsafe impl<T> Projectable for Option<Owned<T>> {
    type Target = T;
    type Marker = OptionMarker<<Owned<T> as Projectable>::Marker>;

    fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
        (
            unsafe { core::mem::transmute(self.as_ref()) },
            self.as_ref().map(|_| Marker::new()).into(),
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
#[repr(transparent)]
pub struct OptionMarker<T>(Option<T>);
impl<T> OptionMarker<T> {
    pub fn check(&self) {}
}
impl<T> From<Option<T>> for OptionMarker<T> {
    fn from(from: Option<T>) -> Self {
        OptionMarker(from)
    }
}

unsafe impl<T> Projectable for &&Option<T>
where
    T: Projectable,
{
    type Target = T::Target;
    type Marker = OptionMarker<T::Marker>;

    fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
        if let Some(x) = self {
            let (raw, marker) = x.get_raw();
            (raw as _, Some(marker).into())
        } else {
            (null_mut(), None.into())
        }
    }
}
unsafe impl<'a, T> Projectable for &&&'a Option<T>
where
    &'a T: Projectable,
{
    type Target = <&'a T as Projectable>::Target;
    type Marker = OptionMarker<<&'a T as Projectable>::Marker>;

    fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
        if let Some(x) = self {
            let (raw, marker) = x.get_raw();
            (raw as _, Some(marker).into())
        } else {
            (null_mut(), None.into())
        }
    }
}

impl<T, M: ProjectableMarker<T>> ProjectableMarker<T> for OptionMarker<M> {
    type Output = Option<M::Output>;

    unsafe fn from_raw(&self, raw: *mut T) -> Self::Output {
        self.0.as_ref().map(|m| m.from_raw(raw))
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
    type Marker = OptionMarker<T::Marker>;

    fn deref_raw(&self) -> (*mut Self::Target, Self::Marker) {
        if let Some(x) = self {
            let (raw, marker) = x.deref_raw();
            (raw, Some(marker).into())
        } else {
            (null_mut(), None.into())
        }
    }
}
unsafe impl<'a, T, Target, Marker> DerefProjectable for &'a Option<T>
where
    &'a T: DerefProjectable<Target = Target, Marker = Marker>,
{
    type Target = Target;
    type Marker = OptionMarker<Marker>;

    fn deref_raw(&self) -> (*mut Self::Target, Self::Marker) {
        if let Some(x) = self {
            let (raw, marker) = x.deref_raw();
            (raw, Some(marker).into())
        } else {
            (null_mut(), None.into())
        }
    }
}

unsafe impl<'a, 'b, ToCheck, NotPacked, M> SupportsPacked
    for &'a (*mut ToCheck, &'b OptionMarker<M>, PhantomData<NotPacked>)
where
    &'a (*mut ToCheck, &'b M, PhantomData<NotPacked>): SupportsPacked<Result = NotPacked>,
{
    type Result = NotPacked;
}
