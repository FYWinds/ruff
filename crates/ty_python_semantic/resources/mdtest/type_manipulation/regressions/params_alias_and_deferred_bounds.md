# Regressions: generic `Params` aliases and deferred bound checks

```toml
[environment]
python-version = "3.14"
```

## Generic alias whose body is `Params[*comprehension]`

The unpacked comprehension inside `Params[...]` must be spliced into the parameter list when the
`Params` comes from a specialized generic alias, exactly as it is for a non-generic alias or an
inline `Params`. The bug: an unpacked sequence that is not known yet (the comprehension iterates
over `T`) was packed into the `TypeVarTuple` argument of `Params` as a single element, so after
specialization the argument was `tuple[tuple[Param[...]]]` instead of `tuple[Param[...]]`.

```py
from typing import Callable, Literal
from ty_extensions import Attrs, Iter, Param, Params

class P:
    x: int

type Ps[T] = Params[*[Param[p.name, p.type] for p in Iter[Attrs[T]]]]
type NonGeneric = Params[*[Param[p.name, p.type] for p in Iter[Attrs[P]]]]

type Ps2[T] = Params[Param[Literal["a"], str], *[Param[p.name, p.type] for p in Iter[Attrs[T]]]]

def _(a: Callable[Ps[P], None], b: Callable[NonGeneric, None], c: Callable[Ps2[P], None]):
    reveal_type(a)  # revealed: (x: int) -> None
    reveal_type(b)  # revealed: (x: int) -> None
    reveal_type(c)  # revealed: (a: str, x: int) -> None
```

The same applies to the `TypeVarTuple` of any generic class:

```py
from ty_extensions import Attrs, Iter

class Box[*Ts]: ...

class Q:
    x: int
    y: str

type Boxed[T] = Box[*[p.type for p in Iter[Attrs[T]]]]
type Boxed2[T] = Box[bytes, *[p.type for p in Iter[Attrs[T]]], None]

def _(a: Boxed[Q], b: Boxed2[Q]):
    reveal_type(a)  # revealed: Box[int, str]
    reveal_type(b)  # revealed: Box[bytes, int, str, None]

def _[T](a: Boxed[T]):
    reveal_type(a)  # revealed: Box[*tuple[*[p.type for p in Iter[Attrs[T@_]]]]]
```

## `Param` defaults written as type aliases

The default of a `Param` is the value of a type alias that spells it (directly, or computed by a
generic alias over a comprehension variable); the signature used to display it as `...` because the
default was kept as the alias.

```py
from typing import Callable, Literal
from ty_extensions import Attrs, GetMemberType, Iter, Param, ParamKind, Params

type Zero = Literal[0]
type KwOnly = Literal[ParamKind.KEYWORD_ONLY]
type DefaultOf[D, N] = GetMemberType[D, N]

class Defaults:
    x: Literal["origin"]

class P:
    x: str

type WithDefaults[T] = Params[*[Param[p.name, p.type, KwOnly, DefaultOf[Defaults, p.name]] for p in Iter[Attrs[T]]]]

def _(
    a: Callable[Params[Param[Literal["n"], int, KwOnly, Zero]], None],
    b: Callable[WithDefaults[P], None],
):
    reveal_type(a)  # revealed: (*, n: int = 0) -> None
    reveal_type(b)  # revealed: (*, x: str = "origin") -> None
```

## Bound checks against deferred type operators

A type operator that can only be evaluated once an enclosing alias is specialized (here
`GetArg[kv, ...]` over a comprehension variable, directly or through a helper alias) used to be
checked eagerly against the upper bound of a type variable as its own upper bound (`object`),
which reported `invalid-type-arguments` and made the specialization `Unknown`. Such an operator
can still evaluate to any type, so it is not checked when it is written (it is checked as `Never`,
which satisfies every bound); a concrete violation is still reported, also next to a deferred
operator in a union.

```py
from typing import Literal
from ty_extensions import GetArg, Iter, Member, NewTypedDict

type Record[T] = NewTypedDict[*[Member[GetArg[kv, tuple, Literal[0]], GetArg[kv, tuple, Literal[1]]] for kv in Iter[T]]]

type First[T] = GetArg[T, tuple, Literal[0]]
type Second[T] = GetArg[T, tuple, Literal[1]]
type Record2[T] = NewTypedDict[*[Member[First[kv], Second[kv]] for kv in Iter[T]]]

def _(
    r: Record[tuple[tuple[Literal["id"], int], tuple[Literal["name"], str]]],
    r2: Record2[tuple[tuple[Literal["id"], int], tuple[Literal["name"], str]]],
):
    reveal_type(r["id"])  # revealed: int
    reveal_type(r["name"])  # revealed: str
    reveal_type(r2["id"])  # revealed: int
    reveal_type(r2["name"])  # revealed: str

# error: [invalid-type-arguments]
type Bad = Member[int, str]

# error: [invalid-type-arguments]
type BadUnion[T] = tuple[*[Member[int | GetArg[kv, tuple, Literal[0]], int] for kv in Iter[T]]]
```

This applies to every bounded or constrained type variable, of a class or of a type alias:

```py
from typing import Literal
from ty_extensions import GetArg, Iter, Uppercase

class Named[S: str]:
    name: S

class Constrained[S: (int, str)]:
    value: S

type Upper[S: str] = Uppercase[S]

type Names[T] = tuple[*[Named[GetArg[kv, tuple, Literal[0]]] for kv in Iter[T]]]
type Values[T] = tuple[*[Constrained[GetArg[kv, tuple, Literal[0]]] for kv in Iter[T]]]
type Uppers[T] = tuple[*[Upper[GetArg[kv, tuple, Literal[0]]] for kv in Iter[T]]]

def _(
    n: Names[tuple[tuple[Literal["a"]], tuple[Literal["b"]]]],
    v: Values[tuple[tuple[int], tuple[str]]],
    u: Uppers[tuple[tuple[Literal["a"]]]],
):
    reveal_type(n)  # revealed: tuple[Named[Literal["a"]], Named[Literal["b"]]]
    reveal_type(v)  # revealed: tuple[Constrained[int], Constrained[str]]
    reveal_type(u)  # revealed: tuple[Literal["A"]]
```

When the deferred operator evaluates to a type that violates the bound only after specialization,
no diagnostic is emitted: like every failure that only shows up at specialization, it is silent.
The specialized `Member[int, str]` is not a valid member, so the `NewProtocol` built from it is
`Unknown`.

```py
from typing import Literal
from ty_extensions import GetArg, Iter, Member, NewProtocol

type Proto[T] = NewProtocol[*[Member[GetArg[kv, tuple, Literal[0]], GetArg[kv, tuple, Literal[1]]] for kv in Iter[T]]]

def _(p: Proto[tuple[tuple[int, str]]]):
    reveal_type(p)  # revealed: Unknown
```
