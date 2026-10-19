# `UpdateClass` edge cases

More cases for `UpdateClass` in class decorators and in `__init_subclass__`.

```toml
[environment]
python-version = "3.14"
```

## A cycle evaluated from the other class first

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

## A cycle through `__init_subclass__`

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

## Generic classes

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

## The class seen by an update

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

## Methods

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

## Overriding a synthesized `__init__`

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

## Removed members on the class

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

## The signature of an update decorator

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

## More invalid uses

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
