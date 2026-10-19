# Invalid member inspection and type comprehensions

Misuses of the phase-2 [PEP 827] operators are reported as `invalid-type-form`, and the failed
operation evaluates to `Unknown`.

```toml
[environment]
python-version = "3.14"
```

## Bare special forms

```py
from ty_extensions import Attrs, FromUnion, GetMember, GetMemberType, Iter, Members, NewProtocol, NewTypedDict

def _(
    a: Attrs,  # error: [invalid-type-form]
    b: Members,  # error: [invalid-type-form]
    c: GetMember,  # error: [invalid-type-form]
    d: GetMemberType,  # error: [invalid-type-form]
    e: FromUnion,  # error: [invalid-type-form]
    f: NewProtocol,  # error: [invalid-type-form]
    g: NewTypedDict,  # error: [invalid-type-form]
    h: Iter,  # error: [invalid-type-form]
):
    pass
```

## Wrong number of arguments

`Attrs`, `Members` and `FromUnion` take one argument; `GetMember` and `GetMemberType` take two.
`Member` is a generic class with two required type parameters.

```py
from typing import Literal
from ty_extensions import Attrs, FromUnion, GetMember, GetMemberType, Member, Members

class A:
    x: int

def _(
    a: Attrs[A, A],  # error: [invalid-type-form]
    b: Members[()],  # error: [invalid-type-form]
    c: FromUnion[int, str],  # error: [invalid-type-form]
    d: GetMember[A],  # error: [invalid-type-form]
    e: GetMemberType[A, Literal["x"], int],  # error: [invalid-type-form]
    f: Member[Literal["x"]],  # error: [invalid-type-arguments]
):
    reveal_type(a)  # revealed: Unknown
    reveal_type(d)  # revealed: Unknown
```

## Operands that are not types

```py
from typing import Literal
from ty_extensions import Attrs, GetMemberType

class A:
    x: int

def _(
    a: Attrs[1],  # error: [invalid-type-form]
    b: GetMemberType[A, [int]],  # error: [invalid-type-form]
):
    pass
```

## Member names must be string literals

```py
from typing import Literal
from ty_extensions import GetMember, GetMemberType

class A:
    x: int

def _(
    a: GetMemberType[A, str],  # error: [invalid-type-form]
    b: GetMember[A, Literal[1]],  # error: [invalid-type-form]
):
    reveal_type(a)  # revealed: Unknown
    reveal_type(b)  # revealed: Unknown
```

## `NewProtocol` and `NewTypedDict` arguments must be members

```py
from typing import Literal
from ty_extensions import Member, NewProtocol, NewTypedDict

def _(
    a: NewProtocol[int],  # error: [invalid-type-form]
    b: NewTypedDict[Member[Literal["a"], int], str],  # error: [invalid-type-form]
):
    reveal_type(a)  # revealed: Unknown
    reveal_type(b)  # revealed: Unknown
```

## `Iter` is required

The `for` clause of a type comprehension must iterate over `Iter[...]`. (The reference
implementation iterates tuple types directly; ty follows the PEP grammar.)

```py
from ty_extensions import Iter

def _(
    a: tuple[*[list[t] for t in tuple[int, str]]],  # error: [invalid-type-form]
):
    reveal_type(a)  # revealed: Unknown
```

`Iter` cannot be used anywhere else:

```py
from ty_extensions import Iter

def _(
    a: Iter[tuple[int]],  # error: [invalid-type-form]
    b: list[Iter[tuple[int]]],  # error: [invalid-type-form]
):
    pass
```

## What can be iterated

`Iter` requires a tuple type, `Never` or `Any`. Iterating something that is not a tuple, or a union
of tuples (for example the result of a lifted `Attrs[A | B]`), is an error. (Tuples with an
unbounded part can be iterated; see `phase3/unbounded_iteration.md`.)

```py
from ty_extensions import Iter

def _(
    a: tuple[*[t for t in Iter[int]]],  # error: [invalid-type-form]
    b: tuple[*[t for t in Iter[list[int]]]],  # error: [invalid-type-form]
    c: tuple[*[t for t in Iter[tuple[int] | tuple[str]]]],  # error: [invalid-type-form]
):
    reveal_type(a)  # revealed: Unknown
    reveal_type(c)  # revealed: Unknown
```

## Unsupported comprehension shapes

Each `for` clause must bind a single name as its target. (Several `for` clauses are supported; see
`phase3/multi_for.md`.)

```py
from ty_extensions import Iter

def _(
    # error: [invalid-type-form]
    b: tuple[*[k for k, v in Iter[tuple[tuple[int, str]]]]],
):
    reveal_type(b)  # revealed: Unknown
```

A comprehension must be unpacked with `*`:

```py
from ty_extensions import Iter

def _(
    a: tuple[[t for t in Iter[tuple[int]]]],  # error: [invalid-type-form]
):
    pass
```

## Comprehensions in value-indexed aliases

The values of implicit type aliases and of `X: TypeAlias = ...` are indexed as value expressions, so
type comprehensions in them are not supported, and the alias is `Unknown`. This also holds in stubs.

```py
from typing import TypeAlias
from ty_extensions import Iter

X = tuple[*[x for x in Iter[tuple[int]]]]  # error: [invalid-type-form]
Y: TypeAlias = tuple[*[x for x in Iter[tuple[int]]]]  # error: [invalid-type-form]

def _(x: X, y: Y):
    reveal_type(x)  # revealed: Unknown
    reveal_type(y)  # revealed: Unknown

from stub import Z, W

def _(z: Z, w: W): ...
```

`stub.pyi`:

```pyi
from typing import TypeAlias
from ty_extensions import Iter

Z = tuple[*[x for x in Iter[tuple[int]]]]  # error: [invalid-type-form]
W: TypeAlias = tuple[*[x for x in Iter[tuple[int]]]]  # error: [invalid-type-form]
```

## Filter conditions must be type booleans

```py
from ty_extensions import Iter

def _(
    a: tuple[*[t for t in Iter[tuple[int, str]] if int]],  # error: [invalid-type-form]
):
    reveal_type(a)  # revealed: Unknown
```

## Errors after specialization are silent

An error that only appears when a deferred operation is evaluated after specialization is not
reported, because specialization has no place to report it. The result is `Unknown`. The definition
of `Missing` is fine on its own; `Missing[A]` evaluates `GetMember[A, Literal["nope"]]`, which fails.

```py
from typing import Literal
from ty_extensions import GetMember

class A:
    x: int

type Missing[T] = GetMember[T, Literal["nope"]]

def _(a: Missing[A]):
    reveal_type(a)  # revealed: Unknown
```

[PEP 827]: https://peps.python.org/pep-0827/
