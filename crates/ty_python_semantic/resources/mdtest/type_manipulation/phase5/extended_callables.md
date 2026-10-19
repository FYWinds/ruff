# Extended callables

[PEP 827] describes a callable's parameter list as `Params[*Ps]`, a sequence of `Param[N, T, K, D]`
types: the name (a string literal, or `None` for an unnamed parameter), the type, the kind (a
`ParamKind` literal, `POSITIONAL_OR_KEYWORD` by default) and the type of the default value (`Never`
if there is no default). `Callable[Params[...], R]` is converted to an ordinary callable type, so it
is displayed, called and compared like any other callable.

```toml
[environment]
python-version = "3.14"
```

## All parameter kinds

This is the example from the PEP. The extended callable is equivalent to the callable type of the
corresponding function.

```py
from typing import Callable, Literal
from ty_extensions import Param, ParamKind, Params, static_assert
from ty_extensions._internal import RegularCallableTypeOf, is_equivalent_to

type Func = Callable[
    Params[
        Param[Literal["a"], int, Literal[ParamKind.POSITIONAL_ONLY]],
        Param[Literal["b"], int],
        Param[Literal["c"], int, Literal[ParamKind.POSITIONAL_OR_KEYWORD], Literal[0]],
        Param[None, int, Literal[ParamKind.VAR_POSITIONAL]],
        Param[Literal["d"], int, Literal[ParamKind.KEYWORD_ONLY]],
        Param[Literal["e"], int, Literal[ParamKind.KEYWORD_ONLY], Literal[0]],
        Param[None, int, Literal[ParamKind.VAR_KEYWORD]],
    ],
    int,
]

def func(a: int, /, b: int, c: int = 0, *args: int, d: int, e: int = 0, **kwargs: int) -> int:
    return 0

def _(f: Func):
    reveal_type(f)  # revealed: (a: int, /, b: int, c: int = 0, *args: int, d: int, e: int = 0, **kwargs: int) -> int
    reveal_type(f(1, 2, d=3))  # revealed: int
    f(1, 2)  # error: [missing-argument]
    f(1, 2, d="x")  # error: [invalid-argument-type]

static_assert(is_equivalent_to(Func, RegularCallableTypeOf[func]))
```

## Abbreviations

The aliases from the PEP are available as well. A default given through an alias has the parameter's
type rather than a literal type, so it is displayed as `...`.

```py
from typing import Callable, Literal
from ty_extensions import (
    ArgsParam,
    DefaultParam,
    KwargsParam,
    NamedDefaultParam,
    NamedParam,
    Param,
    Params,
    PosParam,
)

type Func = Callable[
    Params[
        PosParam[int],
        Param[Literal["b"], int],
        DefaultParam[Literal["c"], int],
        ArgsParam[int],
        NamedParam[Literal["d"], int],
        NamedDefaultParam[Literal["e"], int],
        KwargsParam[int],
    ],
    int,
]

def _(f: Func):
    reveal_type(f)  # revealed: (int, /, b: int, c: int = ..., *args: int, d: int, e: int = ..., **kwargs: int) -> int
```

## Unnamed parameters

A parameter without a name can only be passed positionally. `Params` with only unnamed parameters is
the same as the classic list form.

```py
from typing import Callable
from ty_extensions import Param, Params, static_assert
from ty_extensions._internal import is_equivalent_to

type Classic = Callable[[int, str], bool]
type Extended = Callable[Params[Param[None, int], Param[None, str]], bool]

def _(f: Extended):
    reveal_type(f)  # revealed: (int, str, /) -> bool

static_assert(is_equivalent_to(Classic, Extended))
```

## Gradual parameter lists

`*args: Any` together with `**kwargs: Any` is the gradual parameter list `...`.

```py
from typing import Any, Callable, Literal
from ty_extensions import Param, ParamKind, Params, static_assert
from ty_extensions._internal import is_equivalent_to

type Gradual = Callable[
    Params[
        Param[None, Any, Literal[ParamKind.VAR_POSITIONAL]],
        Param[None, Any, Literal[ParamKind.VAR_KEYWORD]],
    ],
    int,
]

def _(f: Gradual):
    reveal_type(f)  # revealed: (...) -> int

static_assert(is_equivalent_to(Gradual, Callable[..., int]))
```

## Parameter lists of unknown length

When the arguments of `Params` come from a sequence of unknown length and contents (for example a
comprehension over `Any`), the parameter list is gradual.

```py
from typing import Any, Callable
from ty_extensions import Iter, Params

type Mapped[T] = Callable[Params[*[p for p in Iter[T]]], int]

def _(f: Mapped[Any]):
    reveal_type(f)  # revealed: (...) -> int
```

## Empty parameter lists

```py
from typing import Callable
from ty_extensions import Params

def _(f: Callable[Params[()], None]):
    reveal_type(f)  # revealed: () -> None
```

## Components of a `Param`

Like `Member`, `Param` is a regular class. Its components can be read with dot access in type
expressions (`.name`, `.type`, `.kind`, `.default`) and as properties on values.

```py
from typing import Literal
from ty_extensions import Param, ParamKind

type P = Param[Literal["x"], int, Literal[ParamKind.KEYWORD_ONLY], Literal[0]]
type Q = Param[None, str]

def _(
    name: P.name,
    type_: P.type,
    kind: P.kind,
    default: P.default,
    q_name: Q.name,
    q_kind: Q.kind,
    q_default: Q.default,
):
    reveal_type(name)  # revealed: Literal["x"]
    reveal_type(type_)  # revealed: int
    reveal_type(kind)  # revealed: Literal[ParamKind.KEYWORD_ONLY]
    reveal_type(default)  # revealed: Literal[0]
    reveal_type(q_name)  # revealed: None
    reveal_type(q_kind)  # revealed: Literal[ParamKind.POSITIONAL_OR_KEYWORD]
    reveal_type(q_default)  # revealed: Never

def _(p: P):
    reveal_type(p)  # revealed: Param[Literal["x"], int, Literal[ParamKind.KEYWORD_ONLY], Literal[0]]
    reveal_type(p.kind)  # revealed: Literal[ParamKind.KEYWORD_ONLY]
```

`Param` is covariant in all of its type arguments, so a `Param` with a different name or kind is not
assignable:

```py
from typing import Literal
from ty_extensions import Param, ParamKind, static_assert
from ty_extensions._internal import is_assignable_to

static_assert(is_assignable_to(Param[Literal["x"], bool], Param[Literal["x"], int]))
static_assert(not is_assignable_to(Param[Literal["x"], int], Param[Literal["y"], int]))
static_assert(
    not is_assignable_to(
        Param[Literal["x"], int, Literal[ParamKind.KEYWORD_ONLY]],
        Param[Literal["x"], int],
    )
)
```

## Building parameter lists with comprehensions

The arguments of `Params` are a variadic position: unpacked comprehensions and unpacked tuples are
spliced in.

```py
from typing import Callable, Literal
from ty_extensions import Attrs, Iter, Param, ParamKind, Params

class Point:
    x: int
    y: int = 0

type KwOnlyInit[T] = Callable[
    Params[*[Param[p.name, p.type, Literal[ParamKind.KEYWORD_ONLY], p.init] for p in Iter[Attrs[T]]]],
    T,
]

type Suffix = tuple[Param[Literal["flag"], bool], Param[Literal["level"], int]]

def _(make: KwOnlyInit[Point], g: Callable[Params[Param[None, str], *Suffix], None]):
    reveal_type(make)  # revealed: (*, x: int, y: int = 0) -> Point
    reveal_type(make(x=1))  # revealed: Point
    reveal_type(g)  # revealed: (str, /, flag: bool, level: int) -> None
```

## Deferred evaluation

When the parameter list depends on a type variable, the callable type is evaluated once the type
variable is specialized: when a generic alias is specialized, or at a call site.

```py
from typing import Callable, Literal
from ty_extensions import Param, ParamKind, Params

type Setter[N: str, T] = Callable[Params[Param[N, T, Literal[ParamKind.KEYWORD_ONLY]]], None]

def _(a: Setter[Literal["width"], int], b: Setter[Literal["name"], str]):
    reveal_type(a)  # revealed: (*, width: int) -> None
    reveal_type(b)  # revealed: (*, name: str) -> None
    a(width=1)
    a(width=1, height=1)  # error: [unknown-argument]

def keyword_setter[T](default: T) -> Callable[Params[Param[Literal["value"], T, Literal[ParamKind.KEYWORD_ONLY], T]], None]:
    raise NotImplementedError

def _():
    reveal_type(keyword_setter(1.5))  # revealed: (*, value: float = ...) -> None
```

## `classmethod` and `staticmethod`

The `ParamSpec` argument of `classmethod` and `staticmethod` accepts `Params` as well.

```py
from typing import Literal
from ty_extensions import Param, Params

class C: ...

def _(
    c: classmethod[C, Params[Param[Literal["x"], int]], str],
    s: staticmethod[Params[Param[Literal["x"], int]], str],
):
    reveal_type(c)  # revealed: classmethod[C, (x: int), str]
    reveal_type(s)  # revealed: staticmethod[(x: int), str]
```

[PEP 827]: https://peps.python.org/pep-0827/
