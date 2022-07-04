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
    type Marker = Ref<'a, ()>;

    fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
        (
            self.deref() as *const _ as _,
            Ref::map(unsafe { transmute_copy::<_, Self>(self) }, |_| &()),
        )
    }
}

impl<'a, T: 'a> ProjectableMarker<T> for Ref<'a, ()> {
    type Output = Ref<'a, T>;

    unsafe fn from_raw(&self, raw: *mut T) -> Self::Output {
        Ref::map(Ref::clone(self), |_| &*raw)
    }
}

unsafe impl<'a, T> CustomWrapper for RefMut<'a, T> {
    type Output = RefMut<'a, T>;
}

unsafe impl<'a, T> Projectable for RefMut<'a, T> {
    type Target = T;
    type Marker = RefMut<'a, ()>;

    fn get_raw(&self) -> (*mut Self::Target, Self::Marker) {
        (
            // nightly :'( RefMut::leak(unsafe { transmute_copy::<_,Self>(self)}),
            &mut **ManuallyDrop::new(unsafe { transmute_copy::<_, Self>(self) }),
            RefMut::map(unsafe { transmute_copy::<_, Self>(self) }, |_| unsafe {
                NonNull::<()>::dangling().as_mut()
            }),
        )
    }
}

impl<'a, T: 'a> ProjectableMarker<T> for RefMut<'a, ()> {
    type Output = RefMut<'a, T>;

    unsafe fn from_raw(&self, raw: *mut T) -> Self::Output {
        let (old, out) = RefMut::map_split(transmute_copy::<_, Self>(self), |x| (x, &mut *raw));
        mem::forget(old);
        out
    }
}
