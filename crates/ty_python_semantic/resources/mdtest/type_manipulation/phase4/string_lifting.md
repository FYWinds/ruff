# String operators over unions and non-literal strings

Like most [PEP 827] operators, the string operators are *lifted* over unions: every union argument
is split into its members, the operator is applied to every combination (the cross product), and
the results are joined into a union. This is the same as TypeScript's template literal types.

```toml
[environment]
python-version = "3.14"
```

## One union argument

```py
from typing import Literal
from ty_extensions import Concat, Uppercase

type Keys = Literal["x", "y"]

def _(
    a: Uppercase[Literal["foo", "bar"]],
    b: Concat[Literal["get_"], Literal["name", "age"]],
    c: Uppercase[Keys],
):
    reveal_type(a)  # revealed: Literal["FOO", "BAR"]
    reveal_type(b)  # revealed: Literal["get_name", "get_age"]
    reveal_type(c)  # revealed: Literal["X", "Y"]
```

## Cross product

With several union arguments, every combination is computed. Equal results are only included once.

```py
from typing import Literal
from ty_extensions import Concat, Lowercase

def _(
    a: Concat[Literal["a", "b"], Literal["c", "d"]],
    b: Concat[Literal["x", ""], Literal["", "x"]],
    c: Lowercase[Literal["a", "A"]],
):
    reveal_type(a)  # revealed: Literal["ac", "ad", "bc", "bd"]
    reveal_type(b)  # revealed: Literal["x", "xx", ""]
    reveal_type(c)  # revealed: Literal["a"]
```

## `Never`

`Never` is the empty union, so the cross product is empty and the result is `Never`.

```py
from typing import Literal, Never
from ty_extensions import Concat, Uppercase

def _(a: Uppercase[Never], b: Concat[Never, Literal["a"]], c: Concat[Literal["a"], Never]):
    reveal_type(a)  # revealed: Never
    reveal_type(b)  # revealed: Never
    reveal_type(c)  # revealed: Never
```

## Gradual types

If an argument is `Any`, the result is `Any`. In a union, only the combinations that involve `Any`
produce `Any`.

```py
from typing import Any, Literal
from ty_extensions import Concat, Uppercase

def _(
    a: Uppercase[Any],
    b: Concat[Literal["a"], Any],
    c: Uppercase[Literal["a"] | Any],
):
    reveal_type(a)  # revealed: Any
    reveal_type(b)  # revealed: Any
    reveal_type(c)  # revealed: Literal["A"] | Any
```

## `LiteralString`

`LiteralString` stands for some string literal that is not known statically. Applying a string
operator to it gives another unknown string literal, so the result is `LiteralString`.

```py
from typing import Literal, LiteralString
from ty_extensions import Capitalize, Concat, Uppercase

def _(
    a: Uppercase[LiteralString],
    b: Concat[LiteralString, Literal["!"]],
    c: Concat[Literal["a", "b"], LiteralString],
    d: Capitalize[LiteralString],
):
    reveal_type(a)  # revealed: LiteralString
    reveal_type(b)  # revealed: LiteralString
    reveal_type(c)  # revealed: LiteralString
    reveal_type(d)  # revealed: LiteralString
```

## Non-literal strings

PEP 827 only describes string operators on string literal types. When an argument is a `str` that
is not a literal, ty does not report an error: it widens the result to `str`, which is what the
value-level operations do as well (`s + "a"` is a `str` for `s: str`). Subclasses of `str` are
treated the same way.

The reference runtime evaluator rejects such arguments.

```py
from typing import Any, Literal, LiteralString
from ty_extensions import Concat, Uppercase

class MyStr(str): ...

def _(
    a: Uppercase[str],
    b: Concat[Literal["get_"], str],
    c: Concat[LiteralString, str],
    d: Uppercase[MyStr],
    e: Concat[str, Any],
):
    reveal_type(a)  # revealed: str
    reveal_type(b)  # revealed: str
    reveal_type(c)  # revealed: str
    reveal_type(d)  # revealed: str
    # A dynamic argument takes precedence.
    reveal_type(e)  # revealed: Any
```

The same applies to `str`-based enums. Both the enum type and its members are treated as
non-literal strings, so that a member (a subtype of the enum type) never gives a result that is
incompatible with the result for the enum type. (The reference runtime evaluator computes with the
member's value instead: `Uppercase[Literal[Color.RED]]` is `Literal["RED"]` there.)

```py
from enum import StrEnum
from typing import Literal
from ty_extensions import Concat, Uppercase

class Color(StrEnum):
    RED = "red"
    BLUE = "blue"

def _(
    a: Uppercase[Color],
    b: Uppercase[Literal[Color.RED]],
    c: Concat[Literal["is_"], Literal[Color.RED, Color.BLUE]],
):
    reveal_type(a)  # revealed: str
    reveal_type(b)  # revealed: str
    reveal_type(c)  # revealed: str
```

## Size limit

The number of combinations is limited to 256. Larger cross products are reported as
`invalid-type-form`, and the result is `Unknown`. Type aliases that are members of a union argument
are expanded before counting, so `L17` below has 17 members, not 2.

```py
from typing import Literal, LiteralString
from ty_extensions import Concat, Uppercase

type L16 = Literal["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p"]
type L17 = L16 | Literal["q"]

def _(
    ok: Concat[L16, L16],
    # error: [invalid-type-form] "Too many union combinations to evaluate (the limit is 256)"
    too_many: Concat[L17, L16],
    aliased: Uppercase[L17],
):
    # 16 × 16 = 256 combinations: every result is a string literal.
    x: LiteralString = ok
    y: Literal["ap"] = ok  # error: [invalid-assignment]
    reveal_type(too_many)  # revealed: Unknown
    reveal_type(aliased)  # revealed: Literal["A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q"]
```

[PEP 827]: https://peps.python.org/pep-0827/
