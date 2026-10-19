# Invalid uses of the phase 3 operators

All misuses are reported as `invalid-type-form`, and the invalid operation evaluates to `Unknown`.
(`RaiseError` itself reports `raised-type-error`; see `raise_error.md`.)

```toml
[environment]
python-version = "3.14"
```

## Bare special forms

```py
from ty_extensions import GetArg, GetArgs, GetSpecialAttr, Length, RaiseError, Slice

def _(
    a: GetArg,  # error: [invalid-type-form]
    b: GetArgs,  # error: [invalid-type-form]
    c: Length,  # error: [invalid-type-form]
    d: Slice,  # error: [invalid-type-form]
    e: GetSpecialAttr,  # error: [invalid-type-form]
    f: RaiseError,  # error: [invalid-type-form]
):
    pass
```

## Wrong number of arguments

`GetArg` and `Slice` take three arguments, `GetArgs` and `GetSpecialAttr` two, `Length` one, and
`RaiseError` at least one.

```py
from typing import Literal
from ty_extensions import GetArg, GetArgs, GetSpecialAttr, Length, RaiseError, Slice

def _(
    a: GetArg[list[int], list],  # error: [invalid-type-form]
    b: GetArgs[list[int]],  # error: [invalid-type-form]
    c: Length[tuple[int], tuple[str]],  # error: [invalid-type-form]
    d: Slice[tuple[int], Literal[0]],  # error: [invalid-type-form]
    e: GetSpecialAttr[int],  # error: [invalid-type-form]
    f: RaiseError[()],  # error: [invalid-type-form]
):
    reveal_type(a)  # revealed: Unknown
```

## Indexes must be `int` literal types

```py
from typing import Literal
from ty_extensions import GetArg

def _(
    # error: [invalid-type-form] "The index of `GetArg` must be an `int` literal type, but got `int`"
    a: GetArg[list[int], list, int],
    # error: [invalid-type-form] "The index of `GetArg` must be an `int` literal type"
    b: GetArg[list[int], list, Literal["0"]],
    c: GetArg[list[int], list, Literal[False]],
):
    reveal_type(a)  # revealed: Unknown
    reveal_type(c)  # revealed: int
```

## The base must be a class

```py
from typing import Literal
from ty_extensions import GetArg, GetArgs

def _(
    # error: [invalid-type-form] "The `Base` argument of `GetArg` must be a class"
    a: GetArg[list[int], Literal[1], Literal[0]],
    # error: [invalid-type-form] "The `Base` argument of `GetArgs` must be a class"
    b: GetArgs[list[int], Literal["list"]],
):
    reveal_type(a)  # revealed: Unknown
```

## Generators outside of `any()` and `all()`

A generator expression is only a type-level comprehension as the single argument of `any` or `all`.
Elsewhere, the existing diagnostics are unchanged.

```py
def _(
    # error: [invalid-type-form] "Generator expressions are not allowed in parameter annotations"
    a: (x for x in range(3)),
):
    pass
```

## Errors inside `any()` and `all()`

The generator follows the rules of type comprehensions.

```py
from ty_extensions import IsAssignable, Iter

def _(
    # error: [invalid-type-form] "The iterable of a type comprehension must be `Iter[...]`"
    a: any(IsAssignable[t, int] for t in tuple[int, str]),
    # error: [invalid-type-form] "Condition of a type comprehension must be a type boolean"
    b: all(IsAssignable[t, int] for t in Iter[tuple[int]] if int),
):
    reveal_type(a)  # revealed: Unknown
    reveal_type(b)  # revealed: Unknown
```

[PEP 827]: https://peps.python.org/pep-0827/
