use crate::{CustomWrapper, Projectable, ProjectableMarker};
use core::cell::{Ref, RefMut};
use core::mem;
use core::mem::{transmute_copy, ManuallyDrop};
use core::ops::Deref;
use core::ptr::NonNull;

unsafe impl<'a, T> CustomWrapper for Ref<'a, T> {
    type Output = Ref<'a, T>;
}

unsafe impl<'a, T> Projectable for Ref<'a, T> {
    type Target = T;
    type Marker = RefCellMarker<Ref<'a, ()>>;

    fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
        (
            self.deref() as *const _ as _,
            RefCellMarker(Ref::map(unsafe { transmute_copy::<_, Self>(self) }, |_| {
                &()
            })),
        )
    }
}

#[repr(transparent)]
pub struct RefCellMarker<T>(T);
impl<T> RefCellMarker<T> {
    pub fn check(&self) {}
}

impl<'a, T: 'a> ProjectableMarker<T> for RefCellMarker<Ref<'a, ()>> {
    type Output = Ref<'a, T>;

    unsafe fn from_raw(&self, raw: *mut T) -> Self::Output {
        Ref::map(Ref::clone(&self.0), |_| &*raw)
    }
}

unsafe impl<'a, T> CustomWrapper for RefMut<'a, T> {
    type Output = RefMut<'a, T>;
}

unsafe impl<'a, T> Projectable for RefMut<'a, T> {
    type Target = T;
    type Marker = RefCellMarker<RefMut<'a, ()>>;

    fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
        let marker = RefCellMarker(RefMut::map(
            unsafe { transmute_copy::<_, Self>(self) },
            |_| unsafe {
                &mut *NonNull::<()>::dangling().as_ptr()
                // bugged on msrv
                // NonNull::<()>::dangling().as_mut()
            },
        ));
        (
            // nightly :'(
            // RefMut::leak(unsafe { transmute_copy::<_, Self>(self) }),
            &mut **ManuallyDrop::new(unsafe { transmute_copy::<_, Self>(self) }),
            marker,
        )
    }
}

impl<'a, T: 'a> ProjectableMarker<T> for RefCellMarker<RefMut<'a, ()>> {
    type Output = RefMut<'a, T>;

    unsafe fn from_raw(&self, raw: *mut T) -> Self::Output {
        let (old, out) = RefMut::map_split(transmute_copy::<_, RefMut<'a, ()>>(self), |x| {
            (x, &mut *raw)
        });
        mem::forget(old);
        out
    }
}
