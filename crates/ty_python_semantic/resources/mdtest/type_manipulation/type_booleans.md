# Type booleans

[PEP 827] introduces *type booleans*: special forms that, when used in a type expression, evaluate
to either `Literal[True]` or `Literal[False]`. ty supports three of them, importable (for now) only
from `ty_extensions`:

- `IsAssignable[T, S]`: `Literal[True]` if `T` is assignable to `S`.
- `IsEquivalent[T, S]`: `Literal[True]` if `T` and `S` are assignable to each other.
- `Bool[T]`: `Literal[True]` if `T` is assignable to `Literal[True]` and not assignable to `Never`.

When all operands are fully static (they contain no type variables) the result is computed
immediately, so the annotated parameter simply has the type `Literal[True]` or `Literal[False]`.

```toml
[environment]
python-version = "3.14"
```

## `IsAssignable`

`IsAssignable[T, S]` asks whether a value of type `T` can be assigned to a declaration of type `S`,
using ty's ordinary assignability relation.

```py
from typing import Literal, Never
from ty_extensions import IsAssignable

class Base: ...
class Derived(Base): ...

def _(
    a: IsAssignable[bool, int],
    b: IsAssignable[int, bool],
    c: IsAssignable[Derived, Base],
    d: IsAssignable[Base, Derived],
    e: IsAssignable[Literal[1], int],
    f: IsAssignable[int, object],
    g: IsAssignable[Never, int],
    h: IsAssignable[int, int | str],
):
    reveal_type(a)  # revealed: Literal[True]
    reveal_type(b)  # revealed: Literal[False]
    reveal_type(c)  # revealed: Literal[True]
    reveal_type(d)  # revealed: Literal[False]
    reveal_type(e)  # revealed: Literal[True]
    reveal_type(f)  # revealed: Literal[True]
    reveal_type(g)  # revealed: Literal[True]
    reveal_type(h)  # revealed: Literal[True]
```

## Unions are checked as a whole

Unlike many PEP 827 operators, `IsAssignable` is not "lifted" over unions: a union operand is not
split into its members with the results combined. The question is whether the *whole* union is
assignable. `int | str` is not assignable to `int` (the `str` member is not), so the answer is
`Literal[False]`, not `bool`.

```py
from ty_extensions import IsAssignable

def _(
    a: IsAssignable[int | str, int],
    b: IsAssignable[int | str, int | str | bytes],
    c: IsAssignable[bool | int, int],
):
    reveal_type(a)  # revealed: Literal[False]
    reveal_type(b)  # revealed: Literal[True]
    reveal_type(c)  # revealed: Literal[True]
```

## `IsEquivalent`

`IsEquivalent[T, S]` is defined by the PEP as `IsAssignable[T, S] and IsAssignable[S, T]`. For fully
static types, that coincides with type equivalence.

```py
from typing import Literal
from ty_extensions import IsEquivalent

def _(
    a: IsEquivalent[int, int],
    b: IsEquivalent[int, bool],
    c: IsEquivalent[int | str, str | int],
    d: IsEquivalent[Literal[True, False], bool],
):
    reveal_type(a)  # revealed: Literal[True]
    reveal_type(b)  # revealed: Literal[False]
    reveal_type(c)  # revealed: Literal[True]
    reveal_type(d)  # revealed: Literal[True]
```

## Gradual types

The operators reuse ty's assignability relation, so gradual types behave the way they do anywhere
else: `Any` is assignable to every type and every type is assignable to `Any`.

As a consequence, `IsEquivalent[Any, int]` is `Literal[True]`. It checks *consistency* (mutual
assignability), not ty's internal notion of equivalence, under which `Any` and `int` are different
types. `Bool[Any]` is `Literal[False]` because `Any` is also assignable to `Never`.

```py
from typing import Any
from ty_extensions import Bool, IsAssignable, IsEquivalent

def _(
    a: IsAssignable[Any, int],
    b: IsAssignable[int, Any],
    c: IsEquivalent[Any, int],
    d: IsEquivalent[list[Any], list[int]],
    e: Bool[Any],
):
    reveal_type(a)  # revealed: Literal[True]
    reveal_type(b)  # revealed: Literal[True]
    reveal_type(c)  # revealed: Literal[True]
    reveal_type(d)  # revealed: Literal[True]
    reveal_type(e)  # revealed: Literal[False]
```

## `Bool`

`Bool[T]` converts a type into a type boolean. It is `Literal[True]` exactly when `T` is assignable to
`Literal[True]` but is not `Never` (or another type assignable to `Never`). Everything else, including
`bool` itself, is `Literal[False]`.

```py
from typing import Literal, Never
from ty_extensions import Bool, IsAssignable

def _(
    a: Bool[Literal[True]],
    b: Bool[Literal[False]],
    c: Bool[bool],
    d: Bool[Never],
    e: Bool[int],
    f: Bool[IsAssignable[bool, int]],
    g: Bool[IsAssignable[int, bool]],
):
    reveal_type(a)  # revealed: Literal[True]
    reveal_type(b)  # revealed: Literal[False]
    reveal_type(c)  # revealed: Literal[False]
    reveal_type(d)  # revealed: Literal[False]
    reveal_type(e)  # revealed: Literal[False]
    reveal_type(f)  # revealed: Literal[True]
    reveal_type(g)  # revealed: Literal[False]
```

## Type booleans are ordinary types

The result of a type boolean is the plain type `Literal[True]` or `Literal[False]`, so it can be
nested inside other type expressions and compared against other types. It is not promoted to
`bool`.

```py
from typing import Literal
from ty_extensions import IsAssignable, IsEquivalent

def _(
    a: list[IsAssignable[bool, int]],
    b: IsAssignable[IsAssignable[bool, int], Literal[True]],
    c: IsEquivalent[IsAssignable[int, str], Literal[False]],
    d: IsAssignable[bool, int] | None,
):
    reveal_type(a)  # revealed: list[Literal[True]]
    reveal_type(b)  # revealed: Literal[True]
    reveal_type(c)  # revealed: Literal[True]
    reveal_type(d)  # revealed: Literal[True] | None

def _(x: IsAssignable[bool, int]):
    y: Literal[True] = x
    z: Literal[False] = x  # error: [invalid-assignment]
```

## String annotations

The operators also work inside string (forward-reference) annotations.

```py
from ty_extensions import IsAssignable

def _(a: "IsAssignable[bool, int]", b: list["IsAssignable[int, bool]"]):
    reveal_type(a)  # revealed: Literal[True]
    reveal_type(b)  # revealed: list[Literal[False]]
```

[PEP 827]: https://peps.python.org/pep-0827/
