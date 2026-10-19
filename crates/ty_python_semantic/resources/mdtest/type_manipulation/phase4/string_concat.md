# String concatenation

`Concat[S1, S2]` from [PEP 827] concatenates two string literal types. It is the building block for
TypeScript-style template literal types: `` `get${K}` `` is written `Concat[Literal["get"], K]`.

```toml
[environment]
python-version = "3.14"
```

## Basic usage

```py
from typing import Literal
from ty_extensions import Concat

def _(
    a: Concat[Literal["foo"], Literal["bar"]],
    b: Concat[Literal["get_"], Literal["name"]],
    c: Concat[Literal["a b"], Literal[" c"]],
):
    reveal_type(a)  # revealed: Literal["foobar"]
    reveal_type(b)  # revealed: Literal["get_name"]
    reveal_type(c)  # revealed: Literal["a b c"]
```

## More than two parts

`Concat` takes exactly two arguments, like in the PEP. Templates with more parts are written as
nested `Concat`s; concatenation is associative, so the nesting does not matter.

```py
from typing import Literal
from ty_extensions import Concat

type Dotted[A, B, C] = Concat[A, Concat[Literal["."], Concat[B, Concat[Literal["."], C]]]]

def _(
    a: Concat[Concat[Literal["a"], Literal["b"]], Literal["c"]],
    b: Concat[Literal["a"], Concat[Literal["b"], Literal["c"]]],
    c: Dotted[Literal["os"], Literal["path"], Literal["join"]],
):
    reveal_type(a)  # revealed: Literal["abc"]
    reveal_type(b)  # revealed: Literal["abc"]
    reveal_type(c)  # revealed: Literal["os.path.join"]
```

## Empty strings

The empty string is the identity of `Concat`.

```py
from typing import Literal
from ty_extensions import Concat

def _(
    a: Concat[Literal[""], Literal["x"]],
    b: Concat[Literal["x"], Literal[""]],
    c: Concat[Literal[""], Literal[""]],
):
    reveal_type(a)  # revealed: Literal["x"]
    reveal_type(b)  # revealed: Literal["x"]
    reveal_type(c)  # revealed: Literal[""]
```

## Combined with case operators

```py
from typing import Literal
from ty_extensions import Capitalize, Concat, Uppercase

type EventHandler[K] = Concat[Literal["on"], Capitalize[K]]
type Constant[A, B] = Uppercase[Concat[A, Concat[Literal["_"], B]]]

def _(
    a: EventHandler[Literal["click"]],
    b: Constant[Literal["max"], Literal["size"]],
):
    reveal_type(a)  # revealed: Literal["onClick"]
    reveal_type(b)  # revealed: Literal["MAX_SIZE"]
```

## Unicode

Concatenation works on code points, and does not normalize the result.

```py
from typing import Literal
from ty_extensions import Concat

def _(a: Concat[Literal["caf"], Literal["é"]], b: Concat[Literal["Ω"], Literal["mega"]]):
    reveal_type(a)  # revealed: Literal["café"]
    reveal_type(b)  # revealed: Literal["Ωmega"]
```

## Very long results

Like the string concatenation `"a" + "b"` in value expressions, a result that is longer than 4096
bytes is not stored as a literal: it becomes `LiteralString`. This is not an error.

```py
from typing import LiteralString, Literal
from ty_extensions import Concat, IsEquivalent

type S64 = Literal["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"]
type S128 = Concat[S64, S64]
type S256 = Concat[S128, S128]
type S512 = Concat[S256, S256]
type S1024 = Concat[S512, S512]
type S2048 = Concat[S1024, S1024]
type S4096 = Concat[S2048, S2048]
type S8192 = Concat[S4096, S4096]

def _(
    exact: IsEquivalent[S4096, LiteralString],
    widened: IsEquivalent[S8192, LiteralString],
    x: S8192,
):
    # 4096 bytes is still within the limit, so `S4096` is a single literal.
    reveal_type(exact)  # revealed: Literal[False]
    reveal_type(widened)  # revealed: Literal[True]
    reveal_type(x)  # revealed: LiteralString
```

[PEP 827]: https://peps.python.org/pep-0827/
