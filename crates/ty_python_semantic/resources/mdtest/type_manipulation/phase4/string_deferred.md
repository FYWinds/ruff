# String operators: deferred evaluation and composition

When an argument of a string operator mentions a type variable that has not been solved yet, the
operator is kept unevaluated, like the type booleans and conditional types of [PEP 827]. It is
evaluated once the type variable is replaced.

```toml
[environment]
python-version = "3.14"
```

## Generic type aliases

```py
from typing import Literal
from ty_extensions import Capitalize, Concat

type Getter[K] = Concat[Literal["get_"], K]
type EventName[K] = Concat[Literal["on"], Capitalize[K]]

def _(
    a: Getter[Literal["name"]],
    b: EventName[Literal["click"]],
    c: EventName[Literal["click", "focus"]],
    d: list[Getter[Literal["x"]]],
):
    reveal_type(a)  # revealed: Literal["get_name"]
    reveal_type(b)  # revealed: Literal["onClick"]
    reveal_type(c)  # revealed: Literal["onClick", "onFocus"]
    # Aliases nested inside other types keep their name in the display; the element is evaluated.
    reveal_type(d)  # revealed: list[Getter[Literal["x"]]]
    reveal_type(d[0])  # revealed: Literal["get_x"]
```

## Generic functions

A string operator in a return type is evaluated at each call site. Type variables solved from
string literal arguments keep their literal types, so the result is a literal. A non-literal `str`
argument gives `str`.

```py
from typing import Literal, LiteralString
from ty_extensions import Concat, Uppercase

def getter_name[K: str](key: K) -> Concat[Literal["get_"], K]:
    raise NotImplementedError

def shout[S: str](text: S) -> Uppercase[S]:
    raise NotImplementedError

def _(s: str, ls: LiteralString):
    reveal_type(getter_name("name"))  # revealed: Literal["get_name"]
    reveal_type(getter_name(s))  # revealed: str
    reveal_type(getter_name(ls))  # revealed: LiteralString
    reveal_type(shout("hi"))  # revealed: Literal["HI"]
```

## Display of unevaluated operators

Inside a generic function, the operators are displayed in their source form.

```py
from typing import Literal
from ty_extensions import Concat, Lowercase, Uppercase

def f[K: str](
    key: K,
    upper: Uppercase[K],
    name: Concat[Literal["get_"], K],
    nested: Concat[Lowercase[K], Literal["!"]] | None,
):
    reveal_type(upper)  # revealed: Uppercase[K@f]
    reveal_type(name)  # revealed: Concat[Literal["get_"], K@f]
    reveal_type(nested)  # revealed: Concat[Lowercase[K@f], Literal["!"]] | None
```

With phase 3's `Slice`:

```py
from typing import Literal
from ty_extensions import Slice

def g[K: str](part: Slice[K, Literal[1], None]):
    reveal_type(part)  # revealed: Slice[K@g, Literal[1], None]
```

## Unevaluated operators inside generic bodies

The result of a string operator is always a `str`, so an unevaluated operator can be used wherever a
`str` is expected. But a value can only be assigned to it if it is assignable to *every* possible
result, which no actual value is. Returning `"get_" + key` from `getter_name` is therefore an error;
use `cast` to work around it.

```py
from typing import Literal, cast
from ty_extensions import Concat, Uppercase

def widen[K: str](name: Uppercase[K]) -> str:
    return name

def strict[K: str](key: K) -> Concat[Literal["get_"], K]:
    return "get_" + key  # error: [invalid-return-type]

def relaxed[K: str](key: K) -> Concat[Literal["get_"], K]:
    return cast(Concat[Literal["get_"], K], "get_" + key)

def same[K: str](name: Uppercase[K]) -> Uppercase[K]:
    return name
```

## Variance

A larger set of input strings gives a larger set of output strings, so string operators are
covariant in their arguments.

```py
from typing import Literal
from ty_extensions import Concat

class Accessor[K: str]:
    def method_name(self) -> Concat[Literal["get_"], K]:
        raise NotImplementedError

def _(a: Accessor[Literal["name"]]):
    b: Accessor[Literal["name", "age"]] = a
    c: Accessor[str] = a
    d: Accessor[Literal["age"]] = a  # error: [invalid-assignment]
```

## Member names in comprehensions

This section also needs phase 2. The name of a `Member` can be computed with string operators, which
is how TypeScript's key remapping (`as` clauses in mapped types) is written. Here, every attribute
`x` of `T` produces a getter attribute `get_x`.

```py
from typing import Callable, Literal
from ty_extensions import Attrs, Concat, Iter, Member, NewProtocol

class User:
    name: str
    age: int

type Getters[T] = NewProtocol[
    *[Member[Concat[Literal["get_"], p.name], Callable[[], p.type]] for p in Iter[Attrs[T]]]
]

def _(g: Getters[User]):
    reveal_type(g.get_name)  # revealed: () -> str
    reveal_type(g.get_age())  # revealed: int
    g.name  # error: [unresolved-attribute]
```

The same works for the key union of a class or a `TypedDict`, using the `KeyOf` alias from the PEP:

```py
from typing import Literal, TypedDict, Union
from ty_extensions import Concat, Iter, Members, Uppercase

type KeyOf[T] = Union[*[p.name for p in Iter[Members[T]]]]

class Movie(TypedDict):
    title: str
    year: int

def _(a: Concat[Literal["get_"], KeyOf[User]], b: Uppercase[KeyOf[Movie]]):
    reveal_type(a)  # revealed: Literal["get_name", "get_age"]
    reveal_type(b)  # revealed: Literal["TITLE", "YEAR"]
```

## Recursive string processing

This section also needs phase 3: `Slice`, and lazy evaluation of recursive conditional aliases (only
the branch that is selected by the condition is evaluated).

There is no operator that splits or searches strings, but a recursive alias can process a string
one character at a time.

```py
from typing import Literal
from ty_extensions import Concat, IsEquivalent, Slice, Uppercase

type Head[S] = Slice[S, Literal[0], Literal[1]]
type Tail[S] = Slice[S, Literal[1], None]

type Reverse[S] = S if IsEquivalent[S, Literal[""]] else Concat[Reverse[Tail[S]], Head[S]]

type SnakeToCamel[S] = (
    S
    if IsEquivalent[S, Literal[""]]
    else Concat[Uppercase[Head[Tail[S]]], SnakeToCamel[Tail[Tail[S]]]]
    if IsEquivalent[Head[S], Literal["_"]]
    else Concat[Head[S], SnakeToCamel[Tail[S]]]
)

def _(
    a: Reverse[Literal["abc"]],
    b: Reverse[Literal[""]],
    c: SnakeToCamel[Literal["user_name"]],
    d: SnakeToCamel[Literal["created_at_utc"]],
    e: SnakeToCamel[Literal["plain"]],
):
    reveal_type(a)  # revealed: Literal["cba"]
    reveal_type(b)  # revealed: Literal[""]
    reveal_type(c)  # revealed: Literal["userName"]
    reveal_type(d)  # revealed: Literal["createdAtUtc"]
    reveal_type(e)  # revealed: Literal["plain"]
```

Conditional types are not distributed over unions (unlike in TypeScript), while `Slice` and `Concat`
are lifted. With a union argument, `IsEquivalent` looks at the union as a whole, and the characters
of different members get mixed up:

```py
def _(a: Reverse[Literal["ab", "cd"]]):
    reveal_type(a)  # revealed: Literal["ba", "bc", "da", "dc"]
```

## Recursion depth

This section also needs phase 3 (lazy conditional evaluation and its recursion guard).

Recursion through *helper aliases* is limited to a fixed depth. A specialized alias such as
`Tail[S]` is kept as an alias rather than expanded, so the argument of `Reverse[Tail[S]]` gets one
level more deeply nested with every character (`Tail[Tail[Tail[...]]]`). Phase 3 stops evaluating a
generic alias whose arguments are nested more than 64 levels deep. The result is then `Unknown`, and
no diagnostic is reported (like other errors that only appear after specialization). The exact
maximum string length depends on how phase 3 counts the nesting, so this test stays well away from
the boundary.

```py
from typing import Literal
from ty_extensions import Concat, IsEquivalent, Slice

type Head[S] = Slice[S, Literal[0], Literal[1]]
type Tail[S] = Slice[S, Literal[1], None]
type Reverse[S] = S if IsEquivalent[S, Literal[""]] else Concat[Reverse[Tail[S]], Head[S]]

type X10 = Literal["0123456789"]
type X20 = Concat[X10, X10]
type X40 = Concat[X20, X20]
type X80 = Concat[X40, X40]

def _(ok: Reverse[X10], too_deep: Reverse[X80]):
    reveal_type(ok)  # revealed: Literal["9876543210"]
    reveal_type(too_deep)  # revealed: Unknown
```

When `Slice` is written inline instead, it is evaluated to a plain string literal each time the
alias is specialized, so the arguments never get nested and the depth limit does not apply. The
recursion is then only bounded by the length of the string:

```py
type ReverseInline[S] = (
    S
    if IsEquivalent[S, Literal[""]]
    else Concat[ReverseInline[Slice[S, Literal[1], None]], Slice[S, Literal[0], Literal[1]]]
)

def _(a: ReverseInline[X80]):
    reveal_type(a)  # revealed: Literal["98765432109876543210987654321098765432109876543210987654321098765432109876543210"]
```

Even for the longest string literal type ty represents (4096 bytes), this recursion must not
overflow the stack. Since the arguments stay flat, the depth limit above does not help here; instead,
at most 512 evaluations of specialized aliases with type operators may be active at the same time.
A deeper evaluation is cut off: its result is `Unknown`, again without a diagnostic. Strings of up to
511 characters can be reversed this way; the 4096-character case checks that checking terminates
normally.

```py
type A64 = Literal["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"]
type A128 = Concat[A64, A64]
type A256 = Concat[A128, A128]
type A512 = Concat[A256, A256]
type A1024 = Concat[A512, A512]
type A2048 = Concat[A1024, A1024]
type A4096 = Concat[A2048, A2048]

def _(medium: IsEquivalent[ReverseInline[A256], A256], long: ReverseInline[A4096]):
    reveal_type(medium)  # revealed: Literal[True]
    reveal_type(long)  # revealed: Unknown
    # Checking continues normally after the long recursion.
    reveal_type(1)  # revealed: Literal[1]
```

[PEP 827]: https://peps.python.org/pep-0827/
