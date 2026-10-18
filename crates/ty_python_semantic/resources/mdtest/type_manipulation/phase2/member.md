# `Member` and dot access

[PEP 827] describes class members with `Member[N, T, Q, Init, D]`:

- `N` is the name, a string literal type (`.name`);
- `T` is the type of the member (`.type`);
- `Q` is a union of qualifiers from `Literal["ClassVar", "Final", "NotRequired", "ReadOnly"]`, or
  `Never` if there are none (`.quals`);
- `Init` is the type of the initializer in the class body, or `Never` if there is none (`.init`);
- `D` is the class that defines the member, or `Never` for `TypedDict` items (`.definer`).

`Member` is an ordinary generic class in `ty_extensions`. `Q`, `Init` and `D` default to `Never`.

```toml
[environment]
python-version = "3.14"
```

## Constructing members

ty displays all five type arguments, including defaulted ones.

```py
from typing import Literal, Never
from ty_extensions import Member

class A: ...

def _(
    a: Member[Literal["x"], int],
    b: Member[Literal["x"], int, Literal["ClassVar", "Final"]],
    c: Member[Literal["x"], int | None, Never, None, A],
):
    reveal_type(a)  # revealed: Member[Literal["x"], int, Never, Never, Never]
    reveal_type(b)  # revealed: Member[Literal["x"], int, Literal["ClassVar", "Final"], Never, Never]
    reveal_type(c)  # revealed: Member[Literal["x"], int | None, Never, None, A]
```

## Members are covariant

All five type arguments are exposed through read-only accessors, so `Member` is covariant in each of
them. Members with different names are unrelated.

```py
from typing import Literal
from ty_extensions import IsAssignable, Member

def _(
    a: IsAssignable[Member[Literal["x"], bool], Member[Literal["x"], int]],
    b: IsAssignable[Member[Literal["x"], int], Member[Literal["x"], bool]],
    c: IsAssignable[Member[Literal["x"], int], Member[Literal["y"], int]],
    d: IsAssignable[Member[Literal["x"], int], Member[str, int]],
):
    reveal_type(a)  # revealed: Literal[True]
    reveal_type(b)  # revealed: Literal[False]
    reveal_type(c)  # revealed: Literal[False]
    reveal_type(d)  # revealed: Literal[True]
```

## Reading the components of a member value

At runtime and in value expressions, the accessors are properties that return the corresponding type
argument.

```py
from typing import Literal
from ty_extensions import Member

class A: ...

def _(m: Member[Literal["x"], int, Literal["Final"], Literal[1], A]):
    reveal_type(m.name)  # revealed: Literal["x"]
    reveal_type(m.type)  # revealed: int
    reveal_type(m.quals)  # revealed: Literal["Final"]
    reveal_type(m.init)  # revealed: Literal[1]
    reveal_type(m.definer)  # revealed: A
```

## Dot access in type expressions

In a type expression, `m.name`, `m.type`, `m.quals`, `m.init` and `m.definer` evaluate to the
corresponding type argument of the `Member`.

```py
from typing import Literal
from ty_extensions import Member

class A: ...

type M = Member[Literal["x"], int, Literal["Final"], Literal[1], A]
type Plain = Member[Literal["y"], str]

def _(
    name: M.name,
    type_: M.type,
    quals: M.quals,
    init: M.init,
    definer: M.definer,
    no_quals: Plain.quals,
    no_init: Plain.init,
    no_definer: Plain.definer,
):
    reveal_type(name)  # revealed: Literal["x"]
    reveal_type(type_)  # revealed: int
    reveal_type(quals)  # revealed: Literal["Final"]
    reveal_type(init)  # revealed: Literal[1]
    reveal_type(definer)  # revealed: A
    reveal_type(no_quals)  # revealed: Never
    reveal_type(no_init)  # revealed: Never
    reveal_type(no_definer)  # revealed: Never
```

The left side does not have to be an alias; any type expression that evaluates to a `Member` works.

```py
from typing import Literal
from ty_extensions import GetMember, Member

class Owner:
    a: int

def _(
    name: Member[Literal["x"], int].name,
    type_: GetMember[Owner, Literal["a"]].type,
):
    reveal_type(name)  # revealed: Literal["x"]
    reveal_type(type_)  # revealed: int
```

## Dot access on comprehension variables

This is the usual way to use dot access: the loop variable of a type comprehension ranges over
`Member`s.

```py
from typing import Literal, Union
from ty_extensions import Iter, Member

type Ms = tuple[Member[Literal["a"], int], Member[Literal["b"], str, Literal["ClassVar"]]]

def _(
    names: tuple[*[m.name for m in Iter[Ms]]],
    types: tuple[*[m.type for m in Iter[Ms]]],
    quals: Union[*[m.quals for m in Iter[Ms]]],
):
    reveal_type(names)  # revealed: tuple[Literal["a"], Literal["b"]]
    reveal_type(types)  # revealed: tuple[int, str]
    # `Never` disappears from a union (a tuple containing `Never` would itself be `Never`).
    reveal_type(quals)  # revealed: Literal["ClassVar"]
```

## Dot access on type parameters

Dot access on a type variable is deferred until the type variable is specialized.

```py
from typing import Literal
from ty_extensions import Member

type NameOf[M] = M.name
type TypeOf[M] = M.type

def _(
    a: NameOf[Member[Literal["x"], int]],
    b: TypeOf[Member[Literal["x"], int]],
    c: list[TypeOf[Member[Literal["y"], bytes]]],
):
    reveal_type(a)  # revealed: Literal["x"]
    reveal_type(b)  # revealed: int
    reveal_type(c[0])  # revealed: bytes

def f[M](m: M, name: M.name):
    reveal_type(name)  # revealed: M@f.name
```

For a type variable bounded by `Member`, the access is evaluated once the type variable is solved at
a call site.

```py
from typing import Literal
from ty_extensions import Member

def get_name[M: Member[Literal["x"], int]](m: M) -> M.name:
    raise NotImplementedError

reveal_type(get_name(Member[Literal["x"], int]()))  # revealed: Literal["x"]
```

## `Any` and `Never`

Dot access on `Any` gives `Any`, and dot access on `Never` gives `Never`. Neither is an error.

```py
from typing import Any, Never

type A = Any
type N = Never

def _(a: A.name, b: A.type, c: N.name, d: N.type, e: Any.name, f: Never.type):
    reveal_type(a)  # revealed: Any
    reveal_type(b)  # revealed: Any
    reveal_type(e)  # revealed: Any
    reveal_type(c)  # revealed: Never
    reveal_type(d)  # revealed: Never
    reveal_type(f)  # revealed: Never
```

## Dot access is not lifted over unions

Unlike `Attrs` and the other inspection operators, dot access is not applied to each element of a
union. Using it on a union of members, or on anything else that is not a `Member`, is an error, and
the result is `Unknown`.

The reference implementation lifts dot access over unions; ty follows the PEP text here.

```py
from typing import Literal
from ty_extensions import Member

type Either = Member[Literal["a"], int] | Member[Literal["b"], str]

class A:
    name: str

def _(
    a: Either.name,  # error: [invalid-type-form]
    b: A.name,  # error: [invalid-type-form]
    c: int.type,  # error: [invalid-type-form]
):
    reveal_type(a)  # revealed: Unknown
    reveal_type(b)  # revealed: Unknown
    reveal_type(c)  # revealed: Unknown
```

## Unknown components

Only the five components exist, also on comprehension variables.

```py
from typing import Literal
from ty_extensions import Attrs, Iter, Member

class A:
    a: int

type M = Member[Literal["x"], int]

def _(a: M.default):  # error: [invalid-type-form]
    reveal_type(a)  # revealed: Unknown

def _(
    a: tuple[*[m.foo for m in Iter[Attrs[A]]]],  # error: [invalid-type-form]
):
    # Only the element type fails; each element becomes `Unknown`.
    reveal_type(a)  # revealed: tuple[Unknown]
```

## Other dotted names

Dotted names that do not access a `Member` component keep their usual meaning.

```py
import typing

def _(a: typing.List[int]):
    reveal_type(a)  # revealed: list[int]
```

[PEP 827]: https://peps.python.org/pep-0827/
