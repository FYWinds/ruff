# `any()` and `all()` over type comprehensions

PEP 827 allows the builtins `any` and `all` in type booleans, applied to a generator of type booleans:
`any(<type boolean> for x in Iter[T] if <type boolean> ...)`. The generator uses the same rules as an
unpacked type comprehension: `Iter[...]` is required, and `if` clauses filter the elements. ty first
evaluates the generator like a comprehension, to a sequence of type booleans; `any` is the `or` of
that sequence, and `all` is its `and`.

```toml
[environment]
python-version = "3.14"
```

## Basic usage

A type boolean can be used directly as an annotation, where it evaluates to `Literal[True]` or
`Literal[False]`.

```py
from ty_extensions import IsAssignable, Iter

type Ts = tuple[int, str, bool]

def _(
    a: any(IsAssignable[t, str] for t in Iter[Ts]),
    b: any(IsAssignable[t, bytes] for t in Iter[Ts]),
    c: all(IsAssignable[t, object] for t in Iter[Ts]),
    d: all(IsAssignable[t, int] for t in Iter[Ts]),
):
    reveal_type(a)  # revealed: Literal[True]
    reveal_type(b)  # revealed: Literal[False]
    reveal_type(c)  # revealed: Literal[True]
    reveal_type(d)  # revealed: Literal[False]
```

## Empty sequences

As in Python, `any` of nothing is false and `all` of nothing is true.

```py
from typing import Never
from ty_extensions import IsAssignable, Iter

def _(
    a: any(IsAssignable[t, int] for t in Iter[tuple[()]]),
    b: all(IsAssignable[t, int] for t in Iter[tuple[()]]),
    c: all(IsAssignable[t, int] for t in Iter[Never]),
):
    reveal_type(a)  # revealed: Literal[False]
    reveal_type(b)  # revealed: Literal[True]
    reveal_type(c)  # revealed: Literal[True]
```

## Filters

```py
from ty_extensions import IsAssignable, Iter

type Ts = tuple[int, str, bool]

def _(
    a: all(IsAssignable[t, int] for t in Iter[Ts] if not IsAssignable[t, str]),
    b: any(IsAssignable[t, str] for t in Iter[Ts] if IsAssignable[t, int]),
):
    reveal_type(a)  # revealed: Literal[True]
    reveal_type(b)  # revealed: Literal[False]
```

## As a condition

```py
from typing import Literal
from ty_extensions import IsAssignable, Iter

type AllInts[T] = Literal["yes"] if all(IsAssignable[t, int] for t in Iter[T]) else Literal["no"]
type HasStr[T] = any(IsAssignable[t, str] for t in Iter[T])
type Both[T] = HasStr[T] and not any(IsAssignable[t, bytes] for t in Iter[T])

def _(
    a: AllInts[tuple[int, bool]],
    b: AllInts[tuple[int, str]],
    c: HasStr[tuple[int, str]],
    d: Both[tuple[str, int]],
    e: Both[tuple[str, bytes]],
):
    reveal_type(a)  # revealed: Literal["yes"]
    reveal_type(b)  # revealed: Literal["no"]
    reveal_type(c)  # revealed: Literal[True]
    reveal_type(d)  # revealed: Literal[True]
    reveal_type(e)  # revealed: Literal[False]
```

## Indeterminate and dynamic elements

The elements are combined like the operands of `or` / `and`: an element of type `bool` makes the
result `bool` unless another element decides it, and a dynamic element makes the result dynamic.

```py
from typing import Any
from ty_extensions import IsAssignable, Iter

type Maybe = bool

def _(
    a: any(Maybe for t in Iter[tuple[int, str]]),
    b: any(IsAssignable[t, int] or Maybe for t in Iter[tuple[int, str]]),
    c: all(Any for t in Iter[tuple[int]]),
):
    reveal_type(a)  # revealed: bool
    reveal_type(b)  # revealed: Literal[True]
    reveal_type(c)  # revealed: Any
```

## Tuples of unbounded length

The unbounded part of a tuple is assumed to have at least one element, as in the reference
implementation.

```py
from ty_extensions import IsAssignable, Iter

type AnyInt[T] = any(IsAssignable[x, int] for x in Iter[T])
type AllInt[T] = all(IsAssignable[x, int] for x in Iter[T])

def _(
    a: AnyInt[tuple[int, ...]],
    b: AnyInt[tuple[str, ...]],
    c: AnyInt[tuple[str, *tuple[int, ...]]],
    d: AllInt[tuple[int, ...]],
    e: AllInt[tuple[str, ...]],
    f: AllInt[tuple[int, *tuple[str, ...], bool]],
    g: AllInt[tuple[int, *tuple[bool, ...], bool]],
):
    reveal_type(a)  # revealed: Literal[True]
    reveal_type(b)  # revealed: Literal[False]
    reveal_type(c)  # revealed: Literal[True]
    reveal_type(d)  # revealed: Literal[True]
    reveal_type(e)  # revealed: Literal[False]
    reveal_type(f)  # revealed: Literal[False]
    reveal_type(g)  # revealed: Literal[True]
```

## Deferred evaluation

```py
from ty_extensions import IsAssignable, Iter

def f[T](x: T, flag: any(IsAssignable[t, int] for t in Iter[T])):
    reveal_type(flag)  # revealed: any(IsAssignable[t, int] for t in Iter[T@f])

def has_int[T](x: T) -> any(IsAssignable[t, int] for t in Iter[T]):
    raise NotImplementedError

def _(pair: tuple[str, int], names: tuple[str, str]):
    reveal_type(has_int(pair))  # revealed: Literal[True]
    reveal_type(has_int(names))  # revealed: Literal[False]
```

## String annotations

```py
from ty_extensions import IsAssignable, Iter

def _(a: "all(IsAssignable[t, int] for t in Iter[tuple[int, bool]])"):
    reveal_type(a)  # revealed: Literal[True]
```

## Nested positions

A quantifier-shaped call is a quantifier wherever it appears in a type expression, not only at the
top level of an annotation. Its generator has no scope of its own, so it is never inferred as a value
there. A generator in the metadata of `Annotated` is a value expression and is not checked.

```py
from typing import Annotated
from ty_extensions import IsAssignable, Iter

type HasInt[T] = list[any(IsAssignable[t, int] for t in Iter[T])]

def _(
    a: list[any(IsAssignable[t, int] for t in Iter[tuple[int]])],
    b: "list[all(IsAssignable[t, int] for t in Iter[tuple[str]])]",
    c: HasInt[tuple[str, int]],
    d: tuple[int, any(IsAssignable[t, str] for t in Iter[tuple[int]])],
    e: Annotated[int, any(t for t in (1, 2))],
):
    reveal_type(a)  # revealed: list[Literal[True]]
    reveal_type(b)  # revealed: list[Literal[False]]
    reveal_type(c)  # revealed: list[Literal[True]]
    reveal_type(d)  # revealed: tuple[int, Literal[False]]
    reveal_type(e)  # revealed: int

from stub import S, V

def _(s: S, v: V):
    reveal_type(s)  # revealed: list[Literal[True]]
    reveal_type(v)  # revealed: list[Unknown]
```

The same holds in stub files, where annotations are not executed:

`stub.pyi`:

```pyi
from typing import TypeAlias
from ty_extensions import IsAssignable, Iter

type S = list[any(IsAssignable[t, int] for t in Iter[tuple[int]])]

def f(x: any(IsAssignable[t, int] for t in Iter[tuple[int]])) -> list[all(IsAssignable[t, int] for t in Iter[tuple[str]])]: ...

V: TypeAlias = list[any(IsAssignable[t, int] for t in Iter[tuple[int]])]  # error: [invalid-type-form]
```

## Only the builtins

`any` and `all` must refer to the builtin functions. If `any` is shadowed, the generator is still
checked like a type comprehension (so `t` is not reported as unresolved), but the result is an error.

```py
from ty_extensions import IsAssignable, Iter

def any(*args: object) -> bool:
    return True

def _(
    # error: [invalid-type-form] "`any` in a type expression must be the builtin `any`"
    a: any(IsAssignable[t, int] for t in Iter[tuple[int]]),
):
    reveal_type(a)  # revealed: Unknown
```

## The element must be a type boolean

```py
from ty_extensions import Iter

def _(
    # error: [invalid-type-form] "The element of `any()` must be a type boolean"
    a: any(t for t in Iter[tuple[int]]),
):
    reveal_type(a)  # revealed: Unknown
```

## Value-indexed positions

Like type comprehensions, `any()` and `all()` are only supported in annotations, `type` statements
and string annotations. Positions that ty indexes as value expressions (such as `X: TypeAlias = ...`
and the type argument of `cast`) report an error.

```py
from typing import TypeAlias, cast
from ty_extensions import IsAssignable, Iter

# error: [invalid-type-form] "Type comprehensions are only supported in annotations, `type` statements and string annotations"
Y: TypeAlias = any(IsAssignable[t, int] for t in Iter[tuple[int]])
# error: [invalid-type-form]
Z = list[all(IsAssignable[t, int] for t in Iter[tuple[int]])]

def _(y: Y, x: object):
    reveal_type(y)  # revealed: Unknown
    # error: [invalid-type-form] "Type comprehensions are only supported in annotations, `type` statements and string annotations"
    z = cast(all(IsAssignable[t, int] for t in Iter[tuple[int]]), x)
    reveal_type(z)  # revealed: Unknown
```

[PEP 827]: https://peps.python.org/pep-0827/
