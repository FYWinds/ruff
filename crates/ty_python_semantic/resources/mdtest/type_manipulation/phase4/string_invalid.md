# Invalid string operators

Misuses of the [PEP 827] string operators are reported as `invalid-type-form`, and the result is
`Unknown`, so that one mistake does not cause a cascade of errors.

```toml
[environment]
python-version = "3.14"
```

## Bare special forms

```py
from ty_extensions import Capitalize, Concat, Lowercase, Uncapitalize, Uppercase

def _(
    a: Uppercase,  # error: [invalid-type-form]
    b: Lowercase,  # error: [invalid-type-form]
    c: Capitalize,  # error: [invalid-type-form]
    d: Uncapitalize,  # error: [invalid-type-form]
    e: Concat,  # error: [invalid-type-form]
):
    reveal_type(a)  # revealed: Unknown
```

## Wrong number of arguments

The case operators take exactly one argument and `Concat` takes exactly two, like in the PEP.
Templates with more parts are written as nested `Concat`s.

```py
from typing import Literal
from ty_extensions import Concat, Uppercase

def _(
    a: Uppercase[Literal["a"], Literal["b"]],  # error: [invalid-type-form]
    b: Uppercase[()],  # error: [invalid-type-form]
    c: Concat[Literal["a"]],  # error: [invalid-type-form]
    d: Concat[Literal["a"], Literal["b"], Literal["c"]],  # error: [invalid-type-form]
):
    reveal_type(a)  # revealed: Unknown
    reveal_type(d)  # revealed: Unknown
```

## Arguments that are not strings

Every argument must be a string type, that is, a type assignable to `str`: a string literal,
`LiteralString`, `str` or a subclass of it. Other types are rejected, including other literal types
(integers, bytes) and the members of an enum that does not inherit from `str`, even if their values
are strings. (Members of a `str`-based enum such as `StrEnum` are accepted, see
`string_lifting.md`.)

```py
from enum import Enum
from typing import Literal
from ty_extensions import Concat, Uppercase

class Mode(Enum):
    READ = "r"

def _(
    # error: [invalid-type-form] "`Uppercase` requires a string type, but got `int`"
    a: Uppercase[int],
    # error: [invalid-type-form] "`Uppercase` requires a string type, but got `Literal[1]`"
    b: Uppercase[Literal[1]],
    c: Uppercase[Literal[b"abc"]],  # error: [invalid-type-form]
    # error: [invalid-type-form] "`Concat` requires a string type, but got `None`"
    d: Concat[Literal["a"], None],
    e: Uppercase[Literal[Mode.READ]],  # error: [invalid-type-form]
    f: Uppercase[list[str]],  # error: [invalid-type-form]
):
    reveal_type(a)  # revealed: Unknown
    reveal_type(e)  # revealed: Unknown
```

If any member of a union argument is invalid, the whole operator fails, even if the other members
are string literals. In particular, an optional string is not accepted:

```py
from typing import Literal
from ty_extensions import Uppercase

def _(
    # error: [invalid-type-form] "`Uppercase` requires a string type, but got `None`"
    a: Uppercase[Literal["a"] | None],
    b: Uppercase[Literal["a", "b"] | int],  # error: [invalid-type-form]
):
    reveal_type(a)  # revealed: Unknown
    reveal_type(b)  # revealed: Unknown
```

## Plain strings are forward references

In a type expression, a plain string is a forward reference, not a string literal type. The string
arguments of the operators have to be written as `Literal[...]`.

```py
from typing import Literal
from ty_extensions import Concat, Uppercase

def _(
    a: Uppercase["foo"],  # error: [unresolved-reference]
    b: Concat[Literal["get_"], "name"],  # error: [unresolved-reference]
    c: Uppercase[Literal["foo"]],
):
    reveal_type(a)  # revealed: Unknown
    reveal_type(b)  # revealed: Unknown
    reveal_type(c)  # revealed: Literal["FOO"]
```

## Invalid `Slice` arguments

`Start` and `End` must be integer literal types or `None`. A non-literal `int` is rejected even
though slicing a string with an `int` is fine at runtime. The sliced type must be a tuple or a string
type.

```py
from typing import Literal
from ty_extensions import Slice

def _(
    # error: [invalid-type-form] "The start index of `Slice` must be an `int` literal type or `None`, but got `int`"
    a: Slice[Literal["abc"], int, None],
    b: Slice[Literal["abc"], Literal["0"], None],  # error: [invalid-type-form]
    c: Slice[Literal["abc"], Literal[0]],  # error: [invalid-type-form]
    d: Slice[Literal["abc"], Literal[0], Literal[2], Literal[1]],  # error: [invalid-type-form]
    # error: [invalid-type-form] "`Slice` requires a tuple or string type, but got `int`"
    e: Slice[int, Literal[0], None],
    # error: [invalid-type-form] "`Slice` requires a tuple or string type, but got `bytes`"
    f: Slice[bytes, Literal[0], None],
):
    reveal_type(a)  # revealed: Unknown
    reveal_type(e)  # revealed: Unknown
```

## Errors that only appear after specialization

An operator whose argument is a type variable is not checked until the type variable is replaced.
If it then turns out to be invalid, there is no type expression left to attach a diagnostic to, so
no error is reported and the result is `Unknown`.

```py
from ty_extensions import Uppercase

def shout[S](text: S) -> Uppercase[S]:
    raise NotImplementedError

def _():
    reveal_type(shout("hi"))  # revealed: Literal["HI"]
    reveal_type(shout(1))  # revealed: Unknown
```

Use an upper bound to report such mistakes at the call site instead:

```py
from ty_extensions import Uppercase

def shout_checked[S: str](text: S) -> Uppercase[S]:
    raise NotImplementedError

def _():
    shout_checked(1)  # error: [invalid-argument-type]
```

[PEP 827]: https://peps.python.org/pep-0827/
