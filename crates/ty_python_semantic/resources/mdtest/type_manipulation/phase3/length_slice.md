# `Length` and `Slice`

`Length[T]` is the length of the tuple type `T` as an `int` literal type, or `Literal[None]` if the
tuple has an unbounded part. `Slice[S, Start, End]` slices the tuple type `S` like a Python slice
`s[start:end]`. Both are lifted over unions.

```toml
[environment]
python-version = "3.14"
```

## `Length`

Subclasses of `tuple`, such as `NamedTuple`s, have a length too. `Literal[None]` is displayed as
`None`.

```py
from typing import Literal, NamedTuple
from ty_extensions import IsAssignable, Length

class Point(NamedTuple):
    x: int
    y: int

def _(
    a: Length[tuple[int, str]],
    b: Length[tuple[()]],
    c: Length[tuple[int, ...]],
    d: Length[tuple[int, *tuple[str, ...], bytes]],
    e: Length[Point],
    f: IsAssignable[Length[tuple[int, ...]], Literal[None]],
):
    reveal_type(a)  # revealed: Literal[2]
    reveal_type(b)  # revealed: Literal[0]
    reveal_type(c)  # revealed: None
    reveal_type(d)  # revealed: None
    reveal_type(e)  # revealed: Literal[2]
    reveal_type(f)  # revealed: Literal[True]
```

## `Length` of other types

```py
from typing import Any, Never
from ty_extensions import Length

def _(
    a: Length[int],  # error: [invalid-type-form] "`Length` requires a tuple type, but got `int`"
    b: Length[list[int]],  # error: [invalid-type-form] "`Length` requires a tuple type, but got `list[int]`"
    c: Length[Any],
    d: Length[Never],
):
    reveal_type(a)  # revealed: Unknown
    reveal_type(b)  # revealed: Unknown
    reveal_type(c)  # revealed: Any
    reveal_type(d)  # revealed: Never
```

## `Length` is lifted

```py
from ty_extensions import Length

def _(
    a: Length[tuple[int] | tuple[int, str]],
    b: Length[tuple[int] | tuple[str, ...]],
):
    reveal_type(a)  # revealed: Literal[1, 2]
    reveal_type(b)  # revealed: Literal[1] | None
```

## `Slice`

`Start` and `End` are `int` literal types or `None` (also written `Literal[None]`). Negative indexes
and out-of-range indexes behave as in Python.

```py
from typing import Literal
from ty_extensions import Slice

type T5 = tuple[Literal[0], Literal[1], Literal[2], Literal[3], Literal[4]]

def _(
    a: Slice[T5, Literal[1], Literal[3]],
    b: Slice[T5, Literal[1], None],
    c: Slice[T5, None, Literal[3]],
    d: Slice[T5, Literal[None], Literal[None]],
    e: Slice[T5, Literal[1], Literal[1]],
    f: Slice[T5, Literal[-2], None],
    g: Slice[T5, Literal[0], Literal[-1]],
    h: Slice[T5, Literal[10], None],
    i: Slice[tuple[()], Literal[0], Literal[-1]],
):
    reveal_type(a)  # revealed: tuple[Literal[1], Literal[2]]
    reveal_type(b)  # revealed: tuple[Literal[1], Literal[2], Literal[3], Literal[4]]
    reveal_type(c)  # revealed: tuple[Literal[0], Literal[1], Literal[2]]
    reveal_type(d)  # revealed: tuple[Literal[0], Literal[1], Literal[2], Literal[3], Literal[4]]
    reveal_type(e)  # revealed: tuple[()]
    reveal_type(f)  # revealed: tuple[Literal[3], Literal[4]]
    reveal_type(g)  # revealed: tuple[Literal[0], Literal[1], Literal[2], Literal[3]]
    reveal_type(h)  # revealed: tuple[()]
    reveal_type(i)  # revealed: tuple[()]
```

## Slicing tuples of unbounded length

The result is the type that slicing a value of that type gives. (PEP 827 does not specify this
case; the reference implementation slices the raw arguments of `tuple[...]`.)

```py
from typing import Literal
from ty_extensions import Slice

def _(
    a: Slice[tuple[int, *tuple[str, ...], bytes], Literal[1], None],
    b: Slice[tuple[int, ...], Literal[0], Literal[-1]],
    c: Slice[tuple[int, *tuple[str, ...], bytes], Literal[0], Literal[1]],
):
    reveal_type(a)  # revealed: tuple[*tuple[str, ...], bytes]
    reveal_type(b)  # revealed: tuple[int, ...]
    reveal_type(c)  # revealed: tuple[int]
```

## Subclasses of `tuple`

The result is a plain tuple.

```py
from typing import Literal, NamedTuple
from ty_extensions import Slice

class Point(NamedTuple):
    x: int
    y: str

def _(a: Slice[Point, Literal[1], None]):
    reveal_type(a)  # revealed: tuple[str]
```

## `Slice` is lifted

This is the example from the "Lifting over Unions" section of the PEP. The combinations are
evaluated in order, with the last argument varying fastest.

```py
from typing import Literal
from ty_extensions import Slice

class A: ...
class B: ...
class C: ...

def _(x: Slice[tuple[A, B, C], Literal[0, 1], Literal[2, 3]]):
    reveal_type(x)  # revealed: tuple[A, B] | tuple[A, B, C] | tuple[B] | tuple[B, C]
```

## Invalid slices

```py
from typing import Literal
from ty_extensions import Slice

def _(
    # error: [invalid-type-form] "The start index of `Slice` must be an `int` literal type or `None`, but got `int`"
    a: Slice[tuple[int, str], int, None],
    # error: [invalid-type-form] "The end index of `Slice` must be an `int` literal type or `None`"
    b: Slice[tuple[int, str], Literal[0], Literal["1"]],
    # error: [invalid-type-form] "`Slice` requires a tuple"
    c: Slice[list[int], Literal[0], None],
):
    reveal_type(a)  # revealed: Unknown
    reveal_type(c)  # revealed: Unknown
```

## `Any` and `Never`

```py
from typing import Any, Literal, Never
from ty_extensions import Slice

def _(a: Slice[Any, Literal[0], None], b: Slice[Never, Literal[0], None]):
    reveal_type(a)  # revealed: Any
    reveal_type(b)  # revealed: Never
```

## Variance

The tuple argument of `Slice` is covariant: a slice of a subtype is a subtype of the slice. The
indexes are invariant, like the arguments of other operators.

```py
from typing import Literal
from ty_extensions import IsAssignable, Slice

class Rest[T]:
    def get(self) -> Slice[T, Literal[1], None]:
        raise NotImplementedError

def _(
    a: IsAssignable[Rest[tuple[int, bool]], Rest[tuple[int, int]]],
    b: IsAssignable[Rest[tuple[int, int]], Rest[tuple[int, bool]]],
):
    reveal_type(a)  # revealed: Literal[True]
    reveal_type(b)  # revealed: Literal[False]
```

## Deferred evaluation

```py
from typing import Literal
from ty_extensions import Length, Slice

type DropLast[T] = Slice[T, Literal[0], Literal[-1]]

def f[T](x: T, n: Length[T], rest: Slice[T, Literal[1], None]):
    reveal_type(n)  # revealed: Length[T@f]
    reveal_type(rest)  # revealed: Slice[T@f, Literal[1], None]

def size[T](x: T) -> Length[T]:
    raise NotImplementedError

def _(a: DropLast[tuple[int, str, bytes]], b: DropLast[tuple[()]], pair: tuple[int, str]):
    reveal_type(a)  # revealed: tuple[int, str]
    reveal_type(b)  # revealed: tuple[()]
    reveal_type(size(pair))  # revealed: Literal[2]
```

[PEP 827]: https://peps.python.org/pep-0827/
