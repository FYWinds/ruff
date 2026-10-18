# TypeScript-style utility types

[PEP 827] shows how to write several of TypeScript's [utility types] with the new operators. The
definitions below are taken from the PEP. The expected results follow the reference implementation's
tests (`tests/test_ts_utility.py`), except where ty's union simplification differs (noted below).

```toml
[environment]
python-version = "3.14"
```

All examples below form a single test, so that they can share these definitions:

```py
from typing import Literal, TypedDict, Union
from ty_extensions import Attrs, FromUnion, IsAssignable, Iter, Member, Members, NewProtocol, NewTypedDict

# Pick<T, Keys>: the members of T whose names are in Keys.
type Pick[T, Keys] = NewProtocol[*[p for p in Iter[Members[T]] if IsAssignable[p.name, Keys]]]

# Omit<T, Keys>: the members of T whose names are not in Keys.
type Omit[T, Keys] = NewProtocol[*[p for p in Iter[Members[T]] if not IsAssignable[p.name, Keys]]]

# keyof T: the union of the member names of T.
type KeyOf[T] = Union[*[p.name for p in Iter[Members[T]]]]

# Exclude<T, U>: the elements of the union T that are not assignable to U.
type Exclude[T, U] = Union[*[x for x in Iter[FromUnion[T]] if not IsAssignable[x, U]]]

# Extract<T, U>: the elements of the union T that are assignable to U.
type Extract[T, U] = Union[*[x for x in Iter[FromUnion[T]] if IsAssignable[x, U]]]

# Partial<T>: every attribute of T becomes optional (`T | None`).
type Partial[T] = NewProtocol[*[Member[p.name, p.type | None, p.quals] for p in Iter[Attrs[T]]]]

# Partial<T> for TypedDicts: every key becomes `NotRequired`.
type PartialTD[T] = NewTypedDict[
    *[Member[p.name, p.type, p.quals | Literal["NotRequired"]] for p in Iter[Attrs[T]]]
]

class Todo:
    title: str
    description: str
    completed: bool

class TodoTD(TypedDict):
    title: str
    description: str
    completed: bool
```

`Pick` and `Omit` build a protocol from a subset of the members:

```py
from ty_extensions import IsEquivalent, static_assert
from ty_extensions._internal import is_assignable_to

type TitleAndCompleted = NewProtocol[Member[Literal["title"], str], Member[Literal["completed"], bool]]

def _(
    picked: Pick[Todo, Literal["title", "completed"]],
    omitted: Omit[Todo, Literal["description"]],
    same: IsEquivalent[Pick[Todo, Literal["title", "completed"]], TitleAndCompleted],
    same_omit: IsEquivalent[Omit[Todo, Literal["description"]], TitleAndCompleted],
):
    reveal_type(picked.title)  # revealed: str
    reveal_type(picked.completed)  # revealed: bool
    picked.description  # error: [unresolved-attribute]

    reveal_type(omitted.title)  # revealed: str
    reveal_type(omitted.completed)  # revealed: bool
    omitted.description  # error: [unresolved-attribute]

    reveal_type(same)  # revealed: Literal[True]
    reveal_type(same_omit)  # revealed: Literal[True]

static_assert(is_assignable_to(Todo, Pick[Todo, Literal["title"]]))
static_assert(is_assignable_to(Todo, Omit[Todo, Literal["title"]]))
```

`KeyOf` is the union of the member names:

```py
def _(keys: KeyOf[Todo], td_keys: KeyOf[TodoTD]):
    reveal_type(keys)  # revealed: Literal["title", "description", "completed"]
    reveal_type(td_keys)  # revealed: Literal["title", "description", "completed"]

def keys_of[T](obj: T) -> KeyOf[T]:
    raise NotImplementedError

def _(todo: Todo):
    reveal_type(keys_of(todo))  # revealed: Literal["title", "description", "completed"]
```

`Exclude` and `Extract` filter the elements of a union.

ty simplifies `str | int | bool` to `str | int`, because `bool` is a subtype of `int`. So
`Extract[str | int | bool, int]` is `int` in ty, where the reference implementation gives
`int | bool`. The results for `Exclude` are the same. A `float` annotation means `int | float`, so
excluding `int` leaves only instances of the `float` class itself, which ty displays as `float*`.

```py
def _(
    a: Exclude[str | int | bytes, int],
    b: Exclude[str | int | bool, int],
    c: Exclude[Literal["a", "b", "c"], Literal["a"]],
    d: Exclude[str | int | float, str | int],
    e: Exclude[int, int],
):
    reveal_type(a)  # revealed: str | bytes
    reveal_type(b)  # revealed: str
    reveal_type(c)  # revealed: Literal["b", "c"]
    reveal_type(d)  # revealed: float*
    reveal_type(e)  # revealed: Never

def _(
    a: Extract[str | int | bytes, int],
    b: Extract[str | int | bool, int],
    c: Extract[Literal["a", "b", "c"], Literal["a"]],
    d: Extract[str | int | float, str | int],
):
    reveal_type(a)  # revealed: int
    reveal_type(b)  # revealed: int
    reveal_type(c)  # revealed: Literal["a"]
    reveal_type(d)  # revealed: str | int
```

`Partial` makes every attribute optional. Note that unlike in TypeScript, `Todo` itself is not
assignable to `Partial[Todo]`: protocol attributes are mutable, so their types are invariant, and
`str` is not equivalent to `str | None`.

```py
from ty_extensions import static_assert
from ty_extensions._internal import is_assignable_to

def _(partial: Partial[Todo]):
    reveal_type(partial.title)  # revealed: str | None
    reveal_type(partial.description)  # revealed: str | None
    reveal_type(partial.completed)  # revealed: bool | None

static_assert(not is_assignable_to(Todo, Partial[Todo]))
```

`Partial` also works on a `TypedDict`. The `NotRequired` qualifier of a key carries over to the
member, and `NewProtocol` ignores it.

```py
from typing import NotRequired

class Draft(TypedDict):
    title: str
    due: NotRequired[int]

def _(partial: Partial[Draft]):
    reveal_type(partial.title)  # revealed: str | None
    reveal_type(partial.due)  # revealed: int | None
```

`PartialTD` makes every key of a `TypedDict` non-required:

```py
def _(partial: PartialTD[TodoTD]):
    reveal_type(partial.get("title"))  # revealed: str | None

empty: PartialTD[TodoTD] = {}
some: PartialTD[TodoTD] = {"completed": True}
# error: [invalid-argument-type]
wrong: PartialTD[TodoTD] = {"completed": "yes"}
```

[PEP 827]: https://peps.python.org/pep-0827/
[utility types]: https://www.typescriptlang.org/docs/handbook/utility-types.html
