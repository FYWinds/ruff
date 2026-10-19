# Type comprehensions with several `for` clauses

A type comprehension can have several `for` clauses, as in Python. Each one iterates over `Iter[...]`
and binds a single name. Later clauses (and their `if` filters) can refer to the variables of earlier
ones. Each `if` filters the elements of the `for` clause just before it. The results are flattened in
the usual order: the last `for` varies fastest.

```toml
[environment]
python-version = "3.14"
```

## Cross products

```py
from ty_extensions import Iter

def _(
    pairs: tuple[*[tuple[a, b] for a in Iter[tuple[int, str]] for b in Iter[tuple[bytes, bool]]]],
    empty: tuple[*[tuple[a, b] for a in Iter[tuple[int, str]] for b in Iter[tuple[()]]]],
    dicts: tuple[*[dict[k, v] for k in Iter[tuple[int]] for v in Iter[tuple[str]]]],
):
    # revealed: tuple[tuple[int, bytes], tuple[int, bool], tuple[str, bytes], tuple[str, bool]]
    reveal_type(pairs)
    reveal_type(empty)  # revealed: tuple[()]
    reveal_type(dicts)  # revealed: tuple[dict[int, str]]
```

## Iterables that depend on earlier variables

```py
from ty_extensions import Iter

type Flatten[T] = tuple[*[x for row in Iter[T] for x in Iter[row]]]

def _(
    a: Flatten[tuple[tuple[int, str], tuple[()], tuple[bytes]]],
    b: Flatten[tuple[()]],
):
    reveal_type(a)  # revealed: tuple[int, str, bytes]
    reveal_type(b)  # revealed: tuple[()]
```

## Filters belong to the preceding `for`

The first condition only sees `a`; the second one sees both variables.

```py
from ty_extensions import IsAssignable, IsEquivalent, Iter

type Ts = tuple[int, str]

def _(
    a: tuple[*[tuple[a, b] for a in Iter[Ts] if IsAssignable[a, int] for b in Iter[Ts] if IsEquivalent[a, b]]],
    b: tuple[*[tuple[a, b] for a in Iter[Ts] for b in Iter[Ts] if IsEquivalent[a, b]]],
    c: tuple[*[tuple[a, b] for a in Iter[Ts] for b in Iter[Ts] if not IsEquivalent[a, b]]],
):
    reveal_type(a)  # revealed: tuple[tuple[int, int]]
    reveal_type(b)  # revealed: tuple[tuple[int, int], tuple[str, str]]
    reveal_type(c)  # revealed: tuple[tuple[int, str], tuple[str, int]]
```

## Shadowing

As in Python, a later `for` clause can reuse a name; from there on, the name refers to the new
variable. Here the second clause iterates over the tuple bound by the first one.

```py
from ty_extensions import Iter

def _(a: tuple[*[x for x in Iter[tuple[tuple[int, str]]] for x in Iter[x]]]):
    reveal_type(a)  # revealed: tuple[int, str]
```

## Members of several classes

```py
from typing import Union
from ty_extensions import Attrs, Iter

class A:
    x: int

class B:
    y: str
    z: bytes

def _(names: Union[*[m.name for c in Iter[tuple[A, B]] for m in Iter[Attrs[c]]]]):
    reveal_type(names)  # revealed: Literal["x", "y", "z"]
```

## Unbounded inner tuples

An unbounded inner tuple is spliced into the result. An unbounded outer tuple cannot be used with a
nested `for`, because each of its elements must map to exactly one type.

```py
from ty_extensions import Iter

def _(
    a: tuple[*[x for row in Iter[tuple[tuple[int, ...]]] for x in Iter[row]]],
    b: tuple[*[x for row in Iter[tuple[tuple[int], tuple[str, ...]]] for x in Iter[row]]],
    # error: [invalid-type-form] "Each element of an unbounded tuple must map to exactly one type"
    c: tuple[*[x for row in Iter[tuple[tuple[int], ...]] for x in Iter[row]]],
):
    reveal_type(a)  # revealed: tuple[int, ...]
    reveal_type(b)  # revealed: tuple[int, *tuple[str, ...]]
    reveal_type(c)  # revealed: Unknown
```

## Several `for` clauses in `any()` and `all()`

```py
from ty_extensions import IsEquivalent, Iter

type Overlap[As, Bs] = any(IsEquivalent[a, b] for a in Iter[As] for b in Iter[Bs])

def _(
    a: Overlap[tuple[int, str], tuple[bytes, str]],
    b: Overlap[tuple[int, str], tuple[bytes, bool]],
):
    reveal_type(a)  # revealed: Literal[True]
    reveal_type(b)  # revealed: Literal[False]
```

## Deferred evaluation

```py
from ty_extensions import Iter

def f[A, B](x: A, y: B, pairs: tuple[*[tuple[a, b] for a in Iter[A] for b in Iter[B]]]):
    reveal_type(pairs)  # revealed: tuple[*[tuple[a, b] for a in Iter[A@f] for b in Iter[B@f]]]

def product[A, B](x: A, y: B) -> tuple[*[tuple[a, b] for a in Iter[A] for b in Iter[B]]]:
    raise NotImplementedError

def _(x: tuple[int, str], y: tuple[bytes]):
    reveal_type(product(x, y))  # revealed: tuple[tuple[int, bytes], tuple[str, bytes]]
```

## Still unsupported

Every `for` clause must iterate over `Iter[...]` and bind a single name.

```py
from ty_extensions import Iter

def _(
    # error: [invalid-type-form] "The iterable of a type comprehension must be `Iter[...]`"
    a: tuple[*[tuple[a, b] for a in Iter[tuple[int]] for b in tuple[str]]],
    # error: [invalid-type-form] "The target of a type comprehension must be a single name"
    b: tuple[*[a for a in Iter[tuple[int]] for b, c in Iter[tuple[tuple[int, str]]]]],
):
    reveal_type(a)  # revealed: Unknown
    reveal_type(b)  # revealed: Unknown
```

[PEP 827]: https://peps.python.org/pep-0827/
