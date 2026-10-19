# Invalid callables and class updates

As in the earlier phases, misuses of the [PEP 827] forms are reported as `invalid-type-form`, and the
invalid type evaluates to `Unknown`.

```toml
[environment]
python-version = "3.14"
```

## Elements of `Params`

Every element of `Params` must be a `Param`, and a `Param` must be wrapped in `Params`.

```py
from typing import Callable, Literal
from ty_extensions import Param, Params

def _(
    a: Callable[Params[int], None],  # error: [invalid-type-form]
    b: Callable[[Param[Literal["x"], int]], None],  # error: [invalid-type-form] "`Param` types must be wrapped in `Params[...]`"
):
    reveal_type(a)  # revealed: Unknown
    reveal_type(b)  # revealed: Unknown
```

## Names and kinds

The name of a `Param` must be a string literal or `None`. The kind must be exactly one `ParamKind`
literal. Keyword-only parameters need a name.

```py
from typing import Callable, Literal, Never
from ty_extensions import Param, ParamKind, Params

def _(
    a: Callable[Params[Param[int, int]], None],  # error: [invalid-type-arguments]
    # error: [invalid-type-form]
    b: Callable[Params[Param[Literal["x"], int, Literal[ParamKind.POSITIONAL_ONLY, ParamKind.KEYWORD_ONLY]]], None],
    c: Callable[Params[Param[Literal["x"], int, Never]], None],  # error: [invalid-type-form]
    d: Callable[Params[Param[Literal["x"], int, ParamKind]], None],  # error: [invalid-type-form]
    e: Callable[Params[Param[None, int, Literal[ParamKind.KEYWORD_ONLY]]], None],  # error: [invalid-type-form]
):
    reveal_type(b)  # revealed: Unknown
    reveal_type(c)  # revealed: Unknown
    reveal_type(d)  # revealed: Unknown
    reveal_type(e)  # revealed: Unknown
```

## Parameter order and defaults

The parameters must form a valid signature: positional-only, then positional-or-keyword, then
`*args`, then keyword-only, then `**kwargs`; at most one `*args` and one `**kwargs`, neither with a
default; no duplicate names; and no positional parameter without a default after one with a default.
The reference implementation leaves these checks to `inspect.Signature` at runtime.

```py
from typing import Callable, Literal
from ty_extensions import ArgsParam, KwargsParam, NamedParam, Param, ParamKind, Params

type X = Param[Literal["x"], int]
type Y = Param[Literal["y"], int]
type DefaultX = Param[Literal["x"], int, Literal[ParamKind.POSITIONAL_OR_KEYWORD], Literal[0]]

def _(
    a: Callable[Params[NamedParam[Literal["k"], int], X], None],  # error: [invalid-type-form]
    b: Callable[Params[KwargsParam[int], X], None],  # error: [invalid-type-form]
    c: Callable[Params[ArgsParam[int], ArgsParam[str]], None],  # error: [invalid-type-form]
    d: Callable[Params[X, X], None],  # error: [invalid-type-form]
    e: Callable[Params[DefaultX, Y], None],  # error: [invalid-type-form]
    # error: [invalid-type-form]
    f: Callable[Params[Param[None, int, Literal[ParamKind.VAR_POSITIONAL], Literal[0]]], None],
):
    reveal_type(a)  # revealed: Unknown
    reveal_type(e)  # revealed: Unknown
```

Errors that only appear after specialization are not reported, but the result is still `Unknown`:

```py
from typing import Callable, Literal
from ty_extensions import Param, Params

type Two[N1: str, N2: str] = Callable[Params[Param[N1, int], Param[N2, int]], None]

def _(ok: Two[Literal["a"], Literal["b"]], duplicate: Two[Literal["a"], Literal["a"]]):
    reveal_type(ok)  # revealed: (a: int, b: int) -> None
    reveal_type(duplicate)  # revealed: Unknown
```

## Unbounded parameter lists

The number of parameters must be known.

```py
from typing import Callable
from ty_extensions import Param, Params

type Many = tuple[Param[None, int], ...]

def _(f: Callable[Params[*Many], None]):  # error: [invalid-type-form]
    reveal_type(f)  # revealed: Unknown
```

## Dot access

`Param` has the components `name`, `type`, `kind` and `default`; `Member` has `name`, `type`,
`quals`, `init` and `definer`. Each class only has its own components: `.default` on a `Member` is
still an error, as in phase 2.

```py
from typing import Literal
from ty_extensions import Member, Param

type P = Param[Literal["x"], int]
type M = Member[Literal["x"], int]

def _(
    # error: [invalid-type-form] "`.quals` is not a component of `Param`; expected one of `name`, `type`, `kind` or `default`"
    a: P.quals,
    # error: [invalid-type-form] "`.kind` is not a component of `Member`; expected one of `name`, `type`, `quals`, `init` or `definer`"
    b: M.kind,
    c: M.default,  # error: [invalid-type-form]
    d: P.init,  # error: [invalid-type-form]
    e: P.foo,  # error: [invalid-type-form]
    f: int.kind,  # error: [invalid-type-form] "`.kind` in a type expression requires a `Member` or `Param` type, not `int`"
):
    reveal_type(a)  # revealed: Unknown
    reveal_type(b)  # revealed: Unknown
    reveal_type(c)  # revealed: Unknown
```

## Bare special forms and argument counts

`Overloaded` may only be written without arguments as the `Base` argument of `GetArg` and `GetArgs`
(see `overloaded.md`); in an annotation it needs arguments.

```py
from typing import Annotated
from ty_extensions import DropAnnotations, GenericCallable, GetAnnotations, Overloaded, UpdateClass

def _(
    a: Overloaded,  # error: [invalid-type-form]
    b: GenericCallable,  # error: [invalid-type-form]
    c: GetAnnotations,  # error: [invalid-type-form]
    d: DropAnnotations,  # error: [invalid-type-form]
    e: GetAnnotations[Annotated[int, "x"], str],  # error: [invalid-type-form]
    f: DropAnnotations[()],  # error: [invalid-type-form]
):
    pass

def g() -> UpdateClass:  # error: [invalid-type-form]
    pass
```

[PEP 827]: https://peps.python.org/pep-0827/
