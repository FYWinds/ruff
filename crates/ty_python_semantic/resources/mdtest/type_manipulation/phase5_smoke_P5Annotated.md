# `GetAnnotations` and `DropAnnotations`

[PEP 827] lists two operators for `Annotated` as a potential extension:

- `GetAnnotations[T]` is the union of the metadata of `T` if `T` is `Annotated[X, m1, m2, ...]`, and
  `Never` otherwise. Literal metadata keeps its literal type; other metadata has the type of the
  value (`Hidden()` gives `Hidden`).
- `DropAnnotations[T]` removes the `Annotated` wrapper.

ty normally discards `Annotated` metadata. It only keeps it in the type argument `T` of the `Member`s
returned by `Attrs`, `Members` and `GetMember`, for attributes whose annotation is `Annotated[...]`.
The operators can read metadata from three forms, recognized syntactically: `Annotated[...]` written
directly, `X.type` where `X` is a `Member`, and `GetMemberType[T, S]`. Everything else that reads the
type of a member (dot access in other positions, building new types) gets the type without the
metadata.

```toml
[environment]
python-version = "3.14"
```

## Examples from the PEP

```py
from typing import Annotated
from ty_extensions import DropAnnotations, GetAnnotations

class Hidden: ...

def _(
    a: GetAnnotations[Annotated[int, "xxx"]],
    b: GetAnnotations[Annotated[int, "xxx", 5]],
    c: GetAnnotations[int],
    d: DropAnnotations[Annotated[int, "xxx"]],
    e: DropAnnotations[Annotated[int, "xxx", 5]],
    f: DropAnnotations[int],
    g: GetAnnotations[Annotated[str, Hidden()]],
    h: GetAnnotations["Annotated[int, 'quoted']"],
):
    reveal_type(a)  # revealed: Literal["xxx"]
    reveal_type(b)  # revealed: Literal["xxx", 5]
    reveal_type(d)  # revealed: int
    reveal_type(e)  # revealed: int
    reveal_type(f)  # revealed: int
    reveal_type(g)  # revealed: Hidden
    reveal_type(h)  # revealed: Literal["quoted"]
    # `Never` is revealed last: the code after it is unreachable.
    reveal_type(c)  # revealed: Never
```

## Not lifted over unions

Like in the reference implementation, the operators look at the union as a whole, which is not an
`Annotated` type.

```py
from typing import Annotated
from ty_extensions import DropAnnotations, GetAnnotations

def _(
    a: GetAnnotations[Annotated[int, "x"] | str],
    b: DropAnnotations[Annotated[int, "x"] | str],
):
    reveal_type(b)  # revealed: int | str
    reveal_type(a)  # revealed: Never
```

## Metadata of class attributes

### Reading the metadata

`GetAnnotations` reads the metadata through `X.type` and `GetMemberType[T, S]`, including on the
variable of a type comprehension.

```py
from typing import Annotated, Literal, Union
from ty_extensions import Attrs, GetAnnotations, GetMember, GetMemberType, IsAssignable, Iter

class Hidden: ...
class Primary: ...

class Hero:
    id: Annotated[int, Primary()]
    name: str
    secret: Annotated[str, Hidden(), "doc"]

type Visible[T] = Union[*[p.name for p in Iter[Attrs[T]] if not IsAssignable[Hidden, GetAnnotations[p.type]]]]
type Id = GetMember[Hero, Literal["id"]]
type Name = GetMember[Hero, Literal["name"]]

def _(
    visible: Visible[Hero],
    id_meta: GetAnnotations[Id.type],
    secret_meta: GetAnnotations[GetMemberType[Hero, Literal["secret"]]],
    name_meta: GetAnnotations[Name.type],
):
    reveal_type(visible)  # revealed: Literal["id", "name"]
    reveal_type(id_meta)  # revealed: Primary
    reveal_type(secret_meta)  # revealed: Hidden | Literal["doc"]
    reveal_type(name_meta)  # revealed: Never
```

### Metadata is invisible otherwise

The member types are displayed and compared like the plain types, so the results of phase 2 do not
change.

```py
from typing import Annotated, Literal
from ty_extensions import Attrs, DropAnnotations, GetMember, IsEquivalent, Iter

class Hero:
    id: Annotated[int, "pk"]
    secret: Annotated[str, "doc"]

type Plain[T] = tuple[*[DropAnnotations[p.type] for p in Iter[Attrs[T]]]]
type Types[T] = tuple[*[p.type for p in Iter[Attrs[T]]]]
type Secret = GetMember[Hero, Literal["secret"]]

def _(
    attrs: Attrs[Hero],
    plain: Plain[Hero],
    types: Types[Hero],
    secret_type: Secret.type,
    same: IsEquivalent[Secret.type, str],
):
    # revealed: tuple[Member[Literal["id"], int, Never, Never, Hero], Member[Literal["secret"], str, Never, Never, Hero]]
    reveal_type(attrs)
    reveal_type(plain)  # revealed: tuple[int, str]
    reveal_type(types)  # revealed: tuple[int, str]
    reveal_type(secret_type)  # revealed: str
    reveal_type(same)  # revealed: Literal[True]
```

### Utility types still work

A class with `Annotated` attributes can be used with the comprehension-based utility types of phase 2:
reading `p.type` gives the plain type, so nothing is left unevaluated.

```py
from typing import Annotated, Literal
from ty_extensions import Attrs, Iter, Member, NewProtocol

class Hero:
    id: Annotated[int, "pk"]
    name: str

type Partial[T] = NewProtocol[*[Member[p.name, p.type | None, p.quals] for p in Iter[Attrs[T]]]]

def _(partial: Partial[Hero]):
    reveal_type(partial.id)  # revealed: int | None
    reveal_type(partial.name)  # revealed: str | None
```

## Metadata is dropped when building new types

`NewProtocol`, `NewTypedDict`, `Params` and `UpdateClass` drop the metadata of member types, so the
metadata does not leak into ordinary types. Synthesized `TypedDict`s can be inspected with
`GetMemberType`, which shows that the metadata is gone:

```py
from typing import Annotated, Literal
from ty_extensions import Attrs, GetAnnotations, GetMemberType, Iter, NewTypedDict

class Hidden: ...

class Hero:
    secret: Annotated[str, Hidden()]

type AsDict[T] = NewTypedDict[*[p for p in Iter[Attrs[T]]]]

def _(
    copied_type: GetMemberType[AsDict[Hero], Literal["secret"]],
    copied_meta: GetAnnotations[GetMemberType[AsDict[Hero], Literal["secret"]]],
):
    reveal_type(copied_type)  # revealed: str
    reveal_type(copied_meta)  # revealed: Never
```

## Other forms do not see the metadata

Only the three forms above are recognized. A type alias to an `Annotated` type, or a member type that
first goes through a helper alias, gives `Never`.

```py
from typing import Annotated, Literal
from ty_extensions import GetAnnotations, GetMember

type Secret = Annotated[str, "secret"]
type TypeOf[M] = M.type

class Hero:
    secret: Annotated[str, "doc"]

type S = GetMember[Hero, Literal["secret"]]

def _(a: GetAnnotations[Secret]):
    reveal_type(a)  # revealed: Never

def _(b: GetAnnotations[TypeOf[S]]):
    reveal_type(b)  # revealed: Never
```

[PEP 827]: https://peps.python.org/pep-0827/

## Smoke: utility types over classes with `Annotated` attributes

`Pick` / `Omit` / `Partial` still evaluate eagerly: the metadata never reaches the synthesized
protocol.

```py
from typing import Annotated, Literal
from ty_extensions import Attrs, IsAssignable, Iter, Member, Members, NewProtocol, NewTypedDict

class Hero:
    id: Annotated[int, "pk"]
    name: Annotated[str, "doc", 1]
    age: int

type Pick[T, Keys] = NewProtocol[*[p for p in Iter[Members[T]] if IsAssignable[p.name, Keys]]]
type Omit[T, Keys] = NewProtocol[*[p for p in Iter[Members[T]] if not IsAssignable[p.name, Keys]]]
type Partial[T] = NewProtocol[*[Member[p.name, p.type | None, p.quals] for p in Iter[Attrs[T]]]]
type AsDict[T] = NewTypedDict[*[p for p in Iter[Attrs[T]]]]

def _(
    picked: Pick[Hero, Literal["id", "name"]],
    omitted: Omit[Hero, Literal["id"]],
    partial: Partial[Hero],
    as_dict: AsDict[Hero],
):
    reveal_type(picked)  # revealed: <Protocol with members 'id', 'name'>
    reveal_type(picked.id)  # revealed: int
    reveal_type(picked.name)  # revealed: str
    reveal_type(omitted.name)  # revealed: str
    reveal_type(partial.id)  # revealed: int | None
    reveal_type(as_dict["name"])  # revealed: str
```

## Smoke: generic classes and deferred reads

```py
from typing import Annotated, Literal
from ty_extensions import Attrs, DropAnnotations, GetAnnotations, GetMember, GetMemberType, Iter

class Box[T]:
    item: Annotated[T, "item"]
    plain: T

type Metas[C] = tuple[*[GetAnnotations[p.type] for p in Iter[Attrs[C]]]]
type Plains[C] = tuple[*[DropAnnotations[p.type] for p in Iter[Attrs[C]]]]
type ItemMeta[C] = GetAnnotations[GetMemberType[C, Literal["item"]]]
type ItemType[C] = DropAnnotations[GetMember[C, Literal["item"]].type]

def _(
    metas: Metas[Box[int]],
    plains: Plains[Box[int]],
    item_meta: ItemMeta[Box[bytes]],
    item_type: ItemType[Box[bytes]],
    attrs: Attrs[Box[str]],
):
    reveal_type(metas)  # revealed: tuple[Literal["item"], Never]
    reveal_type(plains)  # revealed: tuple[int, int]
    reveal_type(item_meta)  # revealed: Literal["item"]
    reveal_type(item_type)  # revealed: bytes
    # revealed: tuple[Member[Literal["item"], str, Never, Never, Box[str]], Member[Literal["plain"], str, Never, Never, Box[str]]]
    reveal_type(attrs)

def f[C](x: ItemMeta[C], y: GetAnnotations[GetMember[C, Literal["item"]].type]):
    reveal_type(x)  # revealed: GetAnnotations[GetMemberType[C@f, Literal["item"]]]
    reveal_type(y)  # revealed: GetAnnotations[GetMember[C@f, Literal["item"]].type]
```

## Smoke: value-level access to the member type

The `type` property of a `Member` value gives the plain type.

```py
from typing import Annotated
from ty_extensions import Attrs

class Hero:
    id: Annotated[int | str, "pk"]

def _(attrs: Attrs[Hero]):
    t = attrs[0].type
    reveal_type(t)  # revealed: int | str
    if isinstance(t, int):
        reveal_type(t)  # revealed: int
```

## Smoke: invalid uses

```py
from typing import Annotated
from ty_extensions import DropAnnotations, GetAnnotations

# error: [invalid-type-form]
def _(a: GetAnnotations[int, str]): ...
def _(b: GetAnnotations[Annotated[int, "x"]] | None, c: DropAnnotations["Annotated[bytes, 1]"]):
    reveal_type(b)  # revealed: Literal["x"] | None
    reveal_type(c)  # revealed: bytes
```
