# Conditional types

[PEP 827] allows `X if <condition> else Y` in type expressions, where the condition is a type
boolean. Type booleans can also be combined with `not`, `and` and `or`. This file covers the cases
where every operand is fully static, so the result is computed immediately; see `deferred.md` for
conditions that depend on type variables.

```toml
[environment]
python-version = "3.14"
```

## True and false branches

The true branch is taken exactly when the condition is `Literal[True]`. `Literal[False]` selects the
false branch.

```py
from ty_extensions import IsAssignable, IsEquivalent, Bool
from typing import Literal

def _(
    a: int if IsAssignable[bool, int] else bytes,
    b: int if IsAssignable[int, bool] else bytes,
    c: int if IsEquivalent[int, int] else bytes,
    d: int if Bool[Literal[False]] else bytes,
):
    reveal_type(a)  # revealed: int
    reveal_type(b)  # revealed: bytes
    reveal_type(c)  # revealed: int
    reveal_type(d)  # revealed: bytes
```

## Conditions that are plain type booleans

Any type expression that evaluates to a type boolean is accepted as a condition. `Literal[True]`
selects the true branch. Per the PEP, every other non-dynamic condition selects the false branch,
including `bool`: it is not *equivalent* to `Literal[True]`.

```py
from typing import Literal

def _(
    a: int if Literal[True] else bytes,
    b: int if Literal[False] else bytes,
    c: int if bool else bytes,
):
    reveal_type(a)  # revealed: int
    reveal_type(b)  # revealed: bytes
    reveal_type(c)  # revealed: bytes
```

## Dynamic conditions

When the condition is a dynamic type such as `Any`, it is unknown which branch applies, so the result
is the union of both branches. (`any extends X ? A : B` behaves the same way in TypeScript.)

```py
from typing import Any

def _(a: int if Any else bytes):
    reveal_type(a)  # revealed: int | bytes
```

## Nested conditional types

Conditional types can be nested in either branch, and they can appear inside other type
expressions.

```py
from ty_extensions import IsAssignable

def _(
    a: int if IsAssignable[str, int] else (str if IsAssignable[bool, int] else bytes),
    b: list[int if IsAssignable[bool, int] else bytes],
    c: dict[str, int if IsAssignable[str, int] else bytes],
    d: tuple[int if IsAssignable[bool, int] else bytes, ...],
    e: (int if IsAssignable[bool, int] else bytes) | None,
):
    reveal_type(a)  # revealed: str
    reveal_type(b)  # revealed: list[int]
    reveal_type(c)  # revealed: dict[str, bytes]
    reveal_type(d)  # revealed: tuple[int, ...]
    reveal_type(e)  # revealed: int | None
```

## `not`, `and` and `or`

The boolean operators work on type booleans and produce type booleans.

```py
from ty_extensions import IsAssignable

def _(
    a: not IsAssignable[str, int],
    b: not IsAssignable[bool, int],
    c: IsAssignable[bool, int] and IsAssignable[int, object],
    d: IsAssignable[bool, int] and IsAssignable[int, bool],
    e: IsAssignable[str, int] or IsAssignable[bool, int],
    f: IsAssignable[str, int] or IsAssignable[bytes, int],
    g: not (IsAssignable[str, int] or IsAssignable[bytes, int]),
    h: IsAssignable[bool, int] and IsAssignable[int, object] and not IsAssignable[str, int],
):
    reveal_type(a)  # revealed: Literal[True]
    reveal_type(b)  # revealed: Literal[False]
    reveal_type(c)  # revealed: Literal[True]
    reveal_type(d)  # revealed: Literal[False]
    reveal_type(e)  # revealed: Literal[True]
    reveal_type(f)  # revealed: Literal[False]
    reveal_type(g)  # revealed: Literal[True]
    reveal_type(h)  # revealed: Literal[True]
```

They can be used as the condition of a conditional type:

```py
from ty_extensions import IsAssignable

def _(
    a: int if IsAssignable[bool, int] and not IsAssignable[str, int] else bytes,
    b: int if IsAssignable[str, int] or IsAssignable[bytes, int] else bytes,
):
    reveal_type(a)  # revealed: int
    reveal_type(b)  # revealed: bytes
```

## Short-circuiting

`and` and `or` are evaluated left to right and stop as soon as the result is known. A later operand
that depends on an unsolved type variable therefore does not prevent the result from being
computed. Likewise, once the condition of a conditional type is known, only the selected branch
matters, even if the other branch mentions a type variable.

```py
from ty_extensions import IsAssignable

def _[T](
    x: T,
    a: IsAssignable[str, int] and IsAssignable[T, int],
    b: IsAssignable[bool, int] or IsAssignable[T, int],
    c: int if IsAssignable[str, int] and IsAssignable[T, int] else bytes,
    d: int if IsAssignable[bool, int] else T,
):
    reveal_type(a)  # revealed: Literal[False]
    reveal_type(b)  # revealed: Literal[True]
    reveal_type(c)  # revealed: bytes
    reveal_type(d)  # revealed: int
```

## String annotations

Conditional types and boolean operators are also supported in string annotations, either as the
whole annotation or nested inside one.

```py
from ty_extensions import IsAssignable

def _(
    a: "int if IsAssignable[bool, int] else bytes",
    b: "not IsAssignable[bool, int]",
    c: list["int if IsAssignable[str, int] else bytes"],
    d: "IsAssignable[bool, int] and IsAssignable[str, int]",
):
    reveal_type(a)  # revealed: int
    reveal_type(b)  # revealed: Literal[False]
    reveal_type(c)  # revealed: list[bytes]
    reveal_type(d)  # revealed: Literal[False]
```

## Every kind of annotation

Conditional types work in every position that takes a type expression: parameter and return
annotations, variable annotations, the value of a `type` statement, and the bounds and defaults of
PEP 695 type parameters.

```py
from ty_extensions import IsAssignable

type Alias = int if IsAssignable[bool, int] else str

def returns() -> int if IsAssignable[bool, int] else str:
    return 1

def uses_alias(x: Alias):
    reveal_type(x)  # revealed: int

def bounded[T: int if IsAssignable[bool, int] else str](x: T) -> T:
    return x

class Box[T = int if IsAssignable[bool, int] else str]:
    pass

def _(box: Box):
    reveal_type(returns())  # revealed: int
    reveal_type(box)  # revealed: Box[int]

    bounded(1)
    bounded("a")  # error: [invalid-argument-type]

    ok: int if IsAssignable[bool, int] else str = 1
    bad: int if IsAssignable[bool, int] else str = "a"  # error: [invalid-assignment]
```

## Diagnostics in the branch that is not taken

Both branches of a conditional type, and every operand of `and` and `or`, are still checked as type
expressions, even when the condition (or an earlier operand) already determines the result. In
particular, the branch that is not selected must not be treated as unreachable code.

```py
from typing import Literal
from ty_extensions import IsAssignable

type AlwaysTrue = Literal[True]
type AlwaysFalse = Literal[False]

def _(
    a: int if IsAssignable[bool, int] else Undefined1,  # error: [unresolved-reference]
    b: Undefined2 if IsAssignable[str, int] else int,  # error: [unresolved-reference]
    c: int if AlwaysTrue else Undefined3,  # error: [unresolved-reference]
    d: Undefined4 if AlwaysFalse else int,  # error: [unresolved-reference]
    e: IsAssignable[str, int] and IsAssignable[Undefined5, int],  # error: [unresolved-reference]
    f: IsAssignable[bool, int] or IsAssignable[Undefined6, int],  # error: [unresolved-reference]
):
    reveal_type(a)  # revealed: int
    reveal_type(b)  # revealed: int
    reveal_type(c)  # revealed: int
    reveal_type(d)  # revealed: int
    reveal_type(e)  # revealed: Literal[False]
    reveal_type(f)  # revealed: Literal[True]

type InAlias = int if IsAssignable[bool, int] else Undefined7  # error: [unresolved-reference]

def returns() -> int if IsAssignable[bool, int] else Undefined8:  # error: [unresolved-reference]
    return 1
```

[PEP 827]: https://peps.python.org/pep-0827/
