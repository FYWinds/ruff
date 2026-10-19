# Inferring a `TypedDict` from `**kwargs`

[PEP 827] proposes allowing `**kwargs: Unpack[K]` where `K` is a type variable bounded by a
`TypedDict` (for example `BaseTypedDict`, an empty `TypedDict`). At a call site, the keyword arguments
that do not match another parameter are collected into a new `TypedDict`, which becomes the solution
for `K`. Literal types are kept where possible:

- an argument whose name is not in the bound, or is a `ReadOnly` item of the bound, keeps its literal
  type;
- an argument for a mutable item of the bound gets the item's declared type (mutable `TypedDict`
  items are invariant);
- a `ReadOnly` item of the bound that is not required and not passed gets the type `Never`.

This changes one existing behavior: ty used to reject `**kwargs: Unpack[T]` for a type variable `T`
(`typed_dict.md`).

```toml
[environment]
python-version = "3.14"
```

## Literal types are kept

```py
from typing import Unpack
from ty_extensions import BaseTypedDict

def capture[K: BaseTypedDict](**kwargs: Unpack[K]) -> K:
    reveal_type(kwargs)  # revealed: K@capture
    return kwargs

def _(n: int):
    td = capture(x=1, y="a", z=n, flag=True)
    reveal_type(td["x"])  # revealed: Literal[1]
    reveal_type(td["y"])  # revealed: Literal["a"]
    reveal_type(td["z"])  # revealed: int
    reveal_type(td["flag"])  # revealed: Literal[True]
    td["missing"]  # error: [invalid-key]
```

## No keyword arguments

No keyword arguments give an empty `TypedDict`:

```py
from typing import Unpack
from ty_extensions import Attrs, BaseTypedDict, Length

def capture[K: BaseTypedDict](**kwargs: Unpack[K]) -> Length[Attrs[K]]:
    raise NotImplementedError

reveal_type(capture())  # revealed: Literal[0]
reveal_type(capture(a=1, b=2))  # revealed: Literal[2]
```

## Items of the bound

```py
from typing import Literal, ReadOnly, TypedDict, Unpack
from ty_extensions import GetMemberType

class Options(TypedDict, total=False):
    verbose: ReadOnly[bool]
    level: int

def configure[K: Options](**kwargs: Unpack[K]) -> K:
    return kwargs

def verbose_of[K: Options](**kwargs: Unpack[K]) -> GetMemberType[K, Literal["verbose"]]:
    raise NotImplementedError

def _():
    r = configure(verbose=True, level=3, extra="x")
    reveal_type(r["verbose"])  # revealed: Literal[True]
    reveal_type(r["level"])  # revealed: int
    reveal_type(r["extra"])  # revealed: Literal["x"]

    reveal_type(verbose_of(verbose=False))  # revealed: Literal[False]
    # A read-only item that is not passed is `Never` (revealed last: the code after it is unreachable).
    reveal_type(verbose_of(level=1))  # revealed: Never
```

## The bound must be satisfied

```py
from typing import ReadOnly, TypedDict, Unpack

class Options(TypedDict, total=False):
    verbose: ReadOnly[bool]

def configure[K: Options](**kwargs: Unpack[K]) -> K:
    return kwargs

configure(verbose="yes")  # error: [invalid-argument-type]
```

## Mixed with other parameters

Keyword arguments that match a named parameter are not collected.

```py
from typing import Unpack
from ty_extensions import Attrs, BaseTypedDict, Iter

def call[K: BaseTypedDict](name: str, /, *, retries: int = 0, **kwargs: Unpack[K]) -> tuple[*[m.name for m in Iter[Attrs[K]]]]:
    raise NotImplementedError

reveal_type(call("f", retries=3, timeout=1.5, user="me"))  # revealed: tuple[Literal["timeout"], Literal["user"]]
```

## A `select`-style query

The motivating example from the PEP: the keyword arguments select attributes of a model class.

```py
from typing import Literal, Unpack
from ty_extensions import Attrs, BaseTypedDict, GetMemberType, Iter, Member, NewProtocol

class User:
    id: int
    name: str
    email: str

def select[ModelT, K: BaseTypedDict](
    typ: type[ModelT],
    /,
    **kwargs: Unpack[K],
) -> list[NewProtocol[*[Member[c.name, GetMemberType[ModelT, c.name]] for c in Iter[Attrs[K]]]]]:
    raise NotImplementedError

def _():
    rows = select(User, name=True, email=True)
    reveal_type(rows[0].name)  # revealed: str
    reveal_type(rows[0].email)  # revealed: str
    rows[0].id  # error: [unresolved-attribute]
```

## Type variables that are not bounded by a `TypedDict`

These are still rejected.

```py
from typing import Unpack

def f[T](**kwargs: Unpack[T]) -> None:  # error: [invalid-type-form]
    pass

def g[T: int](**kwargs: Unpack[T]) -> None:  # error: [invalid-type-form]
    pass
```

[PEP 827]: https://peps.python.org/pep-0827/
