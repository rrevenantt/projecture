use projecture::project;
use std::marker::PhantomPinned;
use std::pin::Pin;

// all tests are doc-tests for now
#[test]
fn test() {
    struct Foo {
        a: Bar,
        b: Box<Bar>,
        c: Pin<Box<Bar>>,
        d: PhantomPinned,
    }

    struct Bar {
        e: usize,
        d: PhantomPinned,
    }

    fn test1(arg: &mut Foo) {
        project!(let Foo { a: Bar { e: e1 }, b: *Bar{ e:e2, ..}, c: *Bar{ d, .. } , ..} = &mut *arg);
        // let result = project!(deref arg);
        // project!( let Bar { d, .. }   = result );
        // drop(marker);
        // drop(marker);
        let e1: &mut usize = e1;
        let e2: &mut usize = e2;
        let d: Pin<&mut PhantomPinned> = d;

        // let x: &mut usize = project!(arg -> c -> e);
    }
}
