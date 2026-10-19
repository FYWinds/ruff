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
`Property[...]`, and unwrap them with `GetArg`, which reads the type argument of the `Property`
base class. Unlike `GetMemberType[p.type, Literal["value"]]`, it does not need a `value` attribute.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, Any, Literal

if TYPE_CHECKING:
    from ty_extensions import Attrs, GetArg, IsAssignable, Iter, Member, NewProtocol

class Property[T]:
    value: T

class Link: ...

class User:
    name: Property[str]
    age: Property[int]
    posts: Link

type PropsOnly[T] = NewProtocol[
    *[Member[p.name, GetArg[p.type, Property, Literal[0]]] for p in Iter[Attrs[T]] if IsAssignable[p.type, Property[Any]]]
]

def f(props: PropsOnly[User]):
    reveal_type(props.name)  # revealed: str
    reveal_type(props.age)  # revealed: int
    props.posts  # error: [unresolved-attribute]
```

## Type arguments of a base class

[//]: # (playground: slug=get-arg, phase=3, ts="T extends Pointer<infer U> ? U : never")

`GetArg[T, Base, Idx]` reads type argument number `Idx` of `Base`, where `T` is `Base` itself or one
of its subclasses. This is `PointerArg` from the PEP's Prisma example: it works for every kind of
`Pointer`.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, Literal

if TYPE_CHECKING:
    from ty_extensions import GetArg

class Pointer[T]: ...
class Property[T](Pointer[T]): ...
class MultiLink[T](Pointer[T]): ...
class Post: ...

type PointerArg[T] = GetArg[T, Pointer, Literal[0]]

def f(
    a: PointerArg[Property[str]],
    b: PointerArg[MultiLink[Post]],
    c: GetArg[dict[str, bytes], dict, Literal[-1]],
):
    reveal_type(a)  # revealed: str
    reveal_type(b)  # revealed: Post
    reveal_type(c)  # revealed: bytes
```

## Tuple length

[//]: # (playground: slug=tuple-length, phase=3, ts="T['length']")

`Length[T]` is the length of a tuple type as an `int` literal type. A tuple with an unbounded part
has no fixed length, so its `Length` is `None`.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ty_extensions import Length

def f(
    a: Length[tuple[int, str]],
    b: Length[tuple[()]],
    c: Length[tuple[int, ...]],
    d: Length[tuple[int] | tuple[int, str]],
):
    reveal_type(a)  # revealed: Literal[2]
    reveal_type(b)  # revealed: Literal[0]
    reveal_type(c)  # revealed: None
    reveal_type(d)  # revealed: Literal[1, 2]
```

## Tuple slices and lifting

[//]: # (playground: slug=tuple-slice, phase=3, ts="T extends [...infer Init, any] ? Init : never")

`Slice[S, Start, End]` slices a tuple type like `s[start:end]`. Like most operators, it is *lifted*
over unions: each combination of union members is evaluated, and the results are joined. This is the
example from the PEP's section on lifting.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, Literal

if TYPE_CHECKING:
    from ty_extensions import Slice

class A: ...
class B: ...
class C: ...

type DropLast[T] = Slice[T, Literal[0], Literal[-1]]

def f(
    a: DropLast[tuple[A, B, C]],
    b: Slice[tuple[A, B, C], Literal[1], None],
    c: Slice[tuple[A, B, C], Literal[0, 1], Literal[2, 3]],
):
    reveal_type(a)  # revealed: tuple[A, B]
    reveal_type(b)  # revealed: tuple[B, C]
    reveal_type(c)  # revealed: tuple[A, B] | tuple[A, B, C] | tuple[B] | tuple[B, C]
```

## `any()` and `all()`

[//]: # (playground: slug=any-all, phase=3, ts="T[number] extends number ? true : false")

The builtins `any` and `all` work on type booleans, with a generator over `Iter[...]`. They can be
used wherever a type boolean can, for example as the condition of a conditional type.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, Literal

if TYPE_CHECKING:
    from ty_extensions import IsAssignable, Iter

type AllInts[T] = Literal["ints"] if all(IsAssignable[t, int] for t in Iter[T]) else Literal["mixed"]
type HasStr[T] = any(IsAssignable[t, str] for t in Iter[T])

def f(
    a: AllInts[tuple[int, bool]],
    b: AllInts[tuple[int, str]],
    c: HasStr[tuple[int, str]],
    d: HasStr[tuple[()]],
):
    reveal_type(a)  # revealed: Literal["ints"]
    reveal_type(b)  # revealed: Literal["mixed"]
    reveal_type(c)  # revealed: Literal[True]
    reveal_type(d)  # revealed: Literal[False]
```

## Several `for` clauses

[//]: # (playground: slug=multi-for, phase=3, ts="{ [A in X]: { [B in Y]: [A, B] }[Y] }[X]")

A type comprehension can have several `for` clauses. Later clauses can iterate over something that
depends on earlier variables, which makes flattening easy.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ty_extensions import Iter

type Product[X, Y] = tuple[*[tuple[a, b] for a in Iter[X] for b in Iter[Y]]]
type Flatten[T] = tuple[*[x for row in Iter[T] for x in Iter[row]]]

def f(
    a: Product[tuple[int, str], tuple[bytes, bool]],
    b: Flatten[tuple[tuple[int, str], tuple[()], tuple[bytes]]],
):
    # revealed: tuple[tuple[int, bytes], tuple[int, bool], tuple[str, bytes], tuple[str, bool]]
    reveal_type(a)
    reveal_type(b)  # revealed: tuple[int, str, bytes]
```

## Iterating unbounded tuples

[//]: # (playground: slug=unbounded-iter, phase=3, ts="{ [K in keyof T]: T[K] | null }")

`Iter` also accepts tuples with an unbounded part. The fixed elements are handled one by one; the
unbounded part is handled once and stays unbounded, or disappears if it is filtered out.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ty_extensions import IsAssignable, Iter

type Nullable[T] = tuple[*[x | None for x in Iter[T]]]
type IntsOnly[T] = tuple[*[x for x in Iter[T] if IsAssignable[x, int]]]

def f(
    a: Nullable[tuple[int, *tuple[str, ...], bool]],
    b: IntsOnly[tuple[int, *tuple[str, ...], bool]],
    c: IntsOnly[tuple[str, *tuple[int, ...]]],
):
    reveal_type(a)  # revealed: tuple[int | None, *tuple[str | None, ...], bool | None]
    reveal_type(b)  # revealed: tuple[int, bool]
    reveal_type(c)  # revealed: tuple[int, ...]
```

## A typed `zip`

[//]: # (playground: slug=zip-like, phase=3, ts="{ [K in keyof T]: T[K] extends Iterable<infer E> ? E : never }")

The `zip` example from the PEP: the return type maps every argument type to its element type. `*Ts`
is solved at the call site, and then the comprehension is evaluated. `ElemOf` finds the element type
through the `Iterable` base class (a protocol) of each argument. The result's elements are
`ElemOf[...]` aliases, so we unpack them to see the element types.

```toml
[environment]
python-version = "3.14"
```

```py
from collections.abc import Iterable, Iterator
from typing import TYPE_CHECKING, Literal

if TYPE_CHECKING:
    from ty_extensions import GetArg, Iter

type ElemOf[T] = GetArg[T, Iterable, Literal[0]]

def izip[*Ts](*args: *Ts) -> Iterator[tuple[*[ElemOf[t] for t in Iter[tuple[*Ts]]]]]:
    raise NotImplementedError

def f(a: list[int], b: tuple[str, str], c: dict[bytes, float]):
    for x, y, z in izip(a, b, c):
        reveal_type(x)  # revealed: int
        reveal_type(y)  # revealed: str
        reveal_type(z)  # revealed: bytes
```

## Recursive `Zip` with `RaiseError`

[//]: # (playground: slug=raise-error, phase=3, ts="no direct equivalent")

The PEP's `Zip` zips two tuple types by recursion: it drops the last element of both until they are
empty. If only one of them runs out, `RaiseError` reports an error where `Zip` is used. (The result
contains the helper aliases `Last[...]`, so we check it by assigning it rather than revealing it.)

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, Literal

if TYPE_CHECKING:
    from ty_extensions import Bool, GetArg, IsAssignable, Length, RaiseError, Slice

type DropLast[T] = Slice[T, Literal[0], Literal[-1]]
type Last[T] = GetArg[T, tuple, Literal[-1]]
type Empty[T] = IsAssignable[Length[T], Literal[0]]
type Zip[T, S] = (
    tuple[()]
    if Bool[Empty[T]] and Bool[Empty[S]]
    else RaiseError[Literal["Zip length mismatch"], T, S]
    if Bool[Empty[T]] or Bool[Empty[S]]
    else tuple[*Zip[DropLast[T], DropLast[S]], tuple[Last[T], Last[S]]]
)

def f(
    ok: Zip[tuple[int, str], tuple[float, bool]],
    bad: Zip[tuple[int, str], tuple[float]],  # error: [raised-type-error] "Zip length mismatch"
):
    pairs: tuple[tuple[int, float], tuple[str, bool]] = ok
    wrong: tuple[tuple[int, int], tuple[str, bool]] = ok  # error: [invalid-assignment]
```

## String case operators

[//]: # (playground: slug=capitalize, phase=4, ts="Uppercase<S> / Lowercase<S> / Capitalize<S> / Uncapitalize<S>")

`Uppercase`, `Lowercase`, `Capitalize` and `Uncapitalize` change the case of a string literal type.
They follow Python's string methods, with full Unicode case mapping. Note that `Capitalize` works
like `str.capitalize` and lowercases the rest of the string, unlike TypeScript's `Capitalize`.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, Literal

if TYPE_CHECKING:
    from ty_extensions import Capitalize, Lowercase, Uncapitalize, Uppercase

def f(
    a: Uppercase[Literal["straße"]],
    b: Lowercase[Literal["ΟΔΟΣ"]],
    c: Capitalize[Literal["fooBar"]],
    d: Uncapitalize[Literal["FooBar"]],
):
    reveal_type(a)  # revealed: Literal["STRASSE"]
    reveal_type(b)  # revealed: Literal["οδος"]
    reveal_type(c)  # revealed: Literal["Foobar"]
    reveal_type(d)  # revealed: Literal["fooBar"]
```

## Concatenation and unions

[//]: # (playground: slug=concat, phase=4, ts="`btn-${Size}-${Color}`")

`Concat[A, B]` concatenates two string literal types. Like TypeScript's template literal types, it is
applied to every combination of union members. A non-literal `str` argument gives `str`.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, Literal

if TYPE_CHECKING:
    from ty_extensions import Concat

type Size = Literal["sm", "lg"]
type Color = Literal["red", "blue"]
type ClassName = Concat[Concat[Literal["btn-"], Size], Concat[Literal["-"], Color]]

def f(name: ClassName, other: Concat[Literal["btn-"], str]):
    reveal_type(name)  # revealed: Literal["btn-sm-red", "btn-sm-blue", "btn-lg-red", "btn-lg-blue"]
    reveal_type(other)  # revealed: str
```

## Slicing strings

[//]: # (playground: slug=string-slice, phase=4, ts="S extends `${infer Head}${infer Tail}` ? Head : never")

`Slice[S, Start, End]` slices string literal types like Python's `s[start:end]`, by code point and
with negative indices. Together with `Concat`, it can rebuild TypeScript's `Capitalize`, which only
changes the first character.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, Literal

if TYPE_CHECKING:
    from ty_extensions import Concat, Slice, Uppercase

type Head[S] = Slice[S, Literal[0], Literal[1]]
type Tail[S] = Slice[S, Literal[1], None]
type TsCapitalize[S] = Concat[Uppercase[Head[S]], Tail[S]]

def f(
    a: Head[Literal["hello"]],
    b: Slice[Literal["hello"], Literal[-3], None],
    c: TsCapitalize[Literal["fooBar"]],
):
    reveal_type(a)  # revealed: Literal["h"]
    reveal_type(b)  # revealed: Literal["llo"]
    reveal_type(c)  # revealed: Literal["FooBar"]
```

## Getters

[//]: # (playground: slug=getters, phase=4, ts="{ [K in keyof T as `get_${K & string}`]: ... }")

String operators can compute member names. For every attribute `x` of a class, this protocol has a
getter `get_x`, like key remapping with `as` in a TypeScript mapped type. This is the motivating
use case of the PEP's string manipulation section.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, Callable, Literal

if TYPE_CHECKING:
    from ty_extensions import Attrs, Concat, Iter, Member, NewProtocol

type Getters[T] = NewProtocol[
    *[Member[Concat[Literal["get_"], p.name], Callable[[], p.type]] for p in Iter[Attrs[T]]]
]

class User:
    name: str
    age: int

def f(user: Getters[User]):
    reveal_type(user.get_name())  # revealed: str
    reveal_type(user.get_age())  # revealed: int
    user.get_email()  # error: [unresolved-attribute]
```

## Event names

[//]: # (playground: slug=event-names, phase=4, ts="`on${Capitalize<keyof T & string>}Changed`")

Combining `KeyOf` with string operators turns the attribute names of a class into a union of event
names. Because of lifting, the operators are applied to each name separately.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, Literal, Union

if TYPE_CHECKING:
    from ty_extensions import Capitalize, Concat, Iter, Members

type KeyOf[T] = Union[*[p.name for p in Iter[Members[T]]]]
type ChangeEvent[T] = Concat[Literal["on"], Concat[Capitalize[KeyOf[T]], Literal["Changed"]]]

class Person:
    name: str
    age: int

def f(event: ChangeEvent[Person]):
    reveal_type(event)  # revealed: Literal["onNameChanged", "onAgeChanged"]
    ok: ChangeEvent[Person] = "onAgeChanged"
    bad: ChangeEvent[Person] = "onEmailChanged"  # error: [invalid-assignment]
```

## snake_case to camelCase

[//]: # (playground: slug=snake-to-camel, phase=4, ts="S extends `${infer H}_${infer T}` ? `${H}${Capitalize<SnakeToCamel<T>>}` : S")

There is no operator that splits strings, but a recursive alias can walk through a string one
character at a time with `Slice`: it drops each `_` and uppercases the character after it. Only the
branch selected by the condition is evaluated, which is what makes the recursion stop.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, Literal

if TYPE_CHECKING:
    from ty_extensions import Concat, IsEquivalent, Slice, Uppercase

type Head[S] = Slice[S, Literal[0], Literal[1]]
type Tail[S] = Slice[S, Literal[1], None]
type SnakeToCamel[S] = (
    S
    if IsEquivalent[S, Literal[""]]
    else Concat[Uppercase[Head[Tail[S]]], SnakeToCamel[Tail[Tail[S]]]]
    if IsEquivalent[Head[S], Literal["_"]]
    else Concat[Head[S], SnakeToCamel[Tail[S]]]
)

def f(a: SnakeToCamel[Literal["user_name"]], b: SnakeToCamel[Literal["created_at_utc"]]):
    reveal_type(a)  # revealed: Literal["userName"]
    reveal_type(b)  # revealed: Literal["createdAtUtc"]
```

## Extended callables

[//]: # (playground: slug=extended-callable, phase=5, ts="[a: number, b?: string] as a labeled rest tuple")

`Callable[Params[...], R]` describes any signature, one `Param[name, type, kind, default]` per
parameter. Unlike `Callable[[...], R]`, it can express names, keyword-only parameters and defaults.
The result is an ordinary callable type.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, Callable, Literal

if TYPE_CHECKING:
    from ty_extensions import Param, ParamKind, Params

type Handler = Callable[
    Params[
        Param[None, bytes],
        Param[Literal["retries"], int, Literal[ParamKind.KEYWORD_ONLY], Literal[3]],
        Param[None, str, Literal[ParamKind.VAR_KEYWORD]],
    ],
    bool,
]

def run(handler: Handler):
    reveal_type(handler)  # revealed: (bytes, /, *, retries: int = 3, **kwargs: str) -> bool
    handler(b"data", retries=1, user="me")
    handler(b"data", retries="many")  # error: [invalid-argument-type]
```

## Inspecting parameters

[//]: # (playground: slug=callable-params, phase=5, ts="Parameters<typeof f> / ReturnType<typeof f>")

`GetArg[F, Callable, Literal[0]]` is the parameter list of any callable as a `Params[...]`, and
`GetArgs[P, Params]` turns it into a tuple that a comprehension can iterate over. Here we list the
names of the parameters that can be passed by keyword.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, Callable, Literal, Union

if TYPE_CHECKING:
    from ty_extensions import GetArg, GetArgs, IsAssignable, Iter, ParamKind, Params

type ParamsOf[F] = GetArgs[GetArg[F, Callable, Literal[0]], Params]
type Keyword = Literal[ParamKind.POSITIONAL_OR_KEYWORD, ParamKind.KEYWORD_ONLY]
type KeywordNames[F] = Union[*[p.name for p in Iter[ParamsOf[F]] if IsAssignable[p.kind, Keyword]]]
type ReturnOf[F] = GetArg[F, Callable, Literal[1]]

def keyword_names[F](f: F) -> KeywordNames[F]:
    raise NotImplementedError

def returns[F](f: F) -> ReturnOf[F]:
    raise NotImplementedError

def connect(host: str, /, port: int = 80, *, timeout: float = 1.0) -> bytes:
    return b""

def demo():
    reveal_type(keyword_names(connect))  # revealed: Literal["port", "timeout"]
    reveal_type(returns(connect))  # revealed: bytes
```

## Decorators that add a keyword parameter

[//]: # (playground: slug=decorator-add-keyword, phase=5, ts="Parameters<F> plus an options object, ReturnType<F>")

`ParamSpec` cannot add a keyword parameter to an arbitrary function. With extended callables, a
decorator can copy the parameters of the function and append a keyword-only `timeout`.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, Callable, Literal, cast

if TYPE_CHECKING:
    from ty_extensions import GetArg, GetArgs, Param, ParamKind, Params

type ParamsOf[F] = GetArgs[GetArg[F, Callable, Literal[0]], Params]
type Timeout = Param[Literal["timeout"], float, Literal[ParamKind.KEYWORD_ONLY], float]
type WithTimeout[F] = Callable[Params[*ParamsOf[F], Timeout], GetArg[F, Callable, Literal[1]]]

def with_timeout[F](f: F) -> WithTimeout[F]:
    return cast(WithTimeout[F], f)

@with_timeout
def fetch(url: str, retries: int = 0) -> bytes:
    return b""

def demo():
    reveal_type(fetch)  # revealed: (url: str, retries: int = 0, *, timeout: float = ...) -> bytes
    fetch("https://example.com", timeout=2.5)
    fetch("https://example.com", timeout="soon")  # error: [invalid-argument-type]
```

## Inferring a TypedDict from `**kwargs`

[//]: # (playground: slug=kwargs-typed-dict, phase=5, ts="function select<const K extends Fields>")

With `**kwargs: Unpack[K]`, the keyword arguments of a call are collected into a `TypedDict` that
keeps their literal types. A query function can use it to compute its result type from the columns
that were asked for.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, Unpack

if TYPE_CHECKING:
    from ty_extensions import Attrs, BaseTypedDict, GetMemberType, Iter, Member, NewProtocol

type Selected[M, K] = NewProtocol[*[Member[c.name, GetMemberType[M, c.name]] for c in Iter[Attrs[K]]]]

def select[M, K: BaseTypedDict](model: type[M], /, **columns: Unpack[K]) -> list[Selected[M, K]]:
    raise NotImplementedError

class User:
    id: int
    name: str
    email: str | None

def demo():
    rows = select(User, name=True, email=True)
    reveal_type(rows[0].name)  # revealed: str
    reveal_type(rows[0].email)  # revealed: str | None
    rows[0].id  # error: [unresolved-attribute]
```

## Field descriptors

[//]: # (playground: slug=init-field, phase=5, ts="{ default: 0, primaryKey: true } as const")

Subclasses of `InitField` describe fields like `dataclasses.field` or SQLModel's `Field`. Calling
one infers the literal types of its keyword arguments, and the result becomes the `init` of the
attribute's `Member`, so type-level code can read how each field was declared.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, Literal, ReadOnly, TypedDict, Union

if TYPE_CHECKING:
    from ty_extensions import Attrs, GetArg, GetMemberType, InitField, IsAssignable, Iter

class FieldArgs(TypedDict, total=False):
    default: ReadOnly[object]
    primary_key: ReadOnly[bool]

if TYPE_CHECKING:
    class Field[T: FieldArgs](InitField[T]): ...
else:
    def Field(**kwargs):
        return None

type Item[I, K] = GetMemberType[GetArg[I, InitField, Literal[0]], K]
type PrimaryKey[T] = Union[*[p.name for p in Iter[Attrs[T]] if IsAssignable[Literal[True], Item[p.init, Literal["primary_key"]]]]]

class Hero:
    id: int | None = Field(default=None, primary_key=True)
    name: str = Field()

def demo(hero: Hero, key: PrimaryKey[Hero]):
    reveal_type(key)  # revealed: Literal["id"]
    reveal_type(hero.id)  # revealed: int | None
```

## Dataclass-style `__init__`

[//]: # (playground: slug=dataclass-update-class, phase=5, ts="constructor taking { [K in keyof T]: T[K] }")

A class decorator whose return type is `UpdateClass[...]` adds members to the decorated class. This
one generates a keyword-only `__init__` from the attributes, like `@dataclass(kw_only=True)`, but
written in a library instead of being built into the type checker.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, Callable, Literal

if TYPE_CHECKING:
    from ty_extensions import Attrs, Iter, Member, Param, ParamKind, Params, UpdateClass

type KwParams[T] = tuple[*[Param[p.name, p.type, Literal[ParamKind.KEYWORD_ONLY], p.init] for p in Iter[Attrs[T]]]]
type InitFn[T] = Callable[Params[Param[Literal["self"], T], *KwParams[T]], None]

def dataclass_ish[T](cls: type[T]) -> UpdateClass[Member[Literal["__init__"], InitFn[T], Literal["ClassVar"]]]:
    return cls

@dataclass_ish
class Point:
    x: int
    y: int = 0

def demo():
    reveal_type(Point.__init__)  # revealed: (self: Point, *, x: int, y: int = 0) -> None
    Point(x=1)
    Point(1, x=1)  # error: [too-many-positional-arguments]
    Point(y=2)  # error: [missing-argument]
```

## Base classes that update their subclasses

[//]: # (playground: slug=model-init-subclass, phase=5, ts="class Hero extends Model<Hero>")

`__init_subclass__` can return `UpdateClass[...]` too. Then every subclass is updated when it is
defined, like a Pydantic model: `Hero` and `Sidekick` get an `__init__` without a decorator.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, Callable, Literal

if TYPE_CHECKING:
    from ty_extensions import Attrs, Iter, Member, Param, ParamKind, Params, UpdateClass

type KwParams[T] = tuple[*[Param[p.name, p.type, Literal[ParamKind.KEYWORD_ONLY], p.init] for p in Iter[Attrs[T]]]]
type InitFn[T] = Callable[Params[Param[Literal["self"], T], *KwParams[T]], None]

class Model:
    def __init_subclass__[T](cls: type[T]) -> UpdateClass[Member[Literal["__init__"], InitFn[T], Literal["ClassVar"]]]:
        pass

class Hero(Model):
    name: str
    age: int | None = None

class Sidekick(Hero):
    power: str

def demo():
    reveal_type(Sidekick.__init__)  # revealed: (self: Sidekick, *, name: str, age: int | None = None, power: str) -> None
    Hero(name="Deadpond")
    Hero(name="Deadpond", power="none")  # error: [unknown-argument]
```

## FastAPI CRUD models

[//]: # (playground: slug=fastapi-initfield-crud, phase=5, ts="Omit<Hero, 'secret_name'> / Omit<Hero, 'id'> / Partial<...>")

The motivating example of PEP 827: from one `Hero` model, derive the models that a FastAPI endpoint
returns (`Public`: no hidden fields, primary key not optional), accepts for creation (`Create`: no
primary key) and accepts for updates (`Update`: like `Create`, but everything optional). The field
helpers live in `fields.py`.

```toml
[environment]
python-version = "3.14"
```

`fields.py`:

```py
from typing import TYPE_CHECKING, Literal, ReadOnly, TypedDict, Union

if TYPE_CHECKING:
    from ty_extensions import FromUnion, GetArg, GetMemberType, InitField, IsAssignable, Iter

class FieldArgs(TypedDict, total=False):
    default: ReadOnly[object]
    primary_key: ReadOnly[bool]
    hidden: ReadOnly[bool]

if TYPE_CHECKING:
    class Field[T: FieldArgs](InitField[T]): ...
else:
    def Field(**kwargs):
        return None

type Item[I, K] = GetMemberType[GetArg[I, InitField, Literal[0]], K]
type IsSet[I, K] = IsAssignable[Literal[True], Item[I, K]]
type Default[I] = Item[I, Literal["default"]]
type NotOptional[T] = Union[*[x for x in Iter[FromUnion[T]] if not IsAssignable[x, None]]]
```

```py
from typing import TYPE_CHECKING, Literal
from fields import Default, Field, IsSet, NotOptional

if TYPE_CHECKING:
    from ty_extensions import Attrs, Iter, Member, NewProtocol

type PK = Literal["primary_key"]
type Public[T] = NewProtocol[*[
    Member[p.name, NotOptional[p.type] if IsSet[p.init, PK] else p.type]
    for p in Iter[Attrs[T]] if not IsSet[p.init, Literal["hidden"]]
]]
type Create[T] = NewProtocol[*[
    Member[p.name, p.type, p.quals, Default[p.init]] for p in Iter[Attrs[T]] if not IsSet[p.init, PK]
]]
type Update[T] = NewProtocol[*[
    Member[p.name, p.type | None, p.quals, None] for p in Iter[Attrs[T]] if not IsSet[p.init, PK]
]]

class Hero:
    id: int | None = Field(default=None, primary_key=True)
    name: str = Field()
    age: int | None = Field(default=None)
    secret_name: str = Field(hidden=True)

def endpoints(public: Public[Hero], create: Create[Hero], update: Update[Hero]):
    reveal_type(public.id)  # revealed: int
    reveal_type(create.age)  # revealed: int | None
    reveal_type(update.name)  # revealed: str | None
    public.secret_name  # error: [unresolved-attribute]
    create.id  # error: [unresolved-attribute]
```

## Prisma-style `select` with nested relations

[//]: # (playground: slug=showcase-orm-select, phase=6, ts="prisma.user.findMany({ select: { name: true, posts: { select: { title: true } } } })")

An ORM query whose result type is computed from the query itself. The keyword arguments of
`select` are inferred as a `TypedDict` (`**kwargs: Unpack[K]`) that keeps their literal types, so the
result knows exactly which columns were asked for. `Project` walks over that `TypedDict` with
`Attrs`, looks up each column on the model with `GetMemberType`, and unwraps `Property[...]` and
`Link[...]` markers with `GetArg`. A relation selected with `True` gives the scalar properties of the
target model, and a relation selected with `include(...)` is projected recursively (the two aliases
refer to each other). `MultiLink` relations become lists, and `all()` plus `RaiseError` reject columns
that the model does not have.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, Any, Literal, Union, Unpack

if TYPE_CHECKING:
    from ty_extensions import Attrs, BaseTypedDict, GetArg, GetMemberType, IsAssignable, Iter, Member, NewProtocol, RaiseError

class Pointer[T]: ...
class Property[T](Pointer[T]): ...
class Link[T](Pointer[T]): ...
class MultiLink[T](Link[T]): ...
class Nested[K]: ...

def include[K: BaseTypedDict](**columns: Unpack[K]) -> Nested[K]:
    raise NotImplementedError

type Target[F] = GetArg[F, Pointer, Literal[0]]
type Many[F, R] = list[R] if IsAssignable[F, MultiLink[Any]] else R
type Scalars[M] = NewProtocol[
    *[Member[p.name, Target[p.type]] for p in Iter[Attrs[M]] if IsAssignable[p.type, Property[Any]]]
]
type Resolve[F, V] = (
    Target[F]
    if IsAssignable[F, Property[Any]]
    else Many[F, Project[Target[F], GetArg[V, Nested, Literal[0]]]]
    if IsAssignable[V, Nested[Any]]
    else Many[F, Scalars[Target[F]]]
)
type Project[M, K] = NewProtocol[*[Member[c.name, Resolve[GetMemberType[M, c.name], c.type]] for c in Iter[Attrs[K]]]]
type Columns[M] = Union[*[p.name for p in Iter[Attrs[M]]]]
type Checked[M, K] = (
    Project[M, K]
    if all(IsAssignable[c.name, Columns[M]] for c in Iter[Attrs[K]])
    else RaiseError[Literal["unknown columns"], Union[*[c.name for c in Iter[Attrs[K]] if not IsAssignable[c.name, Columns[M]]]]]
)

def select[M, K: BaseTypedDict](model: type[M], /, **columns: Unpack[K]) -> list[Checked[M, K]]:
    raise NotImplementedError

class Comment:
    id: Property[int]
    body: Property[str]

class Post:
    id: Property[int]
    title: Property[str]
    comments: MultiLink[Comment]

class User:
    id: Property[int]
    name: Property[str]
    email: Property[str | None]
    manager: Link[User]
    posts: MultiLink[Post]

def demo():
    users = select(User, name=True, manager=True, posts=include(title=True, comments=True))
    reveal_type(users[0].name)  # revealed: str
    reveal_type(users[0].manager.email)  # revealed: str | None
    users[0].email  # error: [unresolved-attribute]
    users[0].manager.posts  # error: [unresolved-attribute]
    reveal_type(users[0].posts[0].title)  # revealed: str
    reveal_type(users[0].posts[0].comments[0].body)  # revealed: str
    users[0].posts[0].id  # error: [unresolved-attribute]

def typo():
    select(User, name=True, emial=True)  # error: [raised-type-error] "unknown columns: Literal["emial"]"
```

## FastAPI CRUD router

[//]: # (playground: slug=showcase-fastapi-crud, phase=6, ts="createRouter<typeof Hero>(): { create(body: Omit<Hero, 'id'>): Public<Hero> }")

The motivating example of PEP 827, taken one step further: from a single SQLModel-style `Hero`
class, derive a whole typed CRUD router. The fields are declared with `Field(...)`, an `InitField`
subclass, so type-level code can read `primary_key`, `hidden` and `default` from each attribute's
`Member.init`. `Public` (what the API returns) hides the `hidden` fields and makes the primary key
non-optional by filtering `FromUnion`. `create` and `update` are extended callables built with
`Params` and a comprehension: `create` takes one keyword-only parameter per non-key column with the
field's default, and `update` makes every column optional. The field helpers live in `fields.py`.

```toml
[environment]
python-version = "3.14"
```

`fields.py`:

```py
from typing import TYPE_CHECKING, Literal, ReadOnly, TypedDict, Union

if TYPE_CHECKING:
    from ty_extensions import FromUnion, GetArg, GetMemberType, InitField, IsAssignable, Iter

class FieldArgs(TypedDict, total=False):
    default: ReadOnly[object]
    primary_key: ReadOnly[bool]
    hidden: ReadOnly[bool]

if TYPE_CHECKING:
    class Field[T: FieldArgs](InitField[T]): ...
else:
    def Field(**kwargs):
        return None

type Item[I, K] = GetMemberType[GetArg[I, InitField, Literal[0]], K]
type IsSet[I, K] = IsAssignable[Literal[True], Item[I, K]]
type Default[I] = Item[I, Literal["default"]]
type NotOptional[T] = Union[*[x for x in Iter[FromUnion[T]] if not IsAssignable[x, None]]]
```

```py
from typing import TYPE_CHECKING, Callable, Literal
from fields import Default, Field, IsSet, NotOptional

if TYPE_CHECKING:
    from ty_extensions import Attrs, Iter, Member, NewProtocol, Param, ParamKind, Params

type PK = Literal["primary_key"]
type KwOnly = Literal[ParamKind.KEYWORD_ONLY]
type Public[T] = NewProtocol[
    *[
        Member[p.name, NotOptional[p.type] if IsSet[p.init, PK] else p.type]
        for p in Iter[Attrs[T]]
        if not IsSet[p.init, Literal["hidden"]]
    ]
]
type CreateParams[T] = Params[
    *[Param[p.name, p.type, KwOnly, Default[p.init]] for p in Iter[Attrs[T]] if not IsSet[p.init, PK]]
]
type UpdateParams[T] = Params[
    Param[None, int],
    *[Param[p.name, p.type | None, KwOnly, None] for p in Iter[Attrs[T]] if not IsSet[p.init, PK]],
]
type ListParams = Params[Param[Literal["offset"], int, KwOnly, Literal[0]], Param[Literal["limit"], int, KwOnly, Literal[100]]]
type Router[T] = NewProtocol[
    Member[Literal["create"], Callable[CreateParams[T], Public[T]]],
    Member[Literal["get"], Callable[[int], Public[T] | None]],
    Member[Literal["update"], Callable[UpdateParams[T], Public[T]]],
    Member[Literal["list"], Callable[ListParams, list[Public[T]]]],
]

def crud[T](model: type[T]) -> Router[T]:
    raise NotImplementedError

class Hero:
    id: int | None = Field(default=None, primary_key=True)
    name: str = Field()
    age: int | None = Field(default=None)
    secret_name: str = Field(hidden=True)

def endpoints():
    heroes = crud(Hero)
    created = heroes.create(name="Deadpond", secret_name="Dive Wilson")
    # revealed: (*, name: str, age: int | None = None, secret_name: str) -> Public[Hero]
    reveal_type(heroes.create)
    reveal_type(created.id)  # revealed: int
    reveal_type(heroes.update(1, age=31).age)  # revealed: int | None
    reveal_type(heroes.list(limit=10)[0].name)  # revealed: str
    created.secret_name  # error: [unresolved-attribute]
    heroes.create(name="Deadpond")  # error: [missing-argument]
    heroes.create(id=1, name="Deadpond", secret_name="Dive Wilson")  # error: [unknown-argument]
    heroes.update(1, nickname="Wade")  # error: [unknown-argument]
    heroes.list(10)  # error: [too-many-positional-arguments]
```

## Observable store

[//]: # (playground: slug=showcase-observable-store, phase=6, ts="{ [K in keyof T as `on${Capitalize<CamelCase<K>>}Change`]: (h: (old: T[K], new: T[K]) => void) => () => void }")

A typed observable store, like a MobX or Vue store. For every public attribute of `Settings`, the
store has the attribute itself, a `set_x` setter, and an `onXChange` subscription method whose name is
computed with a recursive snake_case to PascalCase conversion (`Slice`, `Concat`, `Uppercase`).
Attributes whose name starts with `_` are private, and attributes annotated with `Silent()` metadata
(read with `GetAnnotations`) do not emit events. `emit` is an `Overloaded` callable with one signature
per observable attribute, so the value type is checked against the attribute name. Handlers are
extended callables with named parameters.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, Annotated, Callable, Literal

if TYPE_CHECKING:
    from ty_extensions import (
        Attrs, Concat, GetAnnotations, IsAssignable, IsEquivalent, Iter, Member, NewProtocol, Overloaded, Param, Params, Slice, Uppercase,
    )

class Silent: ...

type Head[S] = Slice[S, Literal[0], Literal[1]]
type Tail[S] = Slice[S, Literal[1], None]
type Camel[S] = (
    S
    if IsEquivalent[S, Literal[""]]
    else Concat[Uppercase[Head[Tail[S]]], Camel[Tail[Tail[S]]]]
    if IsEquivalent[Head[S], Literal["_"]]
    else Concat[Head[S], Camel[Tail[S]]]
)
type Pascal[S] = Concat[Uppercase[Head[S]], Camel[Tail[S]]]
type IsPublic[P] = not IsEquivalent[Head[P.name], Literal["_"]]
type Fields[T] = tuple[*[p for p in Iter[Attrs[T]] if IsPublic[p]]]
type Observed[T] = tuple[*[p for p in Iter[Attrs[T]] if IsPublic[p] and not IsAssignable[Silent, GetAnnotations[p.type]]]]

type Handler[V] = Callable[Params[Param[Literal["old"], V], Param[Literal["new"], V]], None]
type Store[T] = NewProtocol[
    *[Member[p.name, p.type] for p in Iter[Fields[T]]],
    *[Member[Concat[Literal["set_"], p.name], Callable[[p.type], None]] for p in Iter[Fields[T]]],
    *[Member[Concat[Literal["on"], Concat[Pascal[p.name], Literal["Change"]]], Callable[[Handler[p.type]], Callable[[], None]]] for p in Iter[Observed[T]]],
    Member[Literal["emit"], Overloaded[*[Callable[Params[Param[Literal["field"], p.name], Param[Literal["value"], p.type]], None] for p in Iter[Observed[T]]]]],
]

def observable[T](model: type[T]) -> Store[T]:
    raise NotImplementedError

class Settings:
    user_name: str
    font_size: int
    dark_mode: bool
    session_token: Annotated[str, Silent()]
    _cache: dict[str, str]

def on_font_size(old: int, new: int) -> None: ...

def demo():
    store = observable(Settings)
    reveal_type(store.session_token)  # revealed: str
    store.set_dark_mode(True)
    store.set_font_size("large")  # error: [invalid-argument-type]
    unsubscribe = store.onFontSizeChange(on_font_size)
    reveal_type(unsubscribe)  # revealed: () -> None
    store.onUserNameChange(on_font_size)  # error: [invalid-argument-type]
    store.emit("dark_mode", False)
    store.emit("font_size", "14")  # error: [no-matching-overload]
    store.onSessionTokenChange  # error: [unresolved-attribute]
    store.set__cache  # error: [unresolved-attribute]
```

## Dataclass decorator and builder

[//]: # (playground: slug=showcase-dataclass-builder, phase=6, ts="class decorators cannot change a class type in TypeScript")

`@dataclass` written as a library. The decorator returns `UpdateClass[...]`, which adds members to
the decorated class: a keyword-only `__init__` whose parameters come from `Attrs`, a `replace` method
where every parameter is optional, and `__match_args__`. Defaults are read from the class body:
either a plain value, or the `default` of a `field(...)` call (an `InitField`). `field(init=False)`
removes a parameter from `__init__`. Next to it, `builder()` derives a fluent builder protocol from
the same class, with one `with_x` method per field computed with `Concat`, each returning the
builder again.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, Callable, Literal, Never, ReadOnly, TypedDict

if TYPE_CHECKING:
    from ty_extensions import (
        Attrs, Concat, GetArg, GetMemberType, InitField, IsAssignable, IsEquivalent, Iter, Member, NewProtocol, Param, ParamKind, Params, UpdateClass,
    )

class FieldArgs(TypedDict, total=False):
    default: ReadOnly[object]
    init: ReadOnly[bool]

if TYPE_CHECKING:
    class field[K: FieldArgs](InitField[K]): ...
else:
    def field(**kwargs):
        return None

type KwOnly = Literal[ParamKind.KEYWORD_ONLY]
type IsField[I] = IsAssignable[I, field] and not IsAssignable[I, Never]
type Item[I, K] = GetMemberType[GetArg[I, InitField, Literal[0]], K]
type Default[I] = Item[I, Literal["default"]] if IsField[I] else I
type InInit[I] = not (IsField[I] and IsEquivalent[Item[I, Literal["init"]], Literal[False]])

type InitFn[T] = Callable[
    Params[Param[Literal["self"], T], *[Param[p.name, p.type, KwOnly, Default[p.init]] for p in Iter[Attrs[T]] if InInit[p.init]]],
    None,
]
type ReplaceFn[T] = Callable[Params[Param[Literal["self"], T], *[Param[p.name, p.type, KwOnly, p.type] for p in Iter[Attrs[T]]]], T]
type MatchArgs[T] = tuple[*[p.name for p in Iter[Attrs[T]] if InInit[p.init]]]

def dataclass[T](cls: type[T]) -> UpdateClass[
    Member[Literal["__init__"], InitFn[T], Literal["ClassVar"]],
    Member[Literal["replace"], ReplaceFn[T], Literal["ClassVar"]],
    Member[Literal["__match_args__"], MatchArgs[T], Literal["ClassVar"]],
]:
    return cls

type Builder[T] = NewProtocol[
    *[Member[Concat[Literal["with_"], p.name], Callable[[p.type], Builder[T]]] for p in Iter[Attrs[T]]],
    Member[Literal["build"], Callable[[], T]],
]

def builder[T](cls: type[T]) -> Builder[T]:
    raise NotImplementedError

@dataclass
class Point:
    x: int
    y: int = 0
    label: str = field(default="origin")
    cache: dict[str, int] = field(init=False)

def demo(p: Point):
    reveal_type(Point.__init__)  # revealed: (self: Point, *, x: int, y: int = 0, label: str = "origin") -> None
    reveal_type(Point(x=1))  # revealed: Point
    reveal_type(p.replace(y=2))  # revealed: Point
    reveal_type(Point.__match_args__)  # revealed: tuple[Literal["x"], Literal["y"], Literal["label"]]
    Point(x="1")  # error: [invalid-argument-type]
    Point(x=1, cache={})  # error: [unknown-argument]
    p.replace(z=3)  # error: [unknown-argument]
    reveal_type(builder(Point).with_x(1).with_label("a").build())  # revealed: Point
    builder(Point).with_y("2")  # error: [invalid-argument-type]
```

## Transposing tuples

[//]: # (playground: slug=showcase-transpose, phase=6, ts="type Transpose<Rows extends unknown[][]> = ...")

A strict, typed `zip(*rows)` for rows of fixed length, and `record(keys, values)`, which zips a
header and a row into a `TypedDict`. `Transpose` is a recursive alias: it takes the first element of
every row (`GetArg`), then recurses on the rest of every row (`Slice`) until the rows are empty
(`Length`). Before that, `any()` rejects rows of unknown length and `all()` checks that the rows
have the same length, so mismatches are reported with `RaiseError` at the call.

```toml
[environment]
python-version = "3.14"
```

```py
from typing import TYPE_CHECKING, Literal

if TYPE_CHECKING:
    from ty_extensions import GetArg, IsAssignable, IsEquivalent, Iter, Length, Member, NewTypedDict, RaiseError, Slice

type First[T] = GetArg[T, tuple, Literal[0]]
type Second[T] = GetArg[T, tuple, Literal[1]]
type Rest[T] = Slice[T, Literal[1], None]
type Transpose[Rows] = (
    tuple[()]
    if IsEquivalent[Length[First[Rows]], Literal[0]]
    else tuple[tuple[*[First[r] for r in Iter[Rows]]], *Transpose[tuple[*[Rest[r] for r in Iter[Rows]]]]]
)
type Checked[Rows] = (
    RaiseError[Literal["rows must have a fixed length"]]
    if any(IsAssignable[Length[r], None] for r in Iter[Rows])
    else Transpose[Rows]
    if all(IsEquivalent[Length[r], Length[First[Rows]]] for r in Iter[Rows])
    else RaiseError[Literal["rows have different lengths"], Rows]
)
type Record[K, V] = NewTypedDict[*[Member[First[kv], Second[kv]] for kv in Iter[Checked[tuple[K, V]]]]]

def columns[*Rows](*rows: *Rows) -> Checked[tuple[*Rows]]:
    raise NotImplementedError

def record[K, V](keys: K, values: V) -> Record[K, V]:
    raise NotImplementedError

def demo(
    ada: tuple[int, str, float],
    bob: tuple[bool, str, int],
    short: tuple[int, str],
    many: tuple[int, ...],
    header: tuple[Literal["id"], Literal["name"]],
):
    cols = columns(ada, bob)
    ids: tuple[int, bool] = cols[0]
    scores: tuple[float, int] = cols[2]
    wrong: tuple[str, str] = cols[0]  # error: [invalid-assignment]
    columns(ada, short)  # error: [raised-type-error] "rows have different lengths"
    columns(ada, many)  # error: [raised-type-error] "rows must have a fixed length"
    row = record(header, short)
    reveal_type(row["id"])  # revealed: int
    reveal_type(row["name"])  # revealed: str
    row["age"]  # error: [invalid-key]
```

## Typed task queue

[//]: # (playground: slug=showcase-task-queue, phase=6, ts="perform(args: { [K in keyof Parameters<F>]: ... }): Job<Awaited<ReturnType<F>>>")

A Celery/RQ-style task queue. A task is enqueued with a JSON-like payload, so `perform` takes the
arguments of the function as a `TypedDict`, like a mapped type over TypeScript's `Parameters<F>`.
`GetArg[F, Callable, Literal[0]]` is the `Params[...]` of any callable, `GetArgs` turns it into a
tuple, and `Payload` maps every parameter to an item of a `NewTypedDict`, which is `NotRequired` when
the parameter has a default (`p.default` is not `Never`). `GetArg[F, Callable, Literal[1]]` is the
return type (`ReturnType<F>`), and `Job.result()` unwraps coroutines with a conditional type over the
`Coroutine` base class (`Awaited<T>`). `delay` keeps the original signature, like Celery's
`task.delay(...)`. A payload cannot fill positional-only parameters, `*args` or `**kwargs`, so instead
of silently dropping them, `Checked` rejects such functions with `RaiseError` and names the offending
parameters.

The queue library in `tasks.py` offers three ways to declare a task: a class generic over a callable
type (`Task[F: Callable[..., object]]`, created by the `@task` decorator), a class generic over a
`ParamSpec` and a return type (`Task2[**P, R]`, where `ArgsOf[Callable[P, R]]` is evaluated once `P`
is solved), and a base class whose `__init_subclass__` returns `UpdateClass[...]`: it adds a typed
`perform` method to every subclass, computed from its `run` method (`Slice` drops `self`).

```toml
[environment]
python-version = "3.14"
```

`tasks.py`:

```py
from collections.abc import Coroutine
from typing import TYPE_CHECKING, Any, Callable, Literal, Never, Union

if TYPE_CHECKING:
    from ty_extensions import (
        GetArg, GetArgs, GetMemberType, IsAssignable, IsEquivalent, Iter, Member, NewTypedDict, Param, ParamKind, Params, RaiseError, Slice, UpdateClass,
    )

type ParamsOf[F] = GetArgs[GetArg[F, Callable, Literal[0]], Params]
type Keyword = Literal[ParamKind.POSITIONAL_OR_KEYWORD, ParamKind.KEYWORD_ONLY]
type Optional[P] = Never if IsEquivalent[P.default, Never] else Literal["NotRequired"]
type Payload[Ps] = NewTypedDict[*[Member[p.name, p.type, Optional[p]] for p in Iter[Ps]]]
type ArgsOf[F] = Payload[ParamsOf[F]]
type Awaited[R] = GetArg[R, Coroutine, Literal[2]] if IsAssignable[R, Coroutine[Any, Any, Any]] else R
type Checked[F] = (
    F
    if all(IsAssignable[p.kind, Keyword] for p in Iter[ParamsOf[F]])
    else RaiseError[
        Literal["task parameters must be passable by keyword"],
        Union[*[p.name for p in Iter[ParamsOf[F]] if not IsAssignable[p.kind, Keyword]]],
    ]
)

class Job[R]:
    def result(self) -> Awaited[R]:
        raise NotImplementedError

class Task[F: Callable[..., object]]:
    delay: Callable[GetArg[F, Callable, Literal[0]], Job[GetArg[F, Callable, Literal[1]]]]

    def perform(self, args: ArgsOf[F]) -> Job[GetArg[F, Callable, Literal[1]]]:
        raise NotImplementedError

def task[F: Callable[..., object]](f: F) -> Task[Checked[F]]:
    raise NotImplementedError

class Task2[**P, R]:
    def perform(self, args: ArgsOf[Callable[P, R]]) -> Job[R]:
        raise NotImplementedError

def task2[**P, R](f: Callable[P, R]) -> Task2[P, R]:
    raise NotImplementedError

type RunOf[T] = GetMemberType[T, Literal["run"]]
type PerformFn[T] = Callable[
    Params[Param[Literal["self"], T], Param[Literal["args"], Payload[Slice[ParamsOf[RunOf[T]], Literal[1], None]]]],
    Job[GetArg[RunOf[T], Callable, Literal[1]]],
]

class TaskBase:
    def __init_subclass__[T](cls: type[T]) -> UpdateClass[Member[Literal["perform"], PerformFn[T], Literal["ClassVar"]]]:
        pass
```

```py
from tasks import Task2, TaskBase, task, task2

@task
def send_email(to: str, subject: str, retries: int = 3) -> bool:
    return True

@task
async def fetch(url: str, *, timeout: float = 10.0) -> bytes:
    return b""

def enqueue():
    reveal_type(send_email.perform({"to": "a@b.c", "subject": "hi"}))  # revealed: Job[bool]
    reveal_type(send_email.delay)  # revealed: (to: str, subject: str, retries: int = 3) -> Job[bool]
    reveal_type(fetch.perform({"url": "https://example.com"}).result())  # revealed: bytes
    # error: [missing-typed-dict-key] "Missing required key 'subject'"
    # error: [invalid-argument-type]
    send_email.perform({"to": "a@b.c"})
    # error: [invalid-argument-type] "Invalid argument to key "retries""
    # error: [invalid-argument-type]
    send_email.perform({"to": "a@b.c", "subject": "hi", "retries": "3"})
    # error: [invalid-key] "Unknown key "cc""
    # error: [invalid-argument-type]
    send_email.perform({"to": "a@b.c", "subject": "hi", "cc": "boss@b.c"})
    send_email.delay("a@b.c", "hi", retries="3")  # error: [invalid-argument-type]

# Functions whose parameters cannot come from a payload are rejected.
def legacy(path: str, /, *flags: str) -> None: ...

def declare():
    task(legacy)  # error: [raised-type-error] "task parameters must be passable by keyword: Literal["path", "flags"]"

@task  # error: [raised-type-error] "task parameters must be passable by keyword: Literal["path", "flags"]"
def legacy_task(path: str, /, *flags: str) -> None: ...

# The `ParamSpec` form and the base class form give the same payload types.
@task2
def resize(image: bytes, width: int, height: int = 0) -> bytes:
    return image

class Report(TaskBase):
    def run(self, user_id: int, fmt: str = "pdf") -> str:
        return ""

def other_forms(report: Report):
    reveal_type(resize.perform({"image": b"", "width": 100}))  # revealed: Job[bytes]
    reveal_type(report.perform({"user_id": 1}))  # revealed: Job[str]
    # error: [missing-typed-dict-key]
    # error: [invalid-argument-type]
    resize.perform({"image": b""})
    # error: [invalid-key]
    # error: [invalid-argument-type]
    report.perform({"user_id": 1, "self": report})
```
