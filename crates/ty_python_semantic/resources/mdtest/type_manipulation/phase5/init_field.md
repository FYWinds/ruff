# `InitField`

[PEP 827] models dataclass-style field descriptors (`x: int = Field(default=0)`) with
`InitField[KwargDict: BaseTypedDict]`, whose constructor is
`__init__(self, **kwargs: Unpack[KwargDict])`.

Because of the `**kwargs: Unpack[K]` inference rules, calling `InitField` or a subclass infers a
`TypedDict` with the literal types of the keyword arguments. In a class body, that type becomes the
`Init` of the attribute's `Member`, so type-level code can read the arguments of the field.

```toml
[environment]
python-version = "3.14"
```

## Inferred type of a field

```py
from typing import ReadOnly, TypedDict
from ty_extensions import InitField

class FieldArgs(TypedDict, total=False):
    default: ReadOnly[object]
    primary_key: ReadOnly[bool]
    hidden: ReadOnly[bool]

class Field[T: FieldArgs](InitField[T]): ...

def _():
    f = Field(default=0, primary_key=True)
    kwargs = f.get_kwargs()
    reveal_type(kwargs["default"])  # revealed: Literal[0]
    reveal_type(kwargs["primary_key"])  # revealed: Literal[True]

    Field(primary_key="yes")  # error: [invalid-argument-type]

    # `FieldArgs` is not closed, so other keyword arguments are allowed and keep their literal type.
    reveal_type(Field(index=True).get_kwargs()["index"])  # revealed: Literal[True]

    # Read-only items of the bound that are not passed are `Never`. This is revealed last: the code
    # after it is unreachable.
    reveal_type(kwargs["hidden"])  # revealed: Never
```

`get_kwargs()` is declared on `InitField` in the `ty_extensions` stub; it returns the inferred
`KwargDict`.

## Fields in a class body

In a class body, a field descriptor can be assigned to an attribute of any declared type. The
attribute still has its declared type on instances, while its `Member` records the field as `Init`.

```py
from typing import Literal, ReadOnly, TypedDict
from ty_extensions import GetArg, GetMember, GetMemberType, InitField, IsAssignable

class FieldArgs(TypedDict, total=False):
    default: ReadOnly[object]
    primary_key: ReadOnly[bool]

class Field[T: FieldArgs](InitField[T]): ...

class Hero:
    id: int | None = Field(default=None, primary_key=True)
    name: str = Field()
    age: int = 0

type GetFieldItem[I, K] = GetMemberType[GetArg[I, InitField, Literal[0]], K]
type Id = GetMember[Hero, Literal["id"]]
type Name = GetMember[Hero, Literal["name"]]
type Age = GetMember[Hero, Literal["age"]]

def _(
    hero: Hero,
    id_is_field: IsAssignable[Id.init, Field],
    age_is_field: IsAssignable[Age.init, Field],
    id_default: GetFieldItem[Id.init, Literal["default"]],
    id_primary_key: GetFieldItem[Id.init, Literal["primary_key"]],
    name_primary_key: GetFieldItem[Name.init, Literal["primary_key"]],
    age_init: Age.init,
):
    reveal_type(hero.id)  # revealed: int | None
    reveal_type(hero.name)  # revealed: str
    reveal_type(id_is_field)  # revealed: Literal[True]
    reveal_type(age_is_field)  # revealed: Literal[False]
    reveal_type(id_default)  # revealed: None
    reveal_type(id_primary_key)  # revealed: Literal[True]
    reveal_type(age_init)  # revealed: Literal[0]
    reveal_type(name_primary_key)  # revealed: Never
```

## Outside of a class body

Outside of a class body, the assignment is checked as usual:

```py
from typing import TypedDict
from ty_extensions import InitField

class Args(TypedDict, total=False): ...

class Field[T: Args](InitField[T]): ...

x: int = Field()  # error: [invalid-assignment]

def _():
    y: int = Field()  # error: [invalid-assignment]
```

## Filtering attributes by their fields

```py
from typing import Literal, ReadOnly, TypedDict, Union
from ty_extensions import Attrs, GetArg, GetMemberType, InitField, IsAssignable, Iter

class FieldArgs(TypedDict, total=False):
    primary_key: ReadOnly[bool]

class Field[T: FieldArgs](InitField[T]): ...

class Hero:
    id: int = Field(primary_key=True)
    name: str = Field()
    age: int = 0

type GetFieldItem[I, K] = GetMemberType[GetArg[I, InitField, Literal[0]], K]
type PrimaryKeys[T] = Union[
    *[
        p.name
        for p in Iter[Attrs[T]]
        if IsAssignable[p.init, InitField]
        and IsAssignable[Literal[True], GetFieldItem[p.init, Literal["primary_key"]]]
    ]
]

def _(keys: PrimaryKeys[Hero]):
    reveal_type(keys)  # revealed: Literal["id"]
```

## Nested fields

An `InitField` passed as an argument of another `InitField` keeps its own inferred type, so nested
field arguments can be read as well.

```py
from typing import ReadOnly, TypedDict
from ty_extensions import InitField

class LinkArgs(TypedDict, total=False):
    target: ReadOnly[str]

class Link[T: LinkArgs](InitField[T]): ...

class ColumnArgs(TypedDict, total=False):
    link: ReadOnly[object]

class Column[T: ColumnArgs](InitField[T]): ...

def _():
    column = Column(link=Link(target="Post"))
    link = column.get_kwargs()["link"]
    reveal_type(link.get_kwargs()["target"])  # revealed: Literal["Post"]
```

## `InitField` requires a `TypedDict` bound

```py
from ty_extensions import InitField

class Bad(InitField[int]): ...  # error: [invalid-type-arguments]
```

[PEP 827]: https://peps.python.org/pep-0827/
