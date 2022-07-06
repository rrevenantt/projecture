use projecture::project;
use std::marker::PhantomPinned;
use std::pin::Pin;

// all tests are doc-tests for now
#[test]
fn test_nested() {
    // #[derive(Default)]
    struct Foo {
        a: Bar,
        b: Box<Bar>,
        c: Pin<Box<Bar>>,
        d: PhantomPinned,
    }

    #[derive(Default)]
    struct Bar {
        e: usize,
        d: PhantomPinned,
    }

    fn test(arg: &mut Foo) {
        project!(let Foo { a: Bar { e: e1 }, b: *Bar{ e:e2, ..}, c: *Bar{ d, .. } , ..} = &mut *arg);
        // let result = project!(deref arg);
        // project!( let Bar { d, .. }   = result );
        // drop(marker);
        // drop(marker);
        let e1: &mut usize = e1;
        let e2: &mut usize = e2;
        let d: Pin<&mut PhantomPinned> = d;

        let x: &mut usize = project!(arg -> a -> e);
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
