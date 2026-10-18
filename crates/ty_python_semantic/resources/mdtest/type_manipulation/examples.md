# PEP 827 playground examples

This file is both an mdtest and the source of the local playground's "Examples" menu. Every `##`
section is one example: the `[//]: # (playground: ...)` line right after the heading holds its
metadata (a Markdown link-reference comment, which mdtest treats as prose), the prose up to the
first code block is the description shown in the playground, the `toml` block becomes `ty.json`,
and the unlabeled `py` blocks become `main.py`.

`ty_extensions` does not exist at runtime, so examples only import it under `if TYPE_CHECKING:` and
rely on Python 3.14's deferred evaluation of annotations.

## Intersections and negations

[//]: # (playground: slug=intersection-not, phase=0, ts="A & B")

ty already supports two type constructors that Python itself cannot express: `Intersection[A, B]`
(a value that is both an `A` and a `B`) and `Not[T]` (any value that is not a `T`). They are
available from `ty_extensions`, and ty also creates them on its own when narrowing.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ty_extensions import Intersection, Not

class Walker: ...
class Swimmer: ...

def f(both: Intersection[Walker, Swimmer], only: Intersection[Walker, Not[Swimmer]]):
    reveal_type(both)  # revealed: Walker & Swimmer
    reveal_type(only)  # revealed: Walker & ~Swimmer

def g(value: Not[Not[int]]):
    reveal_type(value)  # revealed: int

def h(value: Walker):
    if not isinstance(value, Swimmer):
        reveal_type(value)  # revealed: Walker & ~Swimmer
```

## Type booleans

[//]: # (playground: slug=type-booleans, phase=1, ts="A extends B ? true : false")

PEP 827 adds *type booleans*. `IsAssignable[T, S]`, `IsEquivalent[T, S]` and `Bool[T]` are type
expressions that evaluate to `Literal[True]` or `Literal[False]`. Unions are checked as a whole:
`int | str` is not assignable to `int`.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, Literal

if TYPE_CHECKING:
    from ty_extensions import Bool, IsAssignable, IsEquivalent

def f(
    a: IsAssignable[bool, int],
    b: IsAssignable[int | str, int],
    c: IsEquivalent[int | str, str | int],
    d: Bool[Literal[True]],
):
    reveal_type(a)  # revealed: Literal[True]
    reveal_type(b)  # revealed: Literal[False]
    reveal_type(c)  # revealed: Literal[True]
    reveal_type(d)  # revealed: Literal[True]
```

## Gradual types

[//]: # (playground: slug=gradual-types, phase=1, ts="any is assignable both ways")

The operators use ty's normal assignability rules, so `Any` is assignable to everything and
everything is assignable to `Any`. `IsEquivalent` therefore checks *consistency*:
`IsEquivalent[Any, int]` is true. `Bool[Any]` is false, because `Any` is also assignable to `Never`.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from ty_extensions import Bool, IsAssignable, IsEquivalent

def f(
    a: IsAssignable[Any, int],
    b: IsAssignable[int, Any],
    c: IsEquivalent[Any, int],
    d: Bool[Any],
):
    reveal_type(a)  # revealed: Literal[True]
    reveal_type(b)  # revealed: Literal[True]
    reveal_type(c)  # revealed: Literal[True]
    reveal_type(d)  # revealed: Literal[False]
```

## Conditional types

[//]: # (playground: slug=conditional-basic, phase=1, ts="T extends U ? X : Y")

A conditional type `X if <type boolean> else Y` picks `X` when the condition is `Literal[True]` and
`Y` otherwise. With concrete operands it is evaluated immediately, anywhere a type expression is
allowed.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ty_extensions import IsAssignable

def f(
    a: int if IsAssignable[bool, int] else bytes,
    b: int if IsAssignable[str, int] else bytes,
    c: list[str if IsAssignable[bool, int] else None],
):
    reveal_type(a)  # revealed: int
    reveal_type(b)  # revealed: bytes
    reveal_type(c)  # revealed: list[str]
```

## Combining conditions

[//]: # (playground: slug=bool-combinators, phase=1, ts="A extends X ? B extends Y ? T : F : F")

`not`, `and` and `or` combine type booleans. They short-circuit from left to right, so a result is
known as soon as possible, even if a later operand still depends on an unsolved type variable.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ty_extensions import IsAssignable

def f[T](
    x: T,
    a: not IsAssignable[str, int],
    b: IsAssignable[bool, int] and IsAssignable[int, bool],
    c: IsAssignable[str, int] and IsAssignable[T, int],
    d: IsAssignable[bool, int] or IsAssignable[T, int],
):
    reveal_type(a)  # revealed: Literal[True]
    reveal_type(b)  # revealed: Literal[False]
    reveal_type(c)  # revealed: Literal[False]
    reveal_type(d)  # revealed: Literal[True]
```

## Generic conditional aliases

[//]: # (playground: slug=generic-alias, phase=1, ts="type Pick<T> = T extends string ? number : Uint8Array")

When a condition mentions a type parameter, it cannot be decided yet. ty keeps it unevaluated and
decides it once the alias is specialized.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ty_extensions import IsAssignable

type Pick[T] = int if IsAssignable[T, str] else bytes

def f(a: Pick[str], b: Pick[float], c: list[Pick[str]]):
    reveal_type(a)  # revealed: int
    reveal_type(b)  # revealed: bytes
    reveal_type(c[0])  # revealed: int
```

## No distribution over unions

[//]: # (playground: slug=no-distribution, phase=1, ts="[T] extends [string] ? number : Uint8Array")

Unlike TypeScript's distributive conditional types, a conditional type is evaluated once for the
whole type argument. `str | int` is not assignable to `str`, so `Pick[str | int]` is `bytes`, not
`int | bytes`.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ty_extensions import IsAssignable

type Pick[T] = int if IsAssignable[T, str] else bytes

def f(a: Pick[str | int], b: Pick[str | str]):
    reveal_type(a)  # revealed: bytes
    reveal_type(b)  # revealed: int
```

## Aliases as conditions

[//]: # (playground: slug=alias-condition, phase=1, ts="type IsStr<T> = T extends string ? true : false")

Any type expression that evaluates to a type boolean can be used as a condition, including a
(generic) type alias. This makes it easy to name and reuse predicates.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ty_extensions import IsAssignable

type IsStr[T] = IsAssignable[T, str]
type Pick[T] = int if IsStr[T] else bytes

def f(a: IsStr[str], b: Pick[str], c: Pick[bytes]):
    reveal_type(a)  # revealed: Literal[True]
    reveal_type(b)  # revealed: int
    reveal_type(c)  # revealed: bytes
```

## Conditional return types

[//]: # (playground: slug=generic-function, phase=1, ts="return type T extends string ? number : Uint8Array")

A conditional return type is evaluated at each call site, using the type inferred for `T`. One
signature can replace a group of `@overload`s.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ty_extensions import IsAssignable

def parse[T](raw: T) -> int if IsAssignable[T, str] else bytes:
    raise NotImplementedError

def f(either: str | int):
    reveal_type(parse("42"))  # revealed: int
    reveal_type(parse(42))  # revealed: bytes
    reveal_type(parse(either))  # revealed: bytes
```

## Limitation inside generic bodies

[//]: # (playground: slug=body-limitation, phase=1, ts="same limitation: use a cast")

Inside the generic function the condition is unknown, so ty checks a returned value against *both*
branches: `1` is an `int` but not `bytes`, so it is rejected. TypeScript has the same limitation.
Use `cast` when you know better.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, cast

if TYPE_CHECKING:
    from ty_extensions import IsAssignable

type Result[T] = int if IsAssignable[T, str] else bytes

def strict[T](raw: T) -> Result[T]:
    return 1  # error: [invalid-return-type]

def relaxed[T](raw: T) -> Result[T]:
    return cast(Result[T], 1)

def f():
    reveal_type(relaxed("a"))  # revealed: int
```

## Invalid usage

[//]: # (playground: slug=invalid-usage, phase=1)

The condition of a conditional type, and the operands of `not`, `and` and `or`, must be type
booleans. The operators also check how many arguments they receive.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ty_extensions import Bool, IsAssignable

def f(
    # error: [invalid-type-form] "Condition of a conditional type must be a type boolean"
    a: int if str else bytes,
    # error: [invalid-type-form] "Operands of `and`/`or`/`not` in a type expression must be type booleans"
    b: not int,
    c: IsAssignable[int],  # error: [invalid-type-form]
    d: Bool[int, str],  # error: [invalid-type-form]
):
    pass
```
