// trait HasSpecialization<S>: Sized {
// }

use alloc::string::String;
use core::marker::PhantomData;
use core::mem::{transmute, transmute_copy};
use core::ops::{Deref, DerefMut};
use core::pin::Pin;
use core::ptr;

struct Specializer<T, S>(pub T, PhantomData<S>);

impl<T, S> Specializer<T, S> {
    fn select(spec: &S) -> Specializer<PhantomData<T>, S> {
        Specializer(PhantomData, PhantomData)
    }
}
impl<S> Specializer<PhantomData<()>, S> {
    unsafe fn init<T>(self, arg: T) -> Specializer<T, S> {
        Specializer(arg, PhantomData)
    }
}

fn test<T: HasSpecialization<S>, S>(x: Specializer<T, S>)
where
    S: SimpleTrait,
{
    x.0.specialized_ref().simple_call()
}

#[cfg(test)]
#[test]
fn call_test() {
    let arg = Foo("");
    let tmp = outermost_level(&arg).select();
    test(unsafe { tmp.init(arg) });

    let arg = Foo(1usize);
    let tmp = outermost_level(&arg).select();
    test(unsafe { tmp.init(arg) });
}

trait SimpleTrait {
    fn simple_call(&self);
}

struct Foo<X>(X);
impl<X> SimpleTrait for Foo<X> {
    fn simple_call(&self) {
        panic!("1");
    }
}

impl SimpleTrait for Level<Foo<usize>> {
    fn simple_call(&self) {
        panic!("2")
    }
}

trait SimpleTraitSpecializer: Sized {
    fn select(&self) -> Specializer<PhantomData<()>, Self> {
        Specializer(PhantomData, PhantomData)
    }
}
impl<T: SimpleTrait> SimpleTraitSpecializer for T {}

impl<X> SpecializationOf for Foo<X> {
    type Original = Foo<X>;
}
// impl<X> HasSpecialization<Foo<X>> for Foo<X> {}
//
// impl<X: HasSpecialization<Y>, Y> HasSpecialization<Level<Y>> for X {}
// impl<X> HasSpecialization<X> for X {
//     fn specialized_ref(&self) -> &X {
//         self
//     }
//
//     fn specialized_mut(&mut self) -> &mut X {
//         self
//     }
//
//     fn specialized(self) -> X {
//         self
//     }
// }

// trait Seal {}
// impl<T> Seal for T where T: HasSpecialization<T> {}

trait SpecializationOf {
    type Original;
}
impl<X: SpecializationOf> SpecializationOf for Level<X> {
    type Original = X::Original;
}

fn outermost_level<T>(
    arg: &T,
) -> &Level<Level<Level<Level<Level<Level<Level<Level<Level<Level<T>>>>>>>>>> {
    unsafe { transmute(arg) }
}

#[repr(transparent)]
struct Level<T>(pub T);
impl<T> Deref for Level<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<T> DerefMut for Level<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
type Implements<T> = Level<T>;

// fn test1<T: Clone + ?Copy>(arg: T) {}
// fn test2<T: Spec>(arg: T) {}
//
// fn caller1<T: Clone>(arg: T) {
//     // test2(arg.clone());
//     test2(1usize);
//     test2(String);
//     #[derive(Clone)]
//     struct Test<T>(T);
//     impl<T: Clone> Copy for Test<T> {}
//     test2(Test(arg.clone()));
//     // #[derive(Clone,Copy)]
//     // struct Test2<T>(T);
//     // test2(Test2(arg.clone()));
//     #[derive(Clone)]
//     struct Test3<T>(T);
//     test2(Test3(arg.clone()));
// }
// fn caller2<T:Clone>(arg:T){
//
// }
// trait Trait: Clone + ?Copy {}

// impl<T: Clone + ?Copy + ?Concrete> Spec for T {}
// impl<T: Clone + Copy + ?Concrete> Spec for T {}
// impl<T: Clone + Copy + Concrete<Type = usize> + ?Concrete<Type = String>> Spec for T {}
// impl<T: Clone + Copy + ?Concrete<Type = usize> + Concrete<Type = String>> Spec for T {}
// impl<T: Clone + Copy + Concrete<Type = usize> + Concrete<Type = String>> Spec for T {}
trait Spec<Marker: Bool> {}

struct True;
impl Bool for True {}
struct False;
impl Bool for False {}

impl<T: Clone + MaybeCopy<IsCopy = True>> Spec<True> for T {}
impl<T: Clone + MaybeCopy> Spec<False> for T {}

trait MaybeCopy {
    type IsCopy: Bool;
    type SelfButKnownToBeCopy: Copy + TypeEq<Self>;
}

// impl<T> MaybeCopy for T {
//     type IsCopy = False;
//     type SelfButKnownToBeCopy = !;
// }
impl<T: Copy> MaybeCopy for T {
    type IsCopy = True;
    type SelfButKnownToBeCopy = T;
}

trait TypeEq<T> {
    fn concrete(self) -> T;
    fn concrete_ref(&self) -> &T;
    fn concrete_mut(&mut self) -> &mut T;
}

impl<T> TypeEq<T> for T {
    fn concrete(self) -> T {
        unsafe { transmute_copy(&self) }
    }

    fn concrete_ref(&self) -> &T {
        unsafe { transmute_copy(&self) }
    }

    fn concrete_mut(&mut self) -> &mut T {
        unsafe { transmute_copy(&self) }
    }
}
