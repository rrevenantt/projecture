// #![feature(arbitrary_self_types)]
use projecture::{pin_projectable, project, OptionMarker};
use std::cell::Cell;
use std::fmt::Debug;
use std::marker::PhantomPinned;
use std::pin::Pin;
use std::ptr::addr_of_mut;

// all tests are doc-tests for now
#[test]
fn test_nested() {
    // #[derive(Default)]
    struct Foo {
        a: Bar,
        b: Box<Cell<Bar>>,
        c: Pin<Box<Bar>>,
        d: PhantomPinned,
    }

    #[derive(Default)]
    struct Bar {
        e: usize,
        d: PhantomPinned,
    }

    pin_projectable! { Bar }

    fn test(arg: &mut Foo) {
        project!(let Foo { a: Bar { e: e1 }, b: *Bar{ e: e2, .. }, c: *Bar{ d, .. } , ..} = &mut *arg);

        let e1: &mut usize = e1;
        let e2: &mut usize = e2;
        let d: Pin<&mut PhantomPinned> = d;

        let x: &mut usize = project!((&mut *arg) -> a -> e);
        //todo
        let x: Pin<&mut PhantomPinned> = project!((&mut *arg) -> c -> d);
        let x = project!((&*arg) -> b -> e);
    }
    test(&mut Foo {
        a: Default::default(),
        b: Box::new(Default::default()),
        c: Box::pin(Default::default()),
        d: Default::default(),
    });
}

#[test]
fn test_packed_destructuring() {
    #[repr(packed)]
    struct Foo(u8, usize);
    let foo = Foo(1, 2);
    project!(let Foo(x,y) = foo);
    assert_eq!((x, y), (1, 2));
}

#[test]
fn test_tmp() {
    #[derive(Default)]
    struct Foo {
        x: Box<usize>,
        y: usize,
        p: PhantomPinned,
    }
    let mut arg = Some(Foo::default());
    let var = core::mem::ManuallyDrop::new(arg);
    let var = {
        use ::projecture::Preprocess;
        core::mem::ManuallyDrop::new((&&&&&var).preprocess())
    };
    let (ptr, marker) = {
        use ::projecture::Projectable;
        (&&&&&&&*var).get_raw()
    };
    if false {
        let Foo { x: _, .. } = unsafe { &*ptr };
    }
    #[allow(unused_mut)]
    let mut tmp = unsafe {
        use ::projecture::{Finalizer, ProjectableMarker, SupportsPacked};

        #[forbid(unaligned_references)]
        #[allow(dead_code)]
        if false {
            struct Foo {
                x: (),
            }
            let check_ptr = (&&(ptr, &marker, core::marker::PhantomData::<Foo>)).select();
            let _ = &(*check_ptr).x;
        }
        fn create_uninit<T>(_ptr: *mut T) -> core::mem::MaybeUninit<T> {
            core::mem::MaybeUninit::uninit()
        }
        let mut mu = create_uninit(ptr);
        let mu_ptr = mu.as_mut_ptr();
        let mu_field_ptr = addr_of_mut!((*mu_ptr).x);
        let offset = (mu_field_ptr as *mut u8).offset_from(mu_ptr as *mut u8);
        fn do_offset<T, U>(ptr: *mut T, _field_ptr_type: *mut U, offset: isize) -> *mut U {
            (ptr as *mut u8).wrapping_offset(offset) as *mut U
        }
        let field_ptr = do_offset(ptr, mu_field_ptr, offset);
        let tmp = core::mem::ManuallyDrop::new(marker.from_raw(field_ptr));
        (&&&&&tmp).call_finalize()
    };
    let x = tmp;
    drop(marker);
    let x: Option<Box<usize>> = x;
    // let y: Option<usize> = y;
}
