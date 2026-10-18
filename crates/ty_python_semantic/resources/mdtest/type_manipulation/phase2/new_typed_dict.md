# `NewTypedDict`

`NewTypedDict[*Ms]` creates a `TypedDict` with one item per `Member`. An item is required unless the
member has the `NotRequired` qualifier, and read-only if it has the `ReadOnly` qualifier. Other
qualifiers, the initializer and the definer are ignored.

The synthesized `TypedDict` has no name. ty displays it as `<TypedDict with items ...>`, listing the
keys alphabetically; the tests below check its structure by assigning dictionary literals instead.

```toml
[environment]
python-version = "3.14"
```

## Required and non-required keys

```py
from typing import Literal
from ty_extensions import Member, NewTypedDict

type Movie = NewTypedDict[
    Member[Literal["name"], str],
    Member[Literal["year"], int, Literal["NotRequired"]],
]

def _(movie: Movie):
    reveal_type(movie)  # revealed: <TypedDict with items 'name', 'year'>
    reveal_type(movie["name"])  # revealed: str
    reveal_type(movie.get("year"))  # revealed: int | None

ok1: Movie = {"name": "Alien", "year": 1979}
ok2: Movie = {"name": "Alien"}
# error: [missing-typed-dict-key]
missing: Movie = {"year": 1979}
# error: [invalid-argument-type]
wrong_type: Movie = {"name": 1}
# error: [invalid-key]
extra: Movie = {"name": "Alien", "director": "Scott"}
```

## Read-only keys

```py
from typing import Literal
from ty_extensions import Member, NewTypedDict

type Config = NewTypedDict[
    Member[Literal["host"], str, Literal["ReadOnly"]],
    Member[Literal["port"], int],
]

def _(config: Config):
    config["port"] = 80
    config["host"] = "localhost"  # error: [invalid-assignment]
```

## Ignored member components

`ClassVar`, `Final`, the initializer and the definer have no effect on the `TypedDict`.

```py
from typing import Literal, Never
from ty_extensions import IsEquivalent, Member, NewTypedDict

class A: ...

type Plain = NewTypedDict[Member[Literal["a"], int]]

def _(
    a: IsEquivalent[NewTypedDict[Member[Literal["a"], int, Literal["ClassVar", "Final"]]], Plain],
    b: IsEquivalent[NewTypedDict[Member[Literal["a"], int, Never, Literal[1], A]], Plain],
):
    reveal_type(a)  # revealed: Literal[True]
    reveal_type(b)  # revealed: Literal[True]
```

## Copying a TypedDict

Iterating over the `Attrs` of a `TypedDict` and building a new one gives a structurally equivalent
`TypedDict`, including required, non-required and read-only keys.

```py
from typing import NotRequired, ReadOnly, TypedDict
from ty_extensions import Attrs, Iter, NewTypedDict, static_assert
from ty_extensions._internal import is_assignable_to

class Movie(TypedDict):
    name: str
    year: NotRequired[int]
    id: ReadOnly[int]

type Copy[T] = NewTypedDict[*[m for m in Iter[Attrs[T]]]]

static_assert(is_assignable_to(Movie, Copy[Movie]))
static_assert(is_assignable_to(Copy[Movie], Movie))

ok: Copy[Movie] = {"name": "Alien", "id": 1}
# error: [missing-typed-dict-key]
missing: Copy[Movie] = {"year": 1979, "id": 1}

def _(movie: Copy[Movie]):
    movie["id"] = 2  # error: [invalid-assignment]
```

## From the attributes of a class

The attributes of an ordinary class can be turned into a `TypedDict`. Their `ClassVar` and `Final`
qualifiers are dropped.

```py
from typing import ClassVar, Final
from ty_extensions import Attrs, Iter, NewTypedDict

class User:
    name: str
    age: int = 0
    kind: ClassVar[str] = "user"
    version: Final[int] = 1

type AsDict[T] = NewTypedDict[*[m for m in Iter[Attrs[T]]]]

ok: AsDict[User] = {"name": "a", "age": 1, "kind": "x", "version": 2}
# error: [missing-typed-dict-key]
missing: AsDict[User] = {"name": "a", "age": 1, "kind": "x"}
```

## Duplicate names

If two members have the same name, the later one wins.

```py
from typing import Literal
from ty_extensions import Member, NewTypedDict

def _(td: NewTypedDict[Member[Literal["a"], int], Member[Literal["a"], str]]):
    reveal_type(td["a"])  # revealed: str
```

## Dynamic members

If the members come from iterating `Any`, the `TypedDict` is `Any`.

```py
from typing import Any
from ty_extensions import Iter, NewTypedDict

def _(td: NewTypedDict[*[m for m in Iter[Any]]]):
    reveal_type(td)  # revealed: Any
```

## Deferred evaluation

```py
from typing import TypedDict
from ty_extensions import Attrs, Iter, NewTypedDict

class Point(TypedDict):
    x: int

def f[T](x: T, td: NewTypedDict[*[m for m in Iter[Attrs[T]]]]):
    reveal_type(td)  # revealed: NewTypedDict[*[m for m in Iter[Attrs[T@f]]]]

def _(td: NewTypedDict[*[m for m in Iter[Attrs[Point]]]]):
    reveal_type(td["x"])  # revealed: int
```
