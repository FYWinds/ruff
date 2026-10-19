# `UpdateClass` in class decorators

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

## A dataclass-like decorator

### Synthesizing `__init__`

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

### Replacing an `__init__` from the class body

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

## Adding and replacing attributes

### New and replaced attributes

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

### The definer of an updated member

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

## Removing members

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

## Several decorators

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

## Calling the decorator directly

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

## The function body may return anything

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

## `RaiseError` in an update

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

## Invalid uses

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

[PEP 827]: https://peps.python.org/pep-0827/
