# `Attrs` and `Members`

`Attrs[T]` is a tuple of `Member`s, one for each attribute of the class `T`. `Members[T]` also
includes the methods, after all attributes. Only explicitly annotated attributes count, and only
methods with at least one annotation (on a parameter or the return type).

Most tests below reveal the full tuple. When a member's type is a callable, the tests read the
components with value-level property access on an element of the tuple instead (`m[0].name`),
because the exact display of method callables is not what is being tested.

```toml
[environment]
python-version = "3.14"
```

## Basic attributes

`Init` is the type ty infers for the initializer in the class body, with literal types preserved.
Without an initializer it is `Never`. `D` is the class that defines the attribute.

```py
from ty_extensions import Attrs, Members

class A:
    x: int
    y: str = "a"
    z: int | None = None

def _(a: Attrs[A], m: Members[A]):
    # revealed: tuple[Member[Literal["x"], int, Never, Never, A], Member[Literal["y"], str, Never, Literal["a"], A], Member[Literal["z"], int | None, Never, None, A]]
    reveal_type(a)
    # revealed: tuple[Member[Literal["x"], int, Never, Never, A], Member[Literal["y"], str, Never, Literal["a"], A], Member[Literal["z"], int | None, Never, None, A]]
    reveal_type(m)
```

## Empty classes

Classes from `builtins` (including `object`) contribute no members, so a class without annotated
members has none.

```py
from ty_extensions import Attrs, Members

class Empty: ...

def _(a: Attrs[Empty], m: Members[Empty]):
    reveal_type(a)  # revealed: tuple[()]
    reveal_type(m)  # revealed: tuple[()]
```

The same holds for subclasses of other builtin classes: the methods of `int` are not members of
`Meters`.

```py
from ty_extensions import Attrs, Members

class Meters(int):
    unit: str = "m"

def _(a: Attrs[Meters], m: Members[Meters]):
    reveal_type(a)  # revealed: tuple[Member[Literal["unit"], str, Never, Literal["m"], Meters]]
    reveal_type(m)  # revealed: tuple[Member[Literal["unit"], str, Never, Literal["m"], Meters]]
```

## What is not a member

Unannotated class-level assignments, implicit instance attributes assigned in methods, and methods
without any annotation are not members.

```py
from ty_extensions import Members

class A:
    x: int
    k = 1

    def __init__(self):
        self.w = 1
        self.v: int = 2

    def untyped(self):
        pass

def _(m: Members[A]):
    reveal_type(m)  # revealed: tuple[Member[Literal["x"], int, Never, Never, A]]
```

## Qualifiers

`ClassVar` and `Final` become qualifiers. A bare `Final` or `ClassVar` uses the declared type that ty
infers for the attribute, as in `A.c` and `A.d`.

```py
from typing import ClassVar, Final
from ty_extensions import Attrs

class A:
    a: ClassVar[int] = 1
    b: Final[int] = 2
    c: Final = 3

def _(attrs: Attrs[A]):
    # revealed: tuple[Member[Literal["a"], int, Literal["ClassVar"], Literal[1], A], Member[Literal["b"], int, Literal["Final"], Literal[2], A], Member[Literal["c"], Literal[3], Literal["Final"], Literal[3], A]]
    reveal_type(attrs)
    reveal_type(A.c)  # revealed: Literal[3]
```

```py
from typing import ClassVar
from ty_extensions import Attrs

class B:
    d: ClassVar = 4

def _(attrs: Attrs[B]):
    reveal_type(B.d)  # revealed: Unknown | Literal[4]
    # revealed: tuple[Member[Literal["d"], Unknown | Literal[4], Literal["ClassVar"], Literal[4], B]]
    reveal_type(attrs)
```

`Annotated` metadata is not displayed, but it can be read with `GetAnnotations`.

```py
from typing import Annotated
from ty_extensions import Attrs

class C:
    x: Annotated[int, "meta"]

def _(attrs: Attrs[C]):
    reveal_type(attrs)  # revealed: tuple[Member[Literal["x"], int, Never, Never, C]]
```

## Callable attributes

An attribute whose type is callable is still an attribute, unless it is also a `ClassVar`: a
`ClassVar` with a callable type is treated like a method, so it is excluded from `Attrs` but present
in `Members`.

```py
from typing import Callable, ClassVar
from ty_extensions import Attrs, Members

class Handlers:
    on_event: Callable[[int], str]
    factory: ClassVar[Callable[[], int]]

def _(a: Attrs[Handlers], m: Members[Handlers]):
    reveal_type(len(a))  # revealed: Literal[1]
    reveal_type(a[0].name)  # revealed: Literal["on_event"]
    reveal_type(a[0].type(1))  # revealed: str

    reveal_type(len(m))  # revealed: Literal[2]
    reveal_type(m[1].name)  # revealed: Literal["factory"]
    reveal_type(m[1].quals)  # revealed: Literal["ClassVar"]
```

## Properties

A property is an attribute. Its type is the return type of the getter.

```py
from ty_extensions import Attrs, Members

class P:
    x: int

    @property
    def double(self) -> int:
        return self.x * 2

def _(a: Attrs[P], m: Members[P]):
    # revealed: tuple[Member[Literal["x"], int, Never, Never, P], Member[Literal["double"], int, Never, Never, P]]
    reveal_type(a)
    # revealed: tuple[Member[Literal["x"], int, Never, Never, P], Member[Literal["double"], int, Never, Never, P]]
    reveal_type(m)
```

## Order and overrides

Members are listed starting from the most basic class (the reverse of the MRO). When a subclass
redeclares a member, the member keeps its original position, but its type, initializer and definer
come from the subclass.

```py
from ty_extensions import Attrs

class Base:
    a: int
    b: str

    @property
    def size(self) -> int:
        return 0

class Child(Base):
    c: bytes
    a: int = 1

    @property
    def size(self) -> bool:
        return True

def _(attrs: Attrs[Child]):
    # revealed: tuple[Member[Literal["a"], int, Never, Literal[1], Child], Member[Literal["b"], str, Never, Never, Base], Member[Literal["size"], bool, Never, Never, Child], Member[Literal["c"], bytes, Never, Never, Child]]
    reveal_type(attrs)
```

With multiple inheritance, the order follows the reversed MRO:

```py
from ty_extensions import Attrs

class Left:
    left: int

class Right:
    right: str

class Both(Left, Right):
    both: bytes

def _(attrs: Attrs[Both]):
    # revealed: tuple[Member[Literal["right"], str, Never, Never, Right], Member[Literal["left"], int, Never, Never, Left], Member[Literal["both"], bytes, Never, Never, Both]]
    reveal_type(attrs)
```

## Methods

`Members` lists all attributes first, then all methods, each group starting from the most basic
class. A method has the `ClassVar` qualifier and no initializer (`Init` is `Never`). Its type is the
callable type of the function, including the `self` parameter.

```py
from ty_extensions import Members

class Base:
    a: int

    def run(self) -> int:
        return 1

class Child(Base):
    b: str

    def stop(self) -> None:
        pass

    def run(self) -> int:
        return 2

    def half(self, x: int):
        return x // 2

    def untyped(self, x):
        return x

    @staticmethod
    def make() -> int:
        return 1

    @classmethod
    def build(cls) -> int:
        return 1

def _(m: Members[Child]):
    reveal_type(len(m))  # revealed: Literal[7]
    reveal_type(m[0].name)  # revealed: Literal["a"]
    reveal_type(m[1].name)  # revealed: Literal["b"]

    # `run` is defined on `Base` first, so it comes before the methods of `Child`. The override
    # updates its definer.
    reveal_type(m[2].name)  # revealed: Literal["run"]
    reveal_type(m[2].quals)  # revealed: Literal["ClassVar"]
    reveal_type(m[2].definer)  # revealed: Child
    reveal_type(m[2].type(Child()))  # revealed: int

    reveal_type(m[3].name)  # revealed: Literal["stop"]
    reveal_type(m[4].name)  # revealed: Literal["half"]
    reveal_type(m[5].name)  # revealed: Literal["make"]
    reveal_type(m[5].quals)  # revealed: Literal["ClassVar"]
    reveal_type(m[6].name)  # revealed: Literal["build"]
    reveal_type(m[6].quals)  # revealed: Literal["ClassVar"]
    # `init` is `Never` for methods, so this reveal comes last: the rest of the function would be
    # unreachable after it.
    reveal_type(m[2].init)  # revealed: Never
```

`__init__` and private names are included. Names are not mangled.

```py
from ty_extensions import Members

class C:
    __secret: int
    _private: str

    def __init__(self, x: int) -> None:
        self.extra = x

def _(m: Members[C]):
    reveal_type(len(m))  # revealed: Literal[3]
    reveal_type(m[0].name)  # revealed: Literal["__secret"]
    reveal_type(m[1].name)  # revealed: Literal["_private"]
    reveal_type(m[2].name)  # revealed: Literal["__init__"]
```

## Method and attribute with the same name

When a method and an attribute share a name, the most derived definition wins and keeps the position
of the first one. Here the method `size` replaces the attribute `size`, so it is no longer an
attribute.

```toml
[environment]
python-version = "3.14"

[rules]
invalid-attribute-override = "ignore"
invalid-method-override = "ignore"
```

```py
from ty_extensions import Attrs, Members

class Base:
    size: int
    name: str

class Child(Base):
    def size(self) -> int:
        return 0

def _(a: Attrs[Child], m: Members[Child]):
    reveal_type(a)  # revealed: tuple[Member[Literal["name"], str, Never, Never, Base]]
    reveal_type(len(m))  # revealed: Literal[2]
    reveal_type(m[0].name)  # revealed: Literal["size"]
    reveal_type(m[0].quals)  # revealed: Literal["ClassVar"]
    reveal_type(m[0].definer)  # revealed: Child
    reveal_type(m[1].name)  # revealed: Literal["name"]
```

## Generic classes

Members of a specialized generic class are specialized as well, and the definer is the specialized
base class.

```py
from ty_extensions import Attrs

class Box[T]:
    item: T
    items: list[T]

class IntBox(Box[int]):
    label: str

def _(a: Attrs[Box[int]], b: Attrs[IntBox]):
    # revealed: tuple[Member[Literal["item"], int, Never, Never, Box[int]], Member[Literal["items"], list[int], Never, Never, Box[int]]]
    reveal_type(a)
    # revealed: tuple[Member[Literal["item"], int, Never, Never, Box[int]], Member[Literal["items"], list[int], Never, Never, Box[int]], Member[Literal["label"], str, Never, Never, IntBox]]
    reveal_type(b)
```

## Generic methods

The type parameters of a generic method are bound by the method's own signature, so they do not
defer the evaluation of `Members`.

```py
from typing import Literal
from ty_extensions import GetMemberType, Members

class C:
    x: int

    def ident[U](self, value: U) -> U:
        return value

def _(m: Members[C], ident: GetMemberType[C, Literal["ident"]]):
    reveal_type(len(m))  # revealed: Literal[2]
    reveal_type(m[1].name)  # revealed: Literal["ident"]
    reveal_type(ident(C(), 1))  # revealed: Literal[1]
```

## Dataclasses

Dataclass fields are ordinary annotated attributes. Methods that the dataclass machinery synthesizes
(`__init__`, `__eq__`, ...) are not written in the class body, so they are not members.

```py
from dataclasses import dataclass
from ty_extensions import Members

@dataclass
class Point:
    x: int
    y: int = 0

def _(m: Members[Point]):
    # revealed: tuple[Member[Literal["x"], int, Never, Never, Point], Member[Literal["y"], int, Never, Literal[0], Point]]
    reveal_type(m)
```

## TypedDicts

For a `TypedDict`, `NotRequired` and `ReadOnly` become qualifiers, the definer is always `Never`, and
there are no methods, so `Members` is the same as `Attrs`.

```py
from typing import NotRequired, ReadOnly, TypedDict
from ty_extensions import Attrs, IsEquivalent, Members

class Movie(TypedDict):
    name: str
    year: NotRequired[int]
    id: ReadOnly[int]
    tag: ReadOnly[NotRequired[str]]

def _(a: Attrs[Movie], same: IsEquivalent[Members[Movie], Attrs[Movie]]):
    # revealed: tuple[Member[Literal["name"], str, Never, Never, Never], Member[Literal["year"], int, Literal["NotRequired"], Never, Never], Member[Literal["id"], int, Literal["ReadOnly"], Never, Never], Member[Literal["tag"], str, Literal["NotRequired", "ReadOnly"], Never, Never]]
    reveal_type(a)
    reveal_type(same)  # revealed: Literal[True]
```

Keys of a `total=False` TypedDict are `NotRequired` unless marked `Required`. This also applies to
keys inherited from a `total=False` base. `Required` is not a qualifier.

```py
from typing import Required, TypedDict
from ty_extensions import Attrs

class OptionalTD(TypedDict, total=False):
    x: int
    y: Required[str]

class ChildTD(OptionalTD):
    z: bool

def _(a: Attrs[OptionalTD], b: Attrs[ChildTD]):
    # revealed: tuple[Member[Literal["x"], int, Literal["NotRequired"], Never, Never], Member[Literal["y"], str, Never, Never, Never]]
    reveal_type(a)
    # revealed: tuple[Member[Literal["x"], int, Literal["NotRequired"], Never, Never], Member[Literal["y"], str, Never, Never, Never], Member[Literal["z"], bool, Never, Never, Never]]
    reveal_type(b)
```

## `Any`

`Attrs[Any]` and `Members[Any]` are `Any`.

```py
from typing import Any
from ty_extensions import Attrs, Members

def _(a: Attrs[Any], m: Members[Any]):
    reveal_type(a)  # revealed: Any
    reveal_type(m)  # revealed: Any
```

## Deferred evaluation

With a type variable as the argument, `Attrs` and `Members` are evaluated once the type variable is
specialized.

```py
from ty_extensions import Attrs, Members

class A:
    x: int

type AttrsOf[T] = Attrs[T]

def f[T](x: T, a: Attrs[T], m: Members[T]):
    reveal_type(a)  # revealed: Attrs[T@f]
    reveal_type(m)  # revealed: Members[T@f]

def attrs_of[T](x: T) -> Attrs[T]:
    raise NotImplementedError

def _(a: AttrsOf[A]):
    reveal_type(a)  # revealed: tuple[Member[Literal["x"], int, Never, Never, A]]
    reveal_type(attrs_of(A()))  # revealed: tuple[Member[Literal["x"], int, Never, Never, A]]
```
