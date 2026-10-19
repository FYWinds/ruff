# Slicing string literal types

`Slice[S, Start, End]` from [PEP 827] slices a tuple type. Phase 4 extends it to string literal
types: `Slice[Literal["abcd"], Literal[1], None]` is `Literal["bcd"]`, the type of
`"abcd"[1:]`. `Start` and `End` are integer literal types or `None`.

```toml
[environment]
python-version = "3.14"
```

## Basic slices

```py
from typing import Literal
from ty_extensions import Slice

def _(
    a: Slice[Literal["abcd"], Literal[0], Literal[1]],
    b: Slice[Literal["abcd"], Literal[1], None],
    c: Slice[Literal["abcd"], None, Literal[2]],
    d: Slice[Literal["abcd"], None, None],
    e: Slice[Literal["abcd"], Literal[1], Literal[None]],
):
    reveal_type(a)  # revealed: Literal["a"]
    reveal_type(b)  # revealed: Literal["bcd"]
    reveal_type(c)  # revealed: Literal["ab"]
    reveal_type(d)  # revealed: Literal["abcd"]
    reveal_type(e)  # revealed: Literal["bcd"]
```

## Python slicing rules

Negative indices count from the end. Out-of-range indices are clamped, and a slice whose start is
after its end is empty. Boolean literals are integers (`True` is `1`).

```py
from typing import Literal
from ty_extensions import Slice

def _(
    a: Slice[Literal["hello"], Literal[1], Literal[-1]],
    b: Slice[Literal["hello"], Literal[-3], None],
    c: Slice[Literal["hello"], Literal[10], None],
    d: Slice[Literal["hello"], Literal[-100], Literal[2]],
    e: Slice[Literal["hello"], Literal[3], Literal[1]],
    f: Slice[Literal["hello"], Literal[True], None],
    g: Slice[Literal[""], Literal[0], Literal[1]],
):
    reveal_type(a)  # revealed: Literal["ell"]
    reveal_type(b)  # revealed: Literal["llo"]
    reveal_type(c)  # revealed: Literal[""]
    reveal_type(d)  # revealed: Literal["he"]
    reveal_type(e)  # revealed: Literal[""]
    reveal_type(f)  # revealed: Literal["ello"]
    reveal_type(g)  # revealed: Literal[""]
```

## Code points

Strings are sliced by code point, like Python's `str`, not by byte.

```py
from typing import Literal
from ty_extensions import Slice

def _(
    a: Slice[Literal["héllo"], Literal[1], Literal[3]],
    b: Slice[Literal["Ωmega"], Literal[0], Literal[1]],
):
    reveal_type(a)  # revealed: Literal["él"]
    reveal_type(b)  # revealed: Literal["Ω"]
```

## Lifting over unions

`Slice` is lifted over unions in all three arguments.

```py
from typing import Literal
from ty_extensions import Slice

def _(
    a: Slice[Literal["abc", "xyz"], Literal[0], Literal[1]],
    b: Slice[Literal["abc", "xyz"], Literal[0], Literal[1, 2]],
    c: Slice[Literal["abc"], Literal[0, 1], None],
):
    reveal_type(a)  # revealed: Literal["a", "x"]
    reveal_type(b)  # revealed: Literal["a", "ab", "x", "xy"]
    reveal_type(c)  # revealed: Literal["abc", "bc"]
```

A union can mix tuples and strings; each member is sliced according to its kind.

```py
from typing import Literal
from ty_extensions import Slice

def _(a: Slice[Literal["ab"] | tuple[int, str], Literal[0], Literal[1]]):
    reveal_type(a)  # revealed: Literal["a"] | tuple[int]
```

## Non-literal strings

As for the other string operators, `LiteralString` gives `LiteralString`, `str` gives `str` and
`Any` gives `Any`.

```py
from typing import Any, Literal, LiteralString
from ty_extensions import Slice

def _(
    a: Slice[LiteralString, Literal[1], None],
    b: Slice[str, Literal[1], None],
    c: Slice[Any, Literal[1], None],
):
    reveal_type(a)  # revealed: LiteralString
    reveal_type(b)  # revealed: str
    reveal_type(c)  # revealed: Any
```

## Combined with other string operators

`Slice` and `Concat` are enough to split and reassemble strings. `Capitalize` from TypeScript can be
written this way, without lowercasing the rest of the string:

```py
from typing import Literal
from ty_extensions import Concat, Slice, Uppercase

type Head[S] = Slice[S, Literal[0], Literal[1]]
type Tail[S] = Slice[S, Literal[1], None]
type TsCapitalize[S] = Concat[Uppercase[Head[S]], Tail[S]]

def _(a: TsCapitalize[Literal["fooBar"]]):
    reveal_type(a)  # revealed: Literal["FooBar"]
```

Lifting happens separately for every operator, so the two halves of a union member are not kept
together: with a union argument, `Head[S]` and `Tail[S]` are each a union, and `Concat` combines every
head with every tail.

```py
def _(a: TsCapitalize[Literal["name", "userId"]]):
    reveal_type(a)  # revealed: Literal["Name", "NserId", "Uame", "UserId"]
```

To apply such an alias to each member of a union separately, iterate over the members explicitly:

```py
from typing import Union
from ty_extensions import FromUnion, Iter

type EachTsCapitalize[S] = Union[*[TsCapitalize[s] for s in Iter[FromUnion[S]]]]

def _(a: EachTsCapitalize[Literal["name", "userId"]]):
    reveal_type(a)  # revealed: Literal["Name", "UserId"]
```

[PEP 827]: https://peps.python.org/pep-0827/
