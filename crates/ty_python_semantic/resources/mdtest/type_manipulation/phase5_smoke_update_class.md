# Phase 5 smoke test: `UpdateClass`

Copies of the phase 5 drafts `design/mdtests/phase5/update_class.md` and
`design/mdtests/phase5/init_subclass.md`, plus the `dataclass-update-class` and
`model-init-subclass` examples of `design/mdtests/phase5/examples.md`. Fixes to the drafts are
noted where they are made.

## `UpdateClass` in class decorators

`UpdateClass[*Ms]` from [PEP 827] is the return type of a class decorator (or of
`__init_subclass__`, see `init_subclass.md`) that adds, replaces or removes members of the decorated
class. Each argument is a `Member`:

- a member whose type is `Never` removes the member;
- a `ClassVar` member whose type is callable becomes a method;
- any other member becomes an attribute declared in the class body.

The decorated name still refers to the original (updated) class. The reference implementation only
supports `__init_subclass__`; ty supports both forms described in the PEP.

```toml
[environment]
python-version = "3.14"
```

### A dataclass-like decorator

#### Synthesizing `__init__`

This is the example from the PEP: the decorator synthesizes a keyword-only `__init__` from the
attributes of the class.

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

def dataclass_ish[T](cls: type[T]) -> UpdateClass[InitFnType[T]]:
    return cls

@dataclass_ish
class Point:
    x: int
    y: int = 0

reveal_type(Point)  # revealed: <class 'Point'>
reveal_type(Point.__init__)  # revealed: (self: Point, *, x: int, y: int = 0) -> None
reveal_type(Point(x=1))  # revealed: Point
reveal_type(Point(x=1, y=2))  # revealed: Point

Point()  # error: [missing-argument]
Point(1, x=1)  # error: [too-many-positional-arguments]
Point(x="a")  # error: [invalid-argument-type]
Point(x=1, z=2)  # error: [unknown-argument]
```

#### Replacing an `__init__` from the class body

The update replaces an `__init__` written in the class body:

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

def dataclass_ish[T](cls: type[T]) -> UpdateClass[InitFnType[T]]:
    return cls

@dataclass_ish
class Point:
    x: int

    def __init__(self, x: int) -> None:
        self.x = x

reveal_type(Point.__init__)  # revealed: (self: Point, *, x: int) -> None
Point(1, x=1)  # error: [too-many-positional-arguments]
```

### Adding and replacing attributes

#### New and replaced attributes

New members come first among the members of the updated class, followed by its other members.

```py
from typing import Literal
from ty_extensions import Attrs, Iter, Member, UpdateClass

def with_id[T](cls: type[T]) -> UpdateClass[Member[Literal["id"], int]]:
    return cls

def str_name[T](cls: type[T]) -> UpdateClass[Member[Literal["name"], str]]:
    return cls

@with_id
class Item:
    name: bytes

@str_name
class Renamed:
    name: bytes
    size: int

type Names[T] = tuple[*[p.name for p in Iter[Attrs[T]]]]

def _(item: Item, renamed: Renamed, names: Names[Item], renamed_names: Names[Renamed]):
    reveal_type(item.id)  # revealed: int
    reveal_type(item.name)  # revealed: bytes
    reveal_type(renamed.name)  # revealed: str
    reveal_type(names)  # revealed: tuple[Literal["id"], Literal["name"]]
    reveal_type(renamed_names)  # revealed: tuple[Literal["name"], Literal["size"]]
```

#### The definer of an updated member

The definer of an updated member is the updated class:

```py
from typing import Literal
from ty_extensions import GetMember, Member, UpdateClass

def with_id[T](cls: type[T]) -> UpdateClass[Member[Literal["id"], int]]:
    return cls

class Base:
    id: str

@with_id
class Child(Base): ...

type Id = GetMember[Child, Literal["id"]]

def _(definer: Id.definer, type_: Id.type, child: Child):
    reveal_type(definer)  # revealed: Child
    reveal_type(type_)  # revealed: int
    reveal_type(child.id)  # revealed: int
```

### Removing members

A member with type `Never` removes the member. It is no longer listed by `Attrs` and `Members`, and
accessing it gives `Never` (it is declared with type `Never` on the updated class, which hides the
inherited declaration).

```py
from typing import Literal, Never
from ty_extensions import Attrs, Iter, Member, UpdateClass

def drop_secret[T](cls: type[T]) -> UpdateClass[Member[Literal["secret"], Never]]:
    return cls

class Base:
    secret: str

@drop_secret
class Account(Base):
    name: str

type Names[T] = tuple[*[p.name for p in Iter[Attrs[T]]]]

def _(account: Account, names: Names[Account]):
    reveal_type(names)  # revealed: tuple[Literal["name"]]
    reveal_type(account.secret)  # revealed: Never
```

### Several decorators

Decorators are applied from the inside out, and each one sees the result of the previous ones. An
update that maps over the attributes of the class sees the class as it was before that update, so it
does not apply to its own output.

```py
from typing import Literal
from ty_extensions import Attrs, Iter, Member, UpdateClass

def add_x[T](cls: type[T]) -> UpdateClass[Member[Literal["x"], int]]:
    return cls

def as_sets[T](cls: type[T]) -> UpdateClass[*[Member[m.name, set[m.type]] for m in Iter[Attrs[T]]]]:
    return cls

@as_sets
@add_x
class C:
    y: str

def _(c: C):
    reveal_type(c.x)  # revealed: set[int]
    reveal_type(c.y)  # revealed: set[str]
```

Decorators that do not return `UpdateClass` do not change the class:

```py
def register[T](cls: type[T]) -> type[T]:
    return cls

@register
class Plain:
    x: int

reveal_type(Plain)  # revealed: <class 'Plain'>
reveal_type(Plain().x)  # revealed: int
```

### Calling the decorator directly

Outside of a decorator position, the call returns an `UpdateClass` value. It has no effect on the
class.

```py
from typing import Literal
from ty_extensions import Member, UpdateClass

def with_id[T](cls: type[T]) -> UpdateClass[Member[Literal["id"], int]]:
    return cls

class Item: ...

reveal_type(with_id(Item))  # revealed: UpdateClass[Member[Literal["id"], int, Never, Never, Never]]
Item().id  # error: [unresolved-attribute]
```

### The function body may return anything

A function whose return type is `UpdateClass[...]` may return any value, including the class itself
or an implicit `None`.

```py
from typing import Literal
from ty_extensions import Member, UpdateClass

def returns_class[T](cls: type[T]) -> UpdateClass[Member[Literal["id"], int]]:
    return cls

def returns_nothing[T](cls: type[T]) -> UpdateClass[Member[Literal["id"], int]]:
    pass
```

### `RaiseError` in an update

An update is evaluated when the class is checked, not where the decorator is defined, so a
`RaiseError` (phase 3) that the update selects is reported on the decorator. The member that failed
is `Unknown`.

```py
from typing import Literal
from ty_extensions import Attrs, IsAssignable, Iter, Member, RaiseError, UpdateClass

type Checked[P] = RaiseError[Literal["bytes attributes are not allowed"], P] if IsAssignable[P, bytes] else P

def no_bytes[T](cls: type[T]) -> UpdateClass[*[Member[p.name, Checked[p.type]] for p in Iter[Attrs[T]]]]:
    return cls

@no_bytes  # error: [raised-type-error] "bytes attributes are not allowed: bytes"
class Blob:
    name: str
    data: bytes

@no_bytes
class Text:
    name: str

def _(blob: Blob, text: Text):
    reveal_type(blob.name)  # revealed: str
    reveal_type(blob.data)  # revealed: Unknown
    reveal_type(text.name)  # revealed: str
```

### Invalid uses

`UpdateClass` is only allowed in return annotations, and its arguments must be `Member`s.

```py
from typing import Literal
from ty_extensions import Member, UpdateClass

def bad_argument[T](cls: type[T]) -> UpdateClass[int]:  # error: [invalid-type-form]
    return cls

def bad_position(x: UpdateClass[Member[Literal["a"], int]]) -> None:  # error: [invalid-type-form]
    pass

y: UpdateClass[Member[Literal["a"], int]]  # error: [invalid-type-form]
```

## `UpdateClass` in `__init_subclass__`

When a class is defined, every ancestor whose `__init_subclass__` returns `UpdateClass[...]` updates
the new class ([PEP 827]). If `cls` is annotated as `type[T]`, `T` is the new class. The class that
defines `__init_subclass__` is not updated itself.

```toml
[environment]
python-version = "3.14"
```

### A model base class

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

### The defining class is not updated

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

### Order of application

#### Ancestors are applied from the most basic one

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

#### Inherited attributes are wrapped again

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

### Generic base classes

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

### Hooks that do not return `UpdateClass`

An ordinary `__init_subclass__` does not change the subclass.

```py
class Registry:
    def __init_subclass__(cls, **kwargs: object) -> None:
        pass

class Plugin(Registry):
    name: str

reveal_type(Plugin().name)  # revealed: str
```

### Cycles between classes

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

## Examples

### Dataclass-style `__init__`

[//]: # (playground: slug=dataclass-update-class, phase=5, ts="constructor taking { [K in keyof T]: T[K] }")

A class decorator whose return type is `UpdateClass[...]` adds members to the decorated class. This
one generates a keyword-only `__init__` from the attributes, like `@dataclass(kw_only=True)`, but
written in a library instead of being built into the type checker.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, Callable, Literal

if TYPE_CHECKING:
    from ty_extensions import Attrs, Iter, Member, Param, ParamKind, Params, UpdateClass

type KwParams[T] = tuple[*[Param[p.name, p.type, Literal[ParamKind.KEYWORD_ONLY], p.init] for p in Iter[Attrs[T]]]]
type InitFn[T] = Callable[Params[Param[Literal["self"], T], *KwParams[T]], None]

def dataclass_ish[T](cls: type[T]) -> UpdateClass[Member[Literal["__init__"], InitFn[T], Literal["ClassVar"]]]:
    return cls

@dataclass_ish
class Point:
    x: int
    y: int = 0

def demo():
    reveal_type(Point.__init__)  # revealed: (self: Point, *, x: int, y: int = 0) -> None
    Point(x=1)
    Point(1, x=1)  # error: [too-many-positional-arguments]
    Point(y=2)  # error: [missing-argument]
```

### Base classes that update their subclasses

[//]: # (playground: slug=model-init-subclass, phase=5, ts="class Hero extends Model<Hero>")

`__init_subclass__` can return `UpdateClass[...]` too. Then every subclass is updated when it is
defined, like a Pydantic model: `Hero` and `Sidekick` get an `__init__` without a decorator.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, Callable, Literal

if TYPE_CHECKING:
    from ty_extensions import Attrs, Iter, Member, Param, ParamKind, Params, UpdateClass

type KwParams[T] = tuple[*[Param[p.name, p.type, Literal[ParamKind.KEYWORD_ONLY], p.init] for p in Iter[Attrs[T]]]]
type InitFn[T] = Callable[Params[Param[Literal["self"], T], *KwParams[T]], None]

class Model:
    def __init_subclass__[T](cls: type[T]) -> UpdateClass[Member[Literal["__init__"], InitFn[T], Literal["ClassVar"]]]:
        pass

class Hero(Model):
    name: str
    age: int | None = None

class Sidekick(Hero):
    power: str

def demo():
    reveal_type(Sidekick.__init__)  # revealed: (self: Sidekick, *, name: str, age: int | None = None, power: str) -> None
    Hero(name="Deadpond")
    Hero(name="Deadpond", power="none")  # error: [unknown-argument]
```

## Additional cases (not in the drafts)

```toml
[environment]
python-version = "3.14"
```

### A cycle evaluated from the other class first

The result does not depend on the order in which the classes are checked.

```py
from ty_extensions import Attrs, IsAssignable, Iter, UpdateClass

type Lazy[T, C] = C if IsAssignable[T, T] else C

def copy_from_q[T](cls: type[T]) -> UpdateClass[*[m for m in Iter[Attrs[Lazy[T, Q]]]]]:
    return cls

def copy_from_p[T](cls: type[T]) -> UpdateClass[*[m for m in Iter[Attrs[Lazy[T, P]]]]]:
    return cls

def _(q: "Q", p: "P"):
    q.a  # error: [unresolved-attribute]
    reveal_type(q.b)  # revealed: str
    reveal_type(p.a)  # revealed: int

@copy_from_p  # error: [invalid-type-form] "UpdateClass evaluation depends on itself"
class Q:
    b: str

@copy_from_q  # error: [invalid-type-form] "UpdateClass evaluation depends on itself"
class P:
    a: int
```

### A cycle through `__init_subclass__`

```py
from ty_extensions import Attrs, IsAssignable, Iter, UpdateClass

type Lazy[T, C] = C if IsAssignable[T, T] else C

class CopiesB:
    def __init_subclass__[T](cls: type[T]) -> UpdateClass[*[m for m in Iter[Attrs[Lazy[T, B]]]]]:
        pass

class CopiesA:
    def __init_subclass__[T](cls: type[T]) -> UpdateClass[*[m for m in Iter[Attrs[Lazy[T, A]]]]]:
        pass

class A(CopiesB):  # error: [invalid-type-form] "UpdateClass evaluation depends on itself"
    a: int

class B(CopiesA):  # error: [invalid-type-form] "UpdateClass evaluation depends on itself"
    b: str

def _(a: A, b: B):
    reveal_type(a.a)  # revealed: int
    reveal_type(b.b)  # revealed: str
```

### Generic classes

An update is evaluated for the class with its own type parameters, and specialized with the class.
The update sees a type parameter as a fixed type (bounded by its upper bound), so a condition on
it is decided for every specialization at once.

```py
from typing import Callable, Literal
from ty_extensions import Attrs, IsAssignable, Iter, Member, Param, ParamKind, Params, UpdateClass

def listed[T](cls: type[T]) -> UpdateClass[*[Member[m.name, list[m.type]] for m in Iter[Attrs[T]]]]:
    return cls

@listed
class Box[U]:
    item: U

def only_ints[T](cls: type[T]) -> UpdateClass[*[Member[m.name, list[m.type]] for m in Iter[Attrs[T]] if IsAssignable[m.type, int]]]:
    return cls

@only_ints
class Cond[U]:
    x: U
    y: int

def _(box: Box[int], cond: Cond[int]):
    reveal_type(box.item)  # revealed: list[int]
    reveal_type(cond.x)  # revealed: int
    reveal_type(cond.y)  # revealed: list[int]

type KwInit[T] = Member[
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

def kw_init[T](cls: type[T]) -> UpdateClass[KwInit[T]]:
    return cls

@kw_init
class Pair[A, B]:
    first: A
    second: B

reveal_type(Pair[int, str].__init__)  # revealed: (self: Pair[int, str], *, first: int, second: str) -> None
reveal_type(Pair(first=1, second="a"))  # revealed: Pair[int, str]
Pair[int, str](first=1, second="a")
Pair[int, str](first="a", second="a")  # error: [invalid-argument-type]
```

### The class seen by an update

`T` stands for the updated class: `GetSpecialAttr`, `GetArg` and `IsAssignable` see it as that
class, and it can be used in member types.

```py
from typing import Literal
from ty_extensions import (
    Attrs,
    GetArg,
    GetSpecialAttr,
    IsAssignable,
    Iter,
    Member,
    UpdateClass,
)

class Base[X]: ...

def describe[T](
    cls: type[T],
) -> UpdateClass[
    Member[Literal["name"], GetSpecialAttr[T, Literal["__name__"]]],
    Member[Literal["arg"], GetArg[T, Base, Literal[0]]],
    Member[Literal["is_base"], IsAssignable[T, Base[int]]],
    Member[Literal["parent"], T | None],
]:
    return cls

@describe
class Sub(Base[int]):
    value: int

type Names[T] = tuple[*[p.name for p in Iter[Attrs[T]]]]

def _(sub: Sub, names: Names[Sub]):
    reveal_type(sub.name)  # revealed: Literal["Sub"]
    reveal_type(sub.arg)  # revealed: int
    reveal_type(sub.is_base)  # revealed: Literal[True]
    reveal_type(sub.parent)  # revealed: Sub | None
    reveal_type(names)  # revealed: tuple[Literal["name"], Literal["arg"], Literal["is_base"], Literal["parent"], Literal["value"]]
```

### Methods

A `ClassVar` member with a callable type is a method: it binds `self` on instances, and it is not
an attribute.

```py
from typing import Callable, Literal
from ty_extensions import Attrs, Iter, Member, Members, Param, Params, UpdateClass

type Greet[T] = Member[
    Literal["greet"],
    Callable[Params[Param[Literal["self"], T], Param[Literal["name"], str]], str],
    Literal["ClassVar"],
]

def with_greet[T](cls: type[T]) -> UpdateClass[Greet[T]]:
    return cls

@with_greet
class Greeter:
    prefix: str

type Names[T] = tuple[*[p.name for p in Iter[Attrs[T]]]]
type MemberNames[T] = tuple[*[p.name for p in Iter[Members[T]]]]

def _(greeter: Greeter, names: Names[Greeter], member_names: MemberNames[Greeter]):
    reveal_type(greeter.greet("a"))  # revealed: str
    reveal_type(Greeter.greet)  # revealed: (self: Greeter, name: str) -> str
    reveal_type(names)  # revealed: tuple[Literal["prefix"]]
    reveal_type(member_names)  # revealed: tuple[Literal["prefix"], Literal["greet"]]
```

### Overriding a synthesized `__init__`

An update also replaces members that ty synthesizes, like the `__init__` of a dataclass.

```py
from dataclasses import dataclass
from typing import Callable, Literal
from ty_extensions import Member, Param, Params, UpdateClass

def no_init[T](
    cls: type[T],
) -> UpdateClass[Member[Literal["__init__"], Callable[Params[Param[Literal["self"], T]], None], Literal["ClassVar"]]]:
    return cls

@no_init
@dataclass
class Data:
    x: int

reveal_type(Data.__init__)  # revealed: (self: Data) -> None
Data()
Data(x=1)  # error: [unknown-argument]
```

### Removed members on the class

```py
from typing import Literal, Never
from ty_extensions import Member, UpdateClass

def drop_x[T](cls: type[T]) -> UpdateClass[Member[Literal["x"], Never]]:
    return cls

class Base:
    x: int = 1

@drop_x
class Child(Base): ...

reveal_type(Child.x)  # revealed: Never
```

### The signature of an update decorator

```py
from typing import Literal
from ty_extensions import Attrs, Iter, Member, UpdateClass

def with_id[T](cls: type[T]) -> UpdateClass[Member[Literal["id"], int]]:
    return cls

def as_lists[T](cls: type[T]) -> UpdateClass[*[Member[m.name, list[m.type]] for m in Iter[Attrs[T]]]]:
    return cls

reveal_type(with_id)  # revealed: def with_id[T](cls: type[T]) -> UpdateClass[Member[Literal["id"], int, Never, Never, Never]]
reveal_type(as_lists)  # revealed: def as_lists[T](cls: type[T]) -> UpdateClass[*[Member[m.name, list[m.type], Never, Never, Never] for m in Iter[Attrs[T]]]]
```

### More invalid uses

`UpdateClass` must be the whole return annotation.

```py
from typing import Literal
from ty_extensions import Iter, Member, UpdateClass

def nested[T](cls: type[T]) -> list[UpdateClass[Member[Literal["a"], int]]]:  # error: [invalid-type-form]
    return []

type Alias = UpdateClass[Member[Literal["a"], int]]  # error: [invalid-type-form]

def bad_name[T](cls: type[T]) -> UpdateClass[Member[str, int]]:  # error: [invalid-type-form]
    return cls

def unbounded[T](cls: type[T]) -> UpdateClass[*tuple[Member[Literal["a"], int], ...]]:  # error: [invalid-type-form]
    return cls
```

[PEP 827]: https://peps.python.org/pep-0827/

