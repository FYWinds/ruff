# `RaiseError`

`RaiseError[S, *Ts]` reports a type error with the message `S` (a string literal type) when it has to
be evaluated. Additional type arguments are appended to the message. Like a failed operator, it
evaluates to `Any`, which ty displays as `Unknown`.

The error is reported with the `raised-type-error` rule where the type is *used*: in an annotation (or
another type expression) with concrete arguments, or at a call whose specialized return type raises.
It is not reported in a branch of a conditional type that is not taken, and not where a generic alias
is defined.

```toml
[environment]
python-version = "3.14"
```

## Evaluated in place

```py
from typing import Literal
from ty_extensions import IsAssignable, RaiseError

def _(
    a: RaiseError[Literal["boom"]],  # error: [raised-type-error] "boom"
    b: RaiseError[Literal["bad pair"], int, str],  # error: [raised-type-error] "bad pair: int, str"
    c: int if IsAssignable[int, int] else RaiseError[Literal["not taken"]],
    d: RaiseError[Literal["taken"]] if IsAssignable[str, int] else int,
):
    reveal_type(a)  # revealed: Unknown
    reveal_type(b)  # revealed: Unknown
    reveal_type(c)  # revealed: int
    reveal_type(d)  # revealed: int
```

## Generic aliases

The alias definition is fine on its own. The error is reported where the alias is specialized with
arguments that select the `RaiseError` branch, including inside other types.

```py
from typing import Literal
from ty_extensions import IsAssignable, RaiseError

type OnlyInts[T] = T if IsAssignable[T, int] else RaiseError[Literal["expected an int"], T]

def _(
    ok: OnlyInts[bool],
    bad: OnlyInts[str],  # error: [raised-type-error] "expected an int: str"
    nested: list[OnlyInts[bytes]],  # error: [raised-type-error] "expected an int: bytes"
    quoted: "OnlyInts[str]",  # error: [raised-type-error] "expected an int: str"
):
    reveal_type(ok)  # revealed: bool
    reveal_type(bad)  # revealed: Unknown
```

## Aliases are expanded in the message

The extra type arguments are displayed after expanding aliases at their outermost level.

```py
from typing import Literal
from ty_extensions import RaiseError, Slice

type Rest[T] = Slice[T, Literal[1], None]
type Complain[T] = RaiseError[Literal["leftover"], T]

def _(
    a: Complain[Rest[tuple[int, str, bytes]]],  # error: [raised-type-error] "leftover: tuple[str, bytes]"
):
    pass
```

## Generic functions

Inside the generic function, the return type is unevaluated and nothing is reported. At a call site
that selects the `RaiseError` branch, the error is reported on the call.

```py
from typing import Literal
from ty_extensions import IsAssignable, RaiseError

def checked[T](x: T) -> T if IsAssignable[T, int] else RaiseError[Literal["expected an int"], T]:
    raise NotImplementedError

def _(n: int, s: str):
    reveal_type(checked(n))  # revealed: int
    # error: [raised-type-error] "expected an int: str"
    reveal_type(checked(s))  # revealed: Unknown
```

## Errors propagate through operators that read them

An operator that has to read a raised error (an argument it inspects, a condition, an iterated
sequence) evaluates to that error, so the message is not lost on the way to the annotation.

```py
from typing import Literal
from ty_extensions import IsAssignable, Iter, Length, RaiseError

type OnlyInts[T] = T if IsAssignable[T, int] else RaiseError[Literal["expected an int"], T]
type Checked[T] = tuple[*[OnlyInts[x] for x in Iter[T]]]

def _(
    a: IsAssignable[OnlyInts[bytes], int],  # error: [raised-type-error] "expected an int: bytes"
    b: Length[OnlyInts[str]],  # error: [raised-type-error] "expected an int: str"
    c: tuple[*[x for x in Iter[tuple[int, str]] if IsAssignable[OnlyInts[x], int]]],  # error: [raised-type-error] "expected an int: str"
    d: Checked[tuple[int, str]],  # error: [raised-type-error] "expected an int: str"
    e: Checked[tuple[int, bool]],
):
    reveal_type(a)  # revealed: Unknown
    reveal_type(b)  # revealed: Unknown
    reveal_type(c)  # revealed: Unknown
    # The elements are the aliases `OnlyInts[int]` and `OnlyInts[bool]`; indexing resolves them.
    reveal_type(e[0])  # revealed: int
    reveal_type(e[1])  # revealed: bool
```

## Errors that are not read stay where they are

Propagation is lazy: an error inside a tuple element is only reported if the element itself ends up
in a checked type. `Length` only looks at the number of elements, so it does not read the element
that raised, and nothing is reported. (The reference implementation evaluates eagerly and raises
here.)

```py
from typing import Literal
from ty_extensions import IsAssignable, Iter, Length, RaiseError

type OnlyInts[T] = T if IsAssignable[T, int] else RaiseError[Literal["expected an int"], T]
type Checked[T] = tuple[*[OnlyInts[x] for x in Iter[T]]]

def _(a: Length[Checked[tuple[int, str]]]):
    reveal_type(a)  # revealed: Literal[2]
```

## Other type expressions

`cast` and other places that take a type expression report the error too.

```py
from typing import Literal, cast
from ty_extensions import IsAssignable, RaiseError

type OnlyInts[T] = T if IsAssignable[T, int] else RaiseError[Literal["expected an int"], T]

def _(x: object):
    y = cast(OnlyInts[str], x)  # error: [raised-type-error] "expected an int: str"
    reveal_type(y)  # revealed: Unknown
```

## The rule can be configured

```toml
[environment]
python-version = "3.14"

[rules]
raised-type-error = "ignore"
```

```py
from typing import Literal
from ty_extensions import RaiseError

def _(a: RaiseError[Literal["silenced"]]):
    reveal_type(a)  # revealed: Unknown
```

## Invalid uses

The message must be a string literal type.

```py
from typing import Literal
from ty_extensions import RaiseError

def _(
    a: RaiseError,  # error: [invalid-type-form]
    # error: [invalid-type-form] "The message of `RaiseError` must be a string literal type, but got `int`"
    b: RaiseError[int],
    # error: [invalid-type-form] "The message of `RaiseError` must be a string literal type, but got `Literal[1]`"
    c: RaiseError[Literal[1]],
):
    reveal_type(b)  # revealed: Unknown
```

[PEP 827]: https://peps.python.org/pep-0827/
