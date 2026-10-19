# Iterating tuples of unbounded length

`Iter` accepts tuples with an unbounded part, such as `tuple[int, ...]` or
`tuple[int, *tuple[str, ...], bytes]`. The fixed elements before and after the unbounded part are
handled one by one. The unbounded part is handled once, with its element type: if the element passes
the filters, the result has an unbounded part of the mapped type; otherwise the unbounded part is
dropped. `tuple[*Ts]` is evaluated once `Ts` is known.

```toml
[environment]
python-version = "3.14"
```

## Mapping

```py
from ty_extensions import Iter

type OptTuple[T] = tuple[*[x | None for x in Iter[T]]]

def _(
    a: OptTuple[tuple[int, str]],
    b: OptTuple[tuple[int, ...]],
    c: OptTuple[tuple[int, *tuple[str, ...], bool]],
):
    reveal_type(a)  # revealed: tuple[int | None, str | None]
    reveal_type(b)  # revealed: tuple[int | None, ...]
    reveal_type(c)  # revealed: tuple[int | None, *tuple[str | None, ...], bool | None]
```

## Filtering

```py
from ty_extensions import IsAssignable, Iter

type IntsOnly[T] = tuple[*[x for x in Iter[T] if IsAssignable[x, int]]]

def _(
    a: IntsOnly[tuple[str, ...]],
    b: IntsOnly[tuple[str, *tuple[int, ...]]],
    c: IntsOnly[tuple[int, *tuple[str, ...], bool]],
):
    reveal_type(a)  # revealed: tuple[()]
    reveal_type(b)  # revealed: tuple[int, ...]
    reveal_type(c)  # revealed: tuple[int, bool]
```

## Several types per element

With `Unpack[...]` as the element, each element of the iterated tuple produces several types. That
is fine for the fixed elements, but the unbounded part must map to exactly one type. Written in
place, the error is reported; inside a generic alias, it only shows up when the alias is specialized,
so it is silent and the result is `Unknown` (phase 2 rule 16).

```py
from typing import Unpack
from ty_extensions import Iter

type Pair[T] = tuple[T, T]
type Pairs[T] = tuple[*[Unpack[Pair[x]] for x in Iter[T]]]

def _(
    a: Pairs[tuple[int, str]],
    b: Pairs[tuple[int, ...]],
    # error: [invalid-type-form] "Each element of an unbounded tuple must map to exactly one type"
    c: tuple[*[Unpack[tuple[x, x]] for x in Iter[tuple[int, ...]]]],
):
    reveal_type(a)  # revealed: tuple[int, int, str, str]
    reveal_type(b)  # revealed: Unknown
    reveal_type(c)  # revealed: Unknown
```

## Splicing into `Union`

All element types, including the one of the unbounded part, end up in the union.

```py
from typing import Union
from ty_extensions import Iter

def _(
    a: Union[*[list[x] for x in Iter[tuple[int, *tuple[str, ...]]]]],
    b: Union[*tuple[int, ...]],
):
    reveal_type(a)  # revealed: list[int] | list[str]
    reveal_type(b)  # revealed: int
```

## Structural types need a fixed number of members

```py
from typing import Literal
from ty_extensions import Iter, Member, NewProtocol

def _(
    # error: [invalid-type-form] "Cannot create a protocol from an unbounded number of members"
    a: NewProtocol[*[m for m in Iter[tuple[Member[Literal["x"], int], ...]]]],
):
    reveal_type(a)  # revealed: Unknown
```

## `tuple[*Ts]`

Inside the generic function, `Ts` is unknown and the comprehension stays unevaluated. At the call
site, it is evaluated with the inferred types, including an unbounded pack.

```py
from ty_extensions import Iter

def lists[*Ts](*args: *Ts) -> tuple[*[list[t] for t in Iter[tuple[*Ts]]]]:
    raise NotImplementedError

def _(a: int, b: str, rest: tuple[bytes, ...]):
    reveal_type(lists(a, b))  # revealed: tuple[list[int], list[str]]
    reveal_type(lists())  # revealed: tuple[()]
    reveal_type(lists(a, *rest))  # revealed: tuple[list[int], *tuple[list[bytes], ...]]
```

## A typed `zip`

The `zip` example from the PEP (milestone R2, since it needs `GetArg`). Each argument contributes its
element type, looked up through its `Iterable` base class.

With the element type written in place, the result displays as plain types:

```py
from collections.abc import Iterable, Iterator
from typing import Literal
from ty_extensions import GetArg, Iter

def izip[*Ts](*args: *Ts) -> Iterator[tuple[*[GetArg[t, Iterable, Literal[0]] for t in Iter[tuple[*Ts]]]]]:
    raise NotImplementedError

def _(a: list[int], b: tuple[str, str], c: set[bytes], d: dict[str, float], rows: list[list[int]]):
    reveal_type(izip(a, b))  # revealed: Iterator[tuple[int, str]]
    reveal_type(izip(a, b, c, d))  # revealed: Iterator[tuple[int, str, bytes, str]]
    reveal_type(izip(*rows))  # revealed: Iterator[tuple[int, ...]]
```

The PEP names the element type with a helper alias, `ElemOf`. The elements of the result are then
`ElemOf[...]` aliases, which are displayed by name inside the tuple; unpacking resolves them.

```py
from collections.abc import Iterable, Iterator
from typing import Literal
from ty_extensions import GetArg, Iter

type ElemOf[T] = GetArg[T, Iterable, Literal[0]]

def zip_elems[*Ts](*args: *Ts) -> Iterator[tuple[*[ElemOf[t] for t in Iter[tuple[*Ts]]]]]:
    raise NotImplementedError

def _(a: list[int], b: tuple[str, str]):
    for x, y in zip_elems(a, b):
        reveal_type(x)  # revealed: int
        reveal_type(y)  # revealed: str
```

[PEP 827]: https://peps.python.org/pep-0827/
