# Lifting over unions

`Attrs`, `Members`, `GetMember` and `GetMemberType` are *lifted* over unions: when an argument is a
union, the operator is applied to every combination of union elements (the cross product of all
arguments), and the results are joined in a union. A `Literal` with several values counts as a union.
If any argument is `Never` (the empty union), there are no combinations and the result is `Never`.

Type booleans, conditional types, `FromUnion`, `NewProtocol`, `NewTypedDict` and dot access on
members are not lifted.

```toml
[environment]
python-version = "3.14"
```

## `GetMemberType`

Both arguments are lifted. `B` has no `y`, which contributes `Never` to the union.

```py
from typing import Literal
from ty_extensions import GetMemberType

class A:
    x: int
    y: str

class B:
    x: bytes

def _(
    a: GetMemberType[A | B, Literal["x"]],
    b: GetMemberType[A, Literal["x", "y"]],
    c: GetMemberType[A | B, Literal["x", "y"]],
    d: GetMemberType[A | B, Literal["y"]],
):
    reveal_type(a)  # revealed: int | bytes
    reveal_type(b)  # revealed: int | str
    reveal_type(c)  # revealed: int | str | bytes
    reveal_type(d)  # revealed: str
```

## `GetMember`

A missing member is an error for `GetMember`, even if only one combination lacks it. The whole
operation then evaluates to `Unknown`.

```py
from typing import Literal
from ty_extensions import GetMember

class A:
    x: int
    y: str

class B:
    x: bytes

def _(
    a: GetMember[A | B, Literal["x"]],
    b: GetMember[A, Literal["x", "y"]],
    c: GetMember[A | B, Literal["y"]],  # error: [invalid-type-form]
):
    reveal_type(a)  # revealed: Member[Literal["x"], int, Never, Never, A] | Member[Literal["x"], bytes, Never, Never, B]
    reveal_type(b)  # revealed: Member[Literal["x"], int, Never, Never, A] | Member[Literal["y"], str, Never, Never, A]
    reveal_type(c)  # revealed: Unknown
```

## `Attrs` and `Members`

The result is a union of tuples.

```py
from ty_extensions import Attrs, Members

class A:
    x: int

class B:
    y: str

    def f(self) -> None:
        pass

def _(a: Attrs[A | B], m: Members[A | B]):
    reveal_type(a)  # revealed: tuple[Member[Literal["x"], int, Never, Never, A]] | tuple[Member[Literal["y"], str, Never, Never, B]]
    reveal_type(len(m))  # revealed: Literal[1, 2]
```

## `Never`

```py
from typing import Literal, Never
from ty_extensions import Attrs, GetMember, GetMemberType, Members

class A:
    x: int

def _(
    a: Attrs[Never],
    b: Members[Never],
    c: GetMember[Never, Literal["x"]],
    d: GetMemberType[A, Never],
):
    reveal_type(a)  # revealed: Never
    reveal_type(b)  # revealed: Never
    reveal_type(c)  # revealed: Never
    reveal_type(d)  # revealed: Never
```

## `Any` inside a union

An `Any` element gives `Any` for its combinations.

```py
from typing import Any, Literal
from ty_extensions import Attrs, GetMemberType

class A:
    x: int

def _(a: Attrs[A | Any], b: GetMemberType[A | Any, Literal["x"]]):
    reveal_type(a)  # revealed: tuple[Member[Literal["x"], int, Never, Never, A]] | Any
    reveal_type(b)  # revealed: int | Any
```

## Lifting after specialization

```py
from typing import Literal
from ty_extensions import GetMemberType

class A:
    x: int

class B:
    x: bytes

type XOf[T] = GetMemberType[T, Literal["x"]]

def _(a: XOf[A | B]):
    reveal_type(a)  # revealed: int | bytes
```

## Iterating a lifted result is an error

`Iter` requires a single tuple, not a union of tuples. A lifted `Attrs` over a union of classes
therefore cannot be iterated.

```py
from ty_extensions import Attrs, Iter

class A:
    x: int

class B:
    y: str

def _(
    a: tuple[*[m.name for m in Iter[Attrs[A | B]]]],  # error: [invalid-type-form]
):
    reveal_type(a)  # revealed: Unknown
```

## Operators that are not lifted

Type booleans compare the union as a whole, and `FromUnion` exists precisely to look at the union as
a whole. Dot access on a union of members is an error (see `member.md`).

```py
from ty_extensions import FromUnion, IsAssignable

def _(
    a: IsAssignable[int | str, int],
    b: FromUnion[int | str],
):
    reveal_type(a)  # revealed: Literal[False]
    reveal_type(b)  # revealed: tuple[int, str]
```

`NewProtocol` and `NewTypedDict` require each argument to be a `Member`, not a union of members:

```py
from typing import Literal
from ty_extensions import Member, NewProtocol, NewTypedDict

type Either = Member[Literal["a"], int] | Member[Literal["b"], str]

def _(
    a: NewProtocol[Either],  # error: [invalid-type-form]
    b: NewTypedDict[Either],  # error: [invalid-type-form]
):
    reveal_type(a)  # revealed: Unknown
    reveal_type(b)  # revealed: Unknown
```

## Too many combinations

Lifting evaluates at most 256 combinations. Beyond that, ty reports an error and the result is
`Unknown`. Here there are 17 × 16 = 272 combinations.

```py
from typing import Literal
from ty_extensions import GetMemberType

class C1: ...
class C2: ...
class C3: ...
class C4: ...
class C5: ...
class C6: ...
class C7: ...
class C8: ...
class C9: ...
class C10: ...
class C11: ...
class C12: ...
class C13: ...
class C14: ...
class C15: ...
class C16: ...
class C17: ...

type Classes = C1 | C2 | C3 | C4 | C5 | C6 | C7 | C8 | C9 | C10 | C11 | C12 | C13 | C14 | C15 | C16 | C17
type Keys = Literal["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p"]

def _(
    ok: GetMemberType[Classes, Literal["a"]],
    too_many: GetMemberType[Classes, Keys],  # error: [invalid-type-form]
):
    reveal_type(too_many)  # revealed: Unknown
    reveal_type(ok)  # revealed: Never
```
