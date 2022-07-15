## Projecture

**This is in a proof of concept state and also internally uses a lot of not yet battle-tested unsafe code
so use it on your own risk meanwhile if you are good with unsafe rust i would appreciate a soundness review**

Allows to do almost arbitrary type projections. In comparison to other crates that do similar things
it is not just more generic as such requires much less transitive dependencies
and also does not impose additional requirements on target struct,
if target struct is located in external crate that crate does not have to explicitly add a support such projection.

Although as of now this crate doesn't support enums yet, but it will be added later.

#### Currently can do following type of projections
- Destructuring projection (similar to usual `let <pattern>` but also supports deref pattern,
  and also works if struct implements `Drop` which is just not called). <br>
  **Note** that due to limitations of declaration macros currently unmentioned fields are leaked.
- Reference(`&`, `&mut`) projection (similar to match ergonomics in `let <pattern>` but also supports deref pattern)
- `Pin` projection
- `Cell` projection
- `MaybeUninit` projection
- `Atomic`(from [`atomic`] crate) projection
- `Option` projection (which works together with other kinds of projections)
- `RefCell` guards projection
- raw pointers projections (`*const T`, `*mut T`, `NonNull<T>`)

Also, where possible, projections can additionally project through a `Deref` type.

Here is a general overview of what you can do, see [`project`]! macro for more usage details.
```rust
#    use std::cell::Cell;
#    use std::marker::PhantomPinned;
#    use std::pin::Pin;
#    use std::rc::Rc;
#    use atomic::Atomic;
#    use projecture::project;
    struct Foo {
        a: Bar,
        b: Rc<Cell<Bar>>,
        c: Pin<Box<Bar>>,
        d: Atomic<Bar>,
    }

    struct Bar(usize,PhantomPinned);

    fn test(arg: &Foo) {
        project!(
        let Foo {
            a: Bar (e, ..),
            b: *Bar{ 0: cell },
            c: *Bar (_, f) ,
            d: Bar(atomic, ..),
        } = arg);
        let _: &usize = e;
        let _: &Cell<usize> = cell;
        let _: &Atomic<usize> = atomic;
        let _: Pin<& PhantomPinned> = f;

        let _: &usize = project!(arg -> a -> 0);
        let _: &Cell<usize> = project!(arg -> b -> 0);
        let _: &Atomic<usize> = project!(arg -> d -> 0);
        let _: Pin<& PhantomPinned> = project!(arg -> c -> 1);
    }
```


Also allows dependent crates to define their own projections via traits.
see `atomic` module for example of how to do a projection of a transparent field wrapper
or `Pin` for doing projections on a custom reference type

MSRV: 1.53 <br>
License: MIT

[`atomic`]: https://docs.rs/atomic