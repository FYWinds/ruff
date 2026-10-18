# PEP 827 playground examples

This file is both an mdtest and the source of the local playground's "Examples" menu. Every `##`
section is one example: the `[//]: # (playground: ...)` line right after the heading holds its
metadata (a Markdown link-reference comment, which mdtest treats as prose), the prose up to the
first code block is the description shown in the playground, the `toml` block becomes `ty.json`,
and the unlabeled `py` blocks become `main.py`.

`ty_extensions` does not exist at runtime, so examples only import it under `if TYPE_CHECKING:` and
rely on Python 3.14's deferred evaluation of annotations.

## Intersections and negations

[//]: # (playground: slug=intersection-not, phase=0, ts="A & B")

ty already supports two type constructors that Python itself cannot express: `Intersection[A, B]`
(a value that is both an `A` and a `B`) and `Not[T]` (any value that is not a `T`). They are
available from `ty_extensions`, and ty also creates them on its own when narrowing.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ty_extensions import Intersection, Not

class Walker: ...
class Swimmer: ...

def f(both: Intersection[Walker, Swimmer], only: Intersection[Walker, Not[Swimmer]]):
    reveal_type(both)  # revealed: Walker & Swimmer
    reveal_type(only)  # revealed: Walker & ~Swimmer

def g(value: Not[Not[int]]):
    reveal_type(value)  # revealed: int

def h(value: Walker):
    if not isinstance(value, Swimmer):
        reveal_type(value)  # revealed: Walker & ~Swimmer
```

## Type booleans

[//]: # (playground: slug=type-booleans, phase=1, ts="A extends B ? true : false")

PEP 827 adds *type booleans*. `IsAssignable[T, S]`, `IsEquivalent[T, S]` and `Bool[T]` are type
expressions that evaluate to `Literal[True]` or `Literal[False]`. Unions are checked as a whole:
`int | str` is not assignable to `int`.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, Literal

if TYPE_CHECKING:
    from ty_extensions import Bool, IsAssignable, IsEquivalent

def f(
    a: IsAssignable[bool, int],
    b: IsAssignable[int | str, int],
    c: IsEquivalent[int | str, str | int],
    d: Bool[Literal[True]],
):
    reveal_type(a)  # revealed: Literal[True]
    reveal_type(b)  # revealed: Literal[False]
    reveal_type(c)  # revealed: Literal[True]
    reveal_type(d)  # revealed: Literal[True]
```

## Gradual types

[//]: # (playground: slug=gradual-types, phase=1, ts="any is assignable both ways")

The operators use ty's normal assignability rules, so `Any` is assignable to everything and
everything is assignable to `Any`. `IsEquivalent` therefore checks *consistency*:
`IsEquivalent[Any, int]` is true. `Bool[Any]` is false, because `Any` is also assignable to `Never`.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from ty_extensions import Bool, IsAssignable, IsEquivalent

def f(
    a: IsAssignable[Any, int],
    b: IsAssignable[int, Any],
    c: IsEquivalent[Any, int],
    d: Bool[Any],
):
    reveal_type(a)  # revealed: Literal[True]
    reveal_type(b)  # revealed: Literal[True]
    reveal_type(c)  # revealed: Literal[True]
    reveal_type(d)  # revealed: Literal[False]
```

## Conditional types

[//]: # (playground: slug=conditional-basic, phase=1, ts="T extends U ? X : Y")

A conditional type `X if <type boolean> else Y` picks `X` when the condition is `Literal[True]` and
`Y` otherwise. With concrete operands it is evaluated immediately, anywhere a type expression is
allowed.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ty_extensions import IsAssignable

def f(
    a: int if IsAssignable[bool, int] else bytes,
    b: int if IsAssignable[str, int] else bytes,
    c: list[str if IsAssignable[bool, int] else None],
):
    reveal_type(a)  # revealed: int
    reveal_type(b)  # revealed: bytes
    reveal_type(c)  # revealed: list[str]
```

## Combining conditions

[//]: # (playground: slug=bool-combinators, phase=1, ts="A extends X ? B extends Y ? T : F : F")

`not`, `and` and `or` combine type booleans. They short-circuit from left to right, so a result is
known as soon as possible, even if a later operand still depends on an unsolved type variable.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ty_extensions import IsAssignable

def f[T](
    x: T,
    a: not IsAssignable[str, int],
    b: IsAssignable[bool, int] and IsAssignable[int, bool],
    c: IsAssignable[str, int] and IsAssignable[T, int],
    d: IsAssignable[bool, int] or IsAssignable[T, int],
):
    reveal_type(a)  # revealed: Literal[True]
    reveal_type(b)  # revealed: Literal[False]
    reveal_type(c)  # revealed: Literal[False]
    reveal_type(d)  # revealed: Literal[True]
```

## Generic conditional aliases

[//]: # (playground: slug=generic-alias, phase=1, ts="type Pick<T> = T extends string ? number : Uint8Array")

When a condition mentions a type parameter, it cannot be decided yet. ty keeps it unevaluated and
decides it once the alias is specialized.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ty_extensions import IsAssignable

type Pick[T] = int if IsAssignable[T, str] else bytes

def f(a: Pick[str], b: Pick[float], c: list[Pick[str]]):
    reveal_type(a)  # revealed: int
    reveal_type(b)  # revealed: bytes
    reveal_type(c[0])  # revealed: int
```

## No distribution over unions

[//]: # (playground: slug=no-distribution, phase=1, ts="[T] extends [string] ? number : Uint8Array")

Unlike TypeScript's distributive conditional types, a conditional type is evaluated once for the
whole type argument. `str | int` is not assignable to `str`, so `Pick[str | int]` is `bytes`, not
`int | bytes`.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ty_extensions import IsAssignable

type Pick[T] = int if IsAssignable[T, str] else bytes

def f(a: Pick[str | int], b: Pick[str | str]):
    reveal_type(a)  # revealed: bytes
    reveal_type(b)  # revealed: int
```

## Aliases as conditions

[//]: # (playground: slug=alias-condition, phase=1, ts="type IsStr<T> = T extends string ? true : false")

Any type expression that evaluates to a type boolean can be used as a condition, including a
(generic) type alias. This makes it easy to name and reuse predicates.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ty_extensions import IsAssignable

type IsStr[T] = IsAssignable[T, str]
type Pick[T] = int if IsStr[T] else bytes

def f(a: IsStr[str], b: Pick[str], c: Pick[bytes]):
    reveal_type(a)  # revealed: Literal[True]
    reveal_type(b)  # revealed: int
    reveal_type(c)  # revealed: bytes
```

## Conditional return types

[//]: # (playground: slug=generic-function, phase=1, ts="return type T extends string ? number : Uint8Array")

A conditional return type is evaluated at each call site, using the type inferred for `T`. One
signature can replace a group of `@overload`s.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ty_extensions import IsAssignable

def parse[T](raw: T) -> int if IsAssignable[T, str] else bytes:
    raise NotImplementedError

def f(either: str | int):
    reveal_type(parse("42"))  # revealed: int
    reveal_type(parse(42))  # revealed: bytes
    reveal_type(parse(either))  # revealed: bytes
```

## Limitation inside generic bodies

[//]: # (playground: slug=body-limitation, phase=1, ts="same limitation: use a cast")

Inside the generic function the condition is unknown, so ty checks a returned value against *both*
branches: `1` is an `int` but not `bytes`, so it is rejected. TypeScript has the same limitation.
Use `cast` when you know better.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, cast

if TYPE_CHECKING:
    from ty_extensions import IsAssignable

type Result[T] = int if IsAssignable[T, str] else bytes

def strict[T](raw: T) -> Result[T]:
    return 1  # error: [invalid-return-type]

def relaxed[T](raw: T) -> Result[T]:
    return cast(Result[T], 1)

def f():
    reveal_type(relaxed("a"))  # revealed: int
```

## Invalid usage

[//]: # (playground: slug=invalid-usage, phase=1)

The condition of a conditional type, and the operands of `not`, `and` and `or`, must be type
booleans. The operators also check how many arguments they receive.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ty_extensions import Bool, IsAssignable

def f(
    # error: [invalid-type-form] "Condition of a conditional type must be a type boolean"
    a: int if str else bytes,
    # error: [invalid-type-form] "Operands of `and`/`or`/`not` in a type expression must be type booleans"
    b: not int,
    c: IsAssignable[int],  # error: [invalid-type-form]
    d: Bool[int, str],  # error: [invalid-type-form]
):
    pass
```

## Mapping over a tuple

[//]: # (playground: slug=tuple-map, phase=2, ts="type Lists<T extends unknown[]> = { [I in keyof T]: T[I][] }")

A type comprehension `*[... for t in Iter[...]]` maps every element of a tuple type to a new type,
and `if` clauses filter the elements. The result can be spliced into `tuple[...]` or `Union[...]`.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, Union

if TYPE_CHECKING:
    from ty_extensions import IsAssignable, Iter

type Lists[T] = tuple[*[list[t] for t in Iter[T]]]
type Numbers[T] = Union[*[t for t in Iter[T] if IsAssignable[t, int | float]]]

def f(a: Lists[tuple[int, str]], b: Numbers[tuple[int, str, float, bytes]]):
    reveal_type(a)  # revealed: tuple[list[int], list[str]]
    # A `float` annotation already means `int | float`, so ty displays this union as `float`.
    reveal_type(b)  # revealed: float
```

## Inspecting attributes

[//]: # (playground: slug=attrs-introspection, phase=2, ts="keyof T and T[K], but as a tuple of members")

`Attrs[T]` is a tuple with one `Member` per annotated attribute of a class: its name, type,
qualifiers, initializer and defining class. `Members[T]` adds the methods, after the attributes.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, ClassVar

if TYPE_CHECKING:
    from ty_extensions import Attrs, Members

class Point:
    x: int
    y: int = 0
    dims: ClassVar[int] = 2

    def norm(self) -> float:
        return (self.x**2 + self.y**2) ** 0.5

def f(attrs: Attrs[Point], members: Members[Point]):
    # revealed: tuple[Member[Literal["x"], int, Never, Never, Point], Member[Literal["y"], int, Never, Literal[0], Point], Member[Literal["dims"], int, Literal["ClassVar"], Literal[2], Point]]
    reveal_type(attrs)
    reveal_type(len(members))  # revealed: Literal[4]
    reveal_type(members[3].name)  # revealed: Literal["norm"]
```

## Member components

[//]: # (playground: slug=member-access, phase=2, ts="{ [K in keyof T]: K }")

Inside a comprehension, `m.name`, `m.type`, `m.quals`, `m.init` and `m.definer` read the components
of a `Member`. `GetMember[T, Literal["x"]]` looks up a single member by name.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, Literal

if TYPE_CHECKING:
    from ty_extensions import Attrs, GetMember, Iter

class User:
    id: int
    name: str = "anonymous"

type Names[T] = tuple[*[m.name for m in Iter[Attrs[T]]]]
type Types[T] = tuple[*[m.type for m in Iter[Attrs[T]]]]

def f(names: Names[User], types: Types[User], name: GetMember[User, Literal["name"]]):
    reveal_type(names)  # revealed: tuple[Literal["id"], Literal["name"]]
    reveal_type(types)  # revealed: tuple[int, str]
    reveal_type(name)  # revealed: Member[Literal["name"], str, Never, Literal["anonymous"], User]
```

## `keyof`

[//]: # (playground: slug=keyof, phase=2, ts="keyof T")

`KeyOf[T]` collects the names of all members into a union of string literals, using the variadic
`Union[*...]`.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, Union

if TYPE_CHECKING:
    from ty_extensions import Iter, Members

type KeyOf[T] = Union[*[p.name for p in Iter[Members[T]]]]

class Todo:
    title: str
    completed: bool

    def toggle(self) -> None:
        self.completed = not self.completed

def keys_of[T](obj: T) -> KeyOf[T]:
    raise NotImplementedError

def f(todo: Todo):
    reveal_type(keys_of(todo))  # revealed: Literal["title", "completed", "toggle"]
```

## Typed property access

[//]: # (playground: slug=typed-getter, phase=2, ts="function get<T, K extends keyof T>(obj: T, key: K): T[K]")

`GetMemberType[T, K]` is the type of the member named `K`. In a generic function, it is evaluated at
each call site with the inferred key. A missing key gives `Never`.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ty_extensions import GetMemberType

class User:
    name: str
    age: int

def get[T, K: str](obj: T, key: K) -> GetMemberType[T, K]:
    return getattr(obj, key)

def f(user: User):
    reveal_type(get(user, "name"))  # revealed: str
    reveal_type(get(user, "age"))  # revealed: int
```

## `Pick` and `Omit`

[//]: # (playground: slug=pick-omit, phase=2, ts="Pick<T, K> / Omit<T, K>")

`NewProtocol[*members]` creates a structural type from members. Filtering the members of a class by
name gives TypeScript's `Pick` and `Omit`. The class itself satisfies both.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, Literal

if TYPE_CHECKING:
    from ty_extensions import IsAssignable, Iter, Members, NewProtocol

type Pick[T, Keys] = NewProtocol[*[p for p in Iter[Members[T]] if IsAssignable[p.name, Keys]]]
type Omit[T, Keys] = NewProtocol[*[p for p in Iter[Members[T]] if not IsAssignable[p.name, Keys]]]

class Todo:
    title: str
    description: str
    completed: bool

def f(todo: Todo, preview: Pick[Todo, Literal["title", "completed"]], rest: Omit[Todo, Literal["title"]]):
    reveal_type(preview.title)  # revealed: str
    preview.description  # error: [unresolved-attribute]
    reveal_type(rest.description)  # revealed: str
    rest.title  # error: [unresolved-attribute]
    also_preview: Pick[Todo, Literal["title"]] = todo
```

## `Partial`

[//]: # (playground: slug=partial, phase=2, ts="Partial<T>")

Rebuilding each attribute with `Member[p.name, p.type | None, p.quals]` makes every attribute
optional. Unlike in TypeScript, `Todo` is not assignable to `Partial[Todo]`: protocol attributes are
mutable, so their types must match exactly.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ty_extensions import Attrs, Iter, Member, NewProtocol

type Partial[T] = NewProtocol[*[Member[p.name, p.type | None, p.quals] for p in Iter[Attrs[T]]]]

class Todo:
    title: str
    completed: bool

def f(todo: Todo, patch: Partial[Todo]):
    reveal_type(patch.title)  # revealed: str | None
    reveal_type(patch.completed)  # revealed: bool | None
    wrong: Partial[Todo] = todo  # error: [invalid-assignment]
```

## `Exclude` and `Extract`

[//]: # (playground: slug=exclude-extract, phase=2, ts="Exclude<T, U> / Extract<T, U>")

`FromUnion[T]` turns a union into a tuple, so its elements can be filtered and joined back with
`Union[*...]`. Note that ty simplifies `int | bool` to `int` before `FromUnion` sees it.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, Literal, Union

if TYPE_CHECKING:
    from ty_extensions import FromUnion, IsAssignable, Iter

type Exclude[T, U] = Union[*[x for x in Iter[FromUnion[T]] if not IsAssignable[x, U]]]
type Extract[T, U] = Union[*[x for x in Iter[FromUnion[T]] if IsAssignable[x, U]]]

def f(
    a: Exclude[str | int | bytes, int],
    b: Extract[str | int | bytes, int],
    c: Exclude[Literal["a", "b", "c"], Literal["a"]],
):
    reveal_type(a)  # revealed: str | bytes
    reveal_type(b)  # revealed: int
    reveal_type(c)  # revealed: Literal["b", "c"]
```

## Building a `TypedDict`

[//]: # (playground: slug=new-typed-dict, phase=2, ts="{ -readonly [K in keyof T]?: T[K] }")

`NewTypedDict[*members]` creates a `TypedDict`. A `NotRequired` qualifier makes a key optional, so
`PartialTD` turns every key of a `TypedDict` into an optional one. Dict literals are checked
against the result.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, Literal, TypedDict

if TYPE_CHECKING:
    from ty_extensions import Attrs, Iter, Member, NewTypedDict

type PartialTD[T] = NewTypedDict[
    *[Member[p.name, p.type, p.quals | Literal["NotRequired"]] for p in Iter[Attrs[T]]]
]

class Movie(TypedDict):
    title: str
    year: int

patch: PartialTD[Movie] = {"year": 1979}
empty: PartialTD[Movie] = {}
bad: PartialTD[Movie] = {"year": "1979"}  # error: [invalid-argument-type]
full: Movie = {"year": 1979}  # error: [missing-typed-dict-key]
```

## Properties only

[//]: # (playground: slug=props-only, phase=2, ts="{ [K in keyof T as T[K] extends Property<any> ? K : never]: T[K]['value'] }")

A variant of the PEP's Prisma-style `PropsOnly`: keep only the attributes whose type is a
`Property[...]`, and unwrap them with `GetMemberType` (the PEP's version uses `GetArg`, which comes
in phase 3).

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, Any, Literal

if TYPE_CHECKING:
    from ty_extensions import Attrs, GetMemberType, IsAssignable, Iter, Member, NewProtocol

class Property[T]:
    value: T

class Link: ...

class User:
    name: Property[str]
    age: Property[int]
    posts: Link

type PropsOnly[T] = NewProtocol[
    *[Member[p.name, GetMemberType[p.type, Literal["value"]]] for p in Iter[Attrs[T]] if IsAssignable[p.type, Property[Any]]]
]

def f(props: PropsOnly[User]):
    reveal_type(props.name)  # revealed: str
    reveal_type(props.age)  # revealed: int
    props.posts  # error: [unresolved-attribute]
```
