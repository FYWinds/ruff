# Deferred evaluation

When an operand of a [PEP 827] type operator or the condition of a conditional type mentions a type
variable that has not been solved yet, the result cannot be computed. ty keeps such a type
unevaluated and evaluates it once the type variable is replaced: when a generic type alias is
specialized, or when a generic function is called.

```toml
[environment]
python-version = "3.14"
```

## Generic type aliases

`Pick[T]` is evaluated separately for every specialization.

```py
from ty_extensions import IsAssignable

type Pick[T] = int if IsAssignable[T, str] else bytes

class MyStr(str): ...

def _(a: Pick[str], b: Pick[float], c: Pick[MyStr], d: list[Pick[str]]):
    reveal_type(a)  # revealed: int
    reveal_type(b)  # revealed: bytes
    reveal_type(c)  # revealed: int
    # Aliases nested inside other types keep their name in the display; the element is evaluated.
    reveal_type(d)  # revealed: list[Pick[str]]
    reveal_type(d[0])  # revealed: int
```

Type booleans and boolean operators are deferred in the same way:

```py
from ty_extensions import IsAssignable, Bool

type IsInt[T] = IsAssignable[T, int]
type StrictInt[T] = IsAssignable[T, int] and not IsAssignable[T, bool]
type Truthy[T] = Bool[T]

def _(a: IsInt[bool], b: IsInt[str], c: StrictInt[int], d: StrictInt[bool], e: Truthy[IsInt[int]]):
    reveal_type(a)  # revealed: Literal[True]
    reveal_type(b)  # revealed: Literal[False]
    reveal_type(c)  # revealed: Literal[True]
    reveal_type(d)  # revealed: Literal[False]
    reveal_type(e)  # revealed: Literal[True]
```

## Conditional types do not distribute over unions

In TypeScript, a conditional type applied to a union is distributed over the members of the union.
PEP 827 conditional types are not: the condition is evaluated once for the whole type argument.
`str | int` is not assignable to `str`, so `Pick[str | int]` is `bytes`, not `int | bytes`.

```py
from ty_extensions import IsAssignable

type Pick[T] = int if IsAssignable[T, str] else bytes

def _(a: Pick[str | int], b: Pick[str | bytes]):
    reveal_type(a)  # revealed: bytes
    reveal_type(b)  # revealed: bytes
```

## Aliases as conditions

The condition does not have to be an operator written in place. Any type expression that evaluates
to a type boolean works, including a generic alias whose value is a type boolean.

```py
from ty_extensions import IsAssignable

type IsStr[T] = IsAssignable[T, str]
type Pick[T] = int if IsStr[T] else bytes
type NotStr[T] = not IsStr[T]

def _(a: IsStr[str], b: Pick[str], c: Pick[bytes], d: NotStr[str]):
    reveal_type(a)  # revealed: Literal[True]
    reveal_type(b)  # revealed: int
    reveal_type(c)  # revealed: bytes
    reveal_type(d)  # revealed: Literal[False]
```

## Nested generic aliases

A deferred alias can be used inside another generic alias. The inner alias is specialized with the
outer type variable, and everything is evaluated once the outer alias is specialized.

```py
from ty_extensions import IsAssignable

type Pick[T] = int if IsAssignable[T, str] else bytes
type Listed[T] = list[Pick[T]]
type Both[T, U] = int if IsAssignable[T, str] and IsAssignable[U, str] else bytes

def _(a: Listed[str], b: Listed[int], c: Both[str, str], d: Both[str, int]):
    reveal_type(a[0])  # revealed: int
    reveal_type(b[0])  # revealed: bytes
    reveal_type(c)  # revealed: int
    reveal_type(d)  # revealed: bytes
```

## Generic functions

A conditional return type is evaluated at each call site, using the type inferred for the type
variable. A single signature can replace a group of overloads.

```py
from ty_extensions import IsAssignable

def parse[T](raw: T) -> int if IsAssignable[T, str] else bytes:
    raise NotImplementedError

def is_str[T](value: T) -> IsAssignable[T, str]:
    raise NotImplementedError

def _(either: str | int):
    reveal_type(parse("a"))  # revealed: int
    reveal_type(parse(1))  # revealed: bytes
    # No distribution at call sites, either.
    reveal_type(parse(either))  # revealed: bytes

    reveal_type(is_str("a"))  # revealed: Literal[True]
    reveal_type(is_str(1))  # revealed: Literal[False]
```

## Display of unevaluated types

Inside the generic function, the type variable is not solved, so the parameter types are displayed
in their unevaluated, source-like form. As an element of a union, an unevaluated conditional is
parenthesized, because `if`/`else` binds more loosely than `|`.

```py
from ty_extensions import IsAssignable

def f[T](
    x: T,
    flag: IsAssignable[T, int],
    value: int if IsAssignable[T, str] else bytes,
    maybe: (int if IsAssignable[T, str] else bytes) | None,
):
    reveal_type(flag)  # revealed: IsAssignable[T@f, int]
    reveal_type(value)  # revealed: int if IsAssignable[T@f, str] else bytes
    reveal_type(maybe)  # revealed: (int if IsAssignable[T@f, str] else bytes) | None
```

## Unevaluated types inside generic bodies

Inside the body of a generic function the condition is unknown, so ty has to be conservative:

- an unevaluated conditional type can be *used* wherever both branches could be used (it is treated
  like the union `X | Y`);
- a value can only be *assigned* to it if it is assignable to both branches (it is treated like the
  intersection `X & Y`).

So returning `1` from a function whose return type is `int if IsAssignable[T, str] else bytes` is an
error, because `1` is not `bytes`. TypeScript has the same limitation; use `cast` to work around it.

```py
from typing import cast
from ty_extensions import IsAssignable

type Result[T] = int if IsAssignable[T, str] else bytes

def strict[T](raw: T) -> Result[T]:
    return 1  # error: [invalid-return-type]

def relaxed[T](raw: T) -> Result[T]:
    return cast(Result[T], 1)

def widen[T](value: Result[T]) -> int | bytes:
    return value

def narrow[T](value: Result[T]) -> int:
    return value  # error: [invalid-return-type]

def same[T](value: Result[T]) -> Result[T]:
    return value

def flag[T](value: IsAssignable[T, int]) -> bool:
    return value
```

## Errors in the branch that is not taken

Diagnostics in both branches are reported where the generic alias is defined, independent of how
it is later specialized.

```py
from ty_extensions import IsAssignable

type Pick[T] = int if IsAssignable[T, str] else Undefined  # error: [unresolved-reference]

def _(a: Pick[str]):
    reveal_type(a)  # revealed: int
```

## Recursive conditional aliases

A recursive alias whose conditional refers to itself must not make ty hang or crash. We only assert
that checking terminates; ty is free to report the alias as circular and to infer anything for its
specializations.

```toml
[environment]
python-version = "3.14"

[rules]
cyclic-type-alias-definition = "ignore"
```

```py
from ty_extensions import IsAssignable

type R[T] = R[T] if IsAssignable[T, int] else str

def _(recursive: R[int], terminal: R[str]):
    # Checking continues normally after the recursive alias.
    reveal_type(1)  # revealed: Literal[1]
```

[PEP 827]: https://peps.python.org/pep-0827/
