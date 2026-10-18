# `FromUnion`

`FromUnion[T]` turns the union `T` into a tuple of its elements, so that they can be iterated over
with `Iter`. A type that is not a union becomes a one-element tuple, and `Never` (the empty union)
becomes the empty tuple.

```toml
[environment]
python-version = "3.14"
```

## Basic

The elements keep the order of ty's union.

```py
from typing import Literal
from ty_extensions import FromUnion

type IntOrStr = int | str

def _(
    a: FromUnion[int | str | bytes],
    b: FromUnion[bytes | int],
    c: FromUnion[int],
    d: FromUnion[int | None],
    e: FromUnion[IntOrStr],
):
    reveal_type(a)  # revealed: tuple[int, str, bytes]
    reveal_type(b)  # revealed: tuple[bytes, int]
    reveal_type(c)  # revealed: tuple[int]
    reveal_type(d)  # revealed: tuple[int, None]
    reveal_type(e)  # revealed: tuple[int, str]
```

## Literals

A `Literal` with several values is a union of single-value literals.

```py
from typing import Literal
from ty_extensions import FromUnion

def _(a: FromUnion[Literal["a", "b"]], b: FromUnion[Literal[1] | str]):
    reveal_type(a)  # revealed: tuple[Literal["a"], Literal["b"]]
    reveal_type(b)  # revealed: tuple[Literal[1], str]
```

## Simplified unions

`FromUnion` sees the union after ty has simplified it. `bool` is a subtype of `int`, so `int | bool`
is just `int`. (The reference implementation keeps `bool` as a separate element here.)

```py
from typing import Literal
from ty_extensions import FromUnion

def _(a: FromUnion[int | bool], b: FromUnion[str | Literal["a"]]):
    reveal_type(a)  # revealed: tuple[int]
    reveal_type(b)  # revealed: tuple[str]
```

## `Never` and `Any`

```py
from typing import Any, Never
from ty_extensions import FromUnion

def _(a: FromUnion[Never], b: FromUnion[Any], c: FromUnion[int | Any]):
    reveal_type(a)  # revealed: tuple[()]
    reveal_type(b)  # revealed: tuple[Any]
    reveal_type(c)  # revealed: tuple[int, Any]
```

## Not lifted over unions

`FromUnion` takes the union as a whole. It is never applied element by element (that would always
produce a union of one-element tuples).

```py
from ty_extensions import FromUnion, IsEquivalent

def _(a: IsEquivalent[FromUnion[int | str], tuple[int] | tuple[str]]):
    reveal_type(a)  # revealed: Literal[False]
```

## Deferred evaluation

```py
from ty_extensions import FromUnion

type Parts[T] = FromUnion[T]

def f[T](x: T, parts: FromUnion[T]):
    reveal_type(parts)  # revealed: FromUnion[T@f]

def _(a: Parts[int | str], b: Parts[bytes]):
    reveal_type(a)  # revealed: tuple[int, str]
    reveal_type(b)  # revealed: tuple[bytes]
```
