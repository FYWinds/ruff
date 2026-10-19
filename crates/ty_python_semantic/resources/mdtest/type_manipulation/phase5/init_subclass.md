# `UpdateClass` in `__init_subclass__`

When a class is defined, every ancestor whose `__init_subclass__` returns `UpdateClass[...]` updates
the new class ([PEP 827]). If `cls` is annotated as `type[T]`, `T` is the new class. The class that
defines `__init_subclass__` is not updated itself.

```toml
[environment]
python-version = "3.14"
```

## A model base class

The PEP's Pydantic-style example: every subclass of `Model` gets a keyword-only `__init__`.

```py
from typing import Callable, Literal
from ty_extensions import Attrs, Iter, Member, Param, ParamKind, Params, UpdateClass

type InitFnType[T] = Member[
    Literal["__init__"],
    Callable[
        Params[
            Param[Literal["self"], T],
            *[Param[p.name, p.type, Literal[ParamKind.KEYWORD_ONLY], p.init] for p in Iter[Attrs[T]]],
        ],
        None,
    ],
    Literal["ClassVar"],
]

class Model:
    def __init_subclass__[T](cls: type[T]) -> UpdateClass[InitFnType[T]]:
        pass

class Hero(Model):
    name: str
    age: int | None = None

class Sidekick(Hero):
    power: str

reveal_type(Hero.__init__)  # revealed: (self: Hero, *, name: str, age: int | None = None) -> None
reveal_type(Sidekick.__init__)  # revealed: (self: Sidekick, *, name: str, age: int | None = None, power: str) -> None

Hero(name="Deadpond")
Sidekick(name="Rusty", power="none")
Hero(name="Deadpond", power="none")  # error: [unknown-argument]
```

The body of `__init_subclass__` is just `pass`: a function returning `UpdateClass[...]` may return
anything, so there is no `invalid-return-type` error.

## The defining class is not updated

```py
from typing import Literal
from ty_extensions import Member, UpdateClass

class Base:
    def __init_subclass__[T](cls: type[T]) -> UpdateClass[Member[Literal["tag"], str]]:
        pass

class Child(Base): ...

def _(base: Base, child: Child):
    reveal_type(child.tag)  # revealed: str
    base.tag  # error: [unresolved-attribute]
```

## Order of application

### Ancestors are applied from the most basic one

All ancestors with an `UpdateClass` hook are applied, starting from the most basic one (reverse MRO
order, as specified by the PEP). Each update sees the result of the previous ones.

The reference implementation applies them in MRO order instead (`test_update_class_inheritance_02`),
which would give `list[set[int]]` here.

```py
from ty_extensions import Attrs, Iter, Member, UpdateClass

class A:
    def __init_subclass__[T](cls: type[T]) -> UpdateClass[*[Member[m.name, list[m.type]] for m in Iter[Attrs[T]]]]:
        pass

class B(A):
    def __init_subclass__[T](cls: type[T]) -> UpdateClass[*[Member[m.name, set[m.type]] for m in Iter[Attrs[T]]]]:
        pass

class C(B):
    x: int

def _(c: C):
    reveal_type(c.x)  # revealed: set[list[int]]
```

### Inherited attributes are wrapped again

`Attrs[T]` includes inherited attributes, so a hook that maps over all attributes wraps an inherited
attribute once more in every subclass, as it would at runtime: `D` sees `x: list[int]` inherited
from `C` and redeclares it as `list[list[int]]`. Within one class, an update only sees the class as
it was before that update, so it is never applied to its own output.

```py
from ty_extensions import Attrs, Iter, Member, UpdateClass

class A:
    def __init_subclass__[T](cls: type[T]) -> UpdateClass[*[Member[m.name, list[m.type]] for m in Iter[Attrs[T]]]]:
        pass

class C(A):
    x: int

class D(C):
    y: str

def _(c: C, d: D):
    reveal_type(c.x)  # revealed: list[int]
    reveal_type(d.x)  # revealed: list[list[int]]
    reveal_type(d.y)  # revealed: list[str]
```

## Generic base classes

The type parameters of the ancestor are replaced according to how the new class specializes it.

```py
from typing import Literal
from ty_extensions import Member, UpdateClass

class Tagged[V]:
    def __init_subclass__[T](cls: type[T]) -> UpdateClass[Member[Literal["tag"], V]]:
        pass

class IntTagged(Tagged[int]): ...

class Wrapper[U](Tagged[list[U]]): ...

def _(a: IntTagged, b: Wrapper[str]):
    reveal_type(a.tag)  # revealed: int
    reveal_type(b.tag)  # revealed: list[str]
```

## Hooks that do not return `UpdateClass`

An ordinary `__init_subclass__` does not change the subclass.

```py
class Registry:
    def __init_subclass__(cls, **kwargs: object) -> None:
        pass

class Plugin(Registry):
    name: str

reveal_type(Plugin().name)  # revealed: str
```

## Cycles between classes

If the update of one class inspects another class whose update inspects the first one, the result
would depend on the order of evaluation. Following the PEP's suggestion, ty rejects this when the
classes are checked, and neither update is applied.

The return annotations below depend on `T` (through `Lazy`), so they are only evaluated when a class
is updated. A return annotation like `UpdateClass[*[m for m in Iter[Attrs[Q]]]]` would be evaluated
where the decorator is defined, and the cycle would then show up while inferring the decorator
signatures instead; that case is not covered here.

```py
from typing import Literal
from ty_extensions import Attrs, IsAssignable, Iter, Member, UpdateClass

type Lazy[T, C] = C if IsAssignable[T, T] else C

def copy_from_q[T](cls: type[T]) -> UpdateClass[*[m for m in Iter[Attrs[Lazy[T, Q]]]]]:
    return cls

def copy_from_p[T](cls: type[T]) -> UpdateClass[*[m for m in Iter[Attrs[Lazy[T, P]]]]]:
    return cls

@copy_from_q  # error: [invalid-type-form] "UpdateClass evaluation depends on itself"
class P:
    a: int

@copy_from_p  # error: [invalid-type-form] "UpdateClass evaluation depends on itself"
class Q:
    b: str

def _(p: P, q: Q):
    reveal_type(p.a)  # revealed: int
    p.b  # error: [unresolved-attribute]
    q.a  # error: [unresolved-attribute]
```

[PEP 827]: https://peps.python.org/pep-0827/
