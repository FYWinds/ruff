# Type comprehensions

[PEP 827] allows an unpacked list comprehension,
`*[<type> for <name> in Iter[<tuple type>] if <type boolean> if ...]`, wherever an unpacked tuple
is allowed.

It iterates over the elements of the tuple type, keeps the elements for which every `if` condition
is `Literal[True]`, and maps each of them to a type. In phase 2 there is exactly one `for` clause,
and any number of `if` clauses.

Comprehensions can be spliced into `tuple[...]`, the now variadic `Union[...]`, `NewProtocol[...]`
and `NewTypedDict[...]`.

```toml
[environment]
python-version = "3.14"
```

## Mapping over a tuple

```py
from ty_extensions import Iter

def _(
    a: tuple[*[list[t] for t in Iter[tuple[int, str]]]],
    b: tuple[*[t for t in Iter[tuple[int, str]]]],
    c: tuple[*[dict[str, t] for t in Iter[tuple[int]]]],
):
    reveal_type(a)  # revealed: tuple[list[int], list[str]]
    reveal_type(b)  # revealed: tuple[int, str]
    reveal_type(c)  # revealed: tuple[dict[str, int]]
```

## Splicing into `tuple` and `Union`

The comprehension can be combined with other elements. `Union` accepts unpacked tuples and
comprehensions, and flattens them.

```py
from typing import Union
from ty_extensions import Iter

def _(
    a: tuple[int, *[list[t] for t in Iter[tuple[str, bytes]]], None],
    b: Union[*[list[t] for t in Iter[tuple[str, bytes]]]],
    c: Union[int, *[list[t] for t in Iter[tuple[str]]]],
    d: Union[*tuple[int, str]],
):
    reveal_type(a)  # revealed: tuple[int, list[str], list[bytes], None]
    reveal_type(b)  # revealed: list[str] | list[bytes]
    reveal_type(c)  # revealed: int | list[str]
    reveal_type(d)  # revealed: int | str
```

## Filters

An element is kept only if every condition is `Literal[True]`. Like the condition of a conditional
type, a condition of type `bool` counts as false.

```py
from typing import Literal
from ty_extensions import IsAssignable, Iter

type Ts = tuple[int, str, bool, bytes]
type Maybe = bool

def _(
    ints: tuple[*[t for t in Iter[Ts] if IsAssignable[t, int]]],
    strict: tuple[*[t for t in Iter[Ts] if IsAssignable[t, int] if not IsAssignable[t, bool]]],
    either: tuple[*[t for t in Iter[Ts] if IsAssignable[t, str] or IsAssignable[t, bytes]]],
    always: tuple[*[t for t in Iter[Ts] if Literal[True]]],
    never: tuple[*[t for t in Iter[Ts] if Maybe]],
):
    reveal_type(ints)  # revealed: tuple[int, bool]
    reveal_type(strict)  # revealed: tuple[int]
    reveal_type(either)  # revealed: tuple[str, bytes]
    reveal_type(always)  # revealed: tuple[int, str, bool, bytes]
    reveal_type(never)  # revealed: tuple[()]
```

## Filters short-circuit

Conditions are evaluated from left to right, and the element type is only computed for elements that
pass all conditions. Here `x.type` and `x.name` would be errors for `int`, which is not a `Member`,
but `int` is filtered out by the first condition before they are evaluated.

```py
from typing import Literal
from ty_extensions import IsAssignable, Iter, Member

type Mixed = tuple[int, Member[Literal["a"], int], Member[Literal["b"], str]]

def _(
    names: tuple[*[x.name for x in Iter[Mixed] if IsAssignable[x, Member[str, object]] if IsAssignable[x.type, int]]],
):
    reveal_type(names)  # revealed: tuple[Literal["a"]]
```

## Empty results

Iterating the empty tuple or `Never` produces no elements. An empty `Union` is `Never`.

```py
from typing import Never, Union
from ty_extensions import IsAssignable, Iter

def _(
    a: tuple[*[list[t] for t in Iter[tuple[()]]]],
    b: tuple[*[list[t] for t in Iter[Never]]],
    c: Union[*[list[t] for t in Iter[tuple[()]]]],
    d: Union[*[t for t in Iter[tuple[int, str]] if IsAssignable[t, bytes]]],
):
    reveal_type(a)  # revealed: tuple[()]
    reveal_type(b)  # revealed: tuple[()]
    reveal_type(c)  # revealed: Never
    reveal_type(d)  # revealed: Never
```

## Iterating `Any`

Iterating `Any` produces an unknown number of unknown elements: `tuple[...]` becomes
`tuple[Any, ...]` and `Union[...]` becomes `Any`.

```py
from typing import Any, Union
from ty_extensions import Iter

def _(
    a: tuple[*[list[t] for t in Iter[Any]]],
    b: Union[*[list[t] for t in Iter[Any]]],
):
    reveal_type(a)  # revealed: tuple[Any, ...]
    reveal_type(b)  # revealed: Any
```

## Iterating unions and members

`FromUnion` turns a union into a tuple, and `Attrs` / `Members` produce tuples of members.

```py
from typing import Union
from ty_extensions import Attrs, FromUnion, IsEquivalent, Iter

class A:
    x: int
    y: str

def _(
    a: Union[*[list[t] for t in Iter[FromUnion[int | str]]]],
    b: tuple[*[m.type for m in Iter[Attrs[A]]]],
    c: Union[*[m.name for m in Iter[Attrs[A]]]],
    d: IsEquivalent[tuple[*[m for m in Iter[Attrs[A]]]], Attrs[A]],
):
    reveal_type(a)  # revealed: list[int] | list[str]
    reveal_type(b)  # revealed: tuple[int, str]
    reveal_type(c)  # revealed: Literal["x", "y"]
    reveal_type(d)  # revealed: Literal[True]
```

## Deferred evaluation in generic aliases

If the iterated type or a condition mentions a type variable, the comprehension is evaluated once
the type variable is specialized.

```py
from typing import Union
from ty_extensions import FromUnion, IsAssignable, Iter

type Ints[T] = tuple[*[x for x in Iter[T] if IsAssignable[x, int]]]
type Matching[T, U] = tuple[*[x for x in Iter[T] if IsAssignable[x, U]]]
type Lists[T] = Union[*[list[x] for x in Iter[FromUnion[T]]]]

def _(
    a: Ints[tuple[int, str, bool, bytes]],
    b: Matching[tuple[int, str, bytes], str | bytes],
    c: Lists[int | str],
    d: Lists[int],
):
    reveal_type(a)  # revealed: tuple[int, bool]
    reveal_type(b)  # revealed: tuple[str, bytes]
    reveal_type(c)  # revealed: list[int] | list[str]
    reveal_type(d)  # revealed: list[int]
```

## Deferred evaluation in generic functions

Inside the generic function, the comprehension is displayed unevaluated. At the call site, it is
evaluated with the inferred type.

```py
from ty_extensions import Attrs, Iter

def wrap[T](
    x: T,
    items: tuple[*[list[t] for t in Iter[T]]],
    names: tuple[*[m.name for m in Iter[Attrs[T]]]],
):
    reveal_type(items)  # revealed: tuple[*[list[t] for t in Iter[T@wrap]]]
    reveal_type(names)  # revealed: tuple[*[m.name for m in Iter[Attrs[T@wrap]]]]

def lists[T](x: T) -> tuple[*[list[t] for t in Iter[T]]]:
    raise NotImplementedError

def _(pair: tuple[int, str]):
    reveal_type(lists(pair))  # revealed: tuple[list[int], list[str]]
```

## Nested comprehensions

The element type can contain another comprehension. Each comprehension has its own loop variable: an
inner variable with the same name shadows the outer one only inside the inner comprehension, and an
inner comprehension can refer to the variable of the outer one.

```py
from ty_extensions import Iter

def _(
    shadow: tuple[*[tuple[*[list[p] for p in Iter[tuple[int, str]]]] for p in Iter[tuple[bytes]]]],
    outer: tuple[*[tuple[*[dict[k, v] for v in Iter[tuple[int, str]]]] for k in Iter[tuple[bytes, bool]]]],
    after: tuple[*[tuple[*[p for p in Iter[tuple[bytes]]], p] for p in Iter[tuple[int, str]]]],
):
    reveal_type(shadow)  # revealed: tuple[tuple[list[int], list[str]]]
    # revealed: tuple[tuple[dict[bytes, int], dict[bytes, str]], tuple[dict[bool, int], dict[bool, str]]]
    reveal_type(outer)
    # After the inner comprehension, `p` refers to the outer loop variable again.
    reveal_type(after)  # revealed: tuple[tuple[bytes, int], tuple[bytes, str]]
```

## Loop variables are local to the comprehension

The loop variable is not visible outside the comprehension, and it does not affect a name with the
same spelling outside of it.

```py
from ty_extensions import Iter

class p: ...

def _(a: tuple[*[list[p] for p in Iter[tuple[int]]]], b: p):
    reveal_type(a)  # revealed: tuple[list[int]]
    reveal_type(b)  # revealed: p

def _(a: tuple[*[list[q] for q in Iter[tuple[int]]]]):
    # error: [unresolved-reference]
    reveal_type(q)  # revealed: Unknown
```

## String annotations

```py
from ty_extensions import Iter

def _(a: "tuple[*[list[t] for t in Iter[tuple[int, str]]]]"):
    reveal_type(a)  # revealed: tuple[list[int], list[str]]
```

[PEP 827]: https://peps.python.org/pep-0827/
