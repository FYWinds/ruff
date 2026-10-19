# Regressions: generic inference from synthesized protocols

```toml
[environment]
python-version = "3.14"
```

A protocol synthesized by `NewProtocol` has no nominal class, so a type variable in a formal
synthesized protocol is solved by comparing the protocols' members structurally. This works the
same whether the argument's type is still the alias (`Q[int]`) or has already been evaluated to the
synthesized protocol, as for a value read from an attribute.

## Values of every origin

```py
from typing import Literal
from ty_extensions import Member, NewProtocol

type Q[T] = NewProtocol[Member[Literal["v"], T]]

class HQ:
    q: Q[int]
    qs: list[Q[int]]

def g[T](x: Q[T]) -> T:
    return x.v

def gs[T](xs: list[Q[T]]) -> T:
    return xs[0].v

def make() -> Q[str]:
    raise NotImplementedError

def _(h: HQ, p: Q[bytes], ps: list[Q[bytes]]):
    reveal_type(h.q)  # revealed: <Protocol with members 'v'>
    reveal_type(g(h.q))  # revealed: int
    reveal_type(g(p))  # revealed: bytes
    reveal_type(g(make()))  # revealed: str
    reveal_type(gs(h.qs))  # revealed: int
    reveal_type(gs(ps))  # revealed: bytes

    q = h.q
    reveal_type(g(q))  # revealed: int
```

## Structural matches from other protocols and classes

The argument does not need to come from the same alias: any type with a matching member solves
the type variable.

```py
from typing import Literal, Protocol
from ty_extensions import Member, NewProtocol

type Q[T] = NewProtocol[Member[Literal["v"], T]]
type R = NewProtocol[Member[Literal["v"], int], Member[Literal["w"], str]]

class P(Protocol):
    v: float

class C:
    v: bytes

def g[T](x: Q[T]) -> T:
    return x.v

def _(r: R, p: P, c: C):
    reveal_type(g(r))  # revealed: int
    reveal_type(g(p))  # revealed: float
    reveal_type(g(c))  # revealed: bytes
```

## Recursive synthesized protocols

```py
from typing import Literal
from ty_extensions import Member, NewProtocol

type Chain[T] = NewProtocol[Member[Literal["next"], Chain[T]], Member[Literal["value"], T]]

class Link:
    next: Chain[int]
    value: int

def first[T](chain: Chain[T]) -> T:
    return chain.value

def _(link: Link, chain: Chain[str]):
    reveal_type(first(link))  # revealed: int
    reveal_type(first(link.next))  # revealed: int
    reveal_type(first(link.next.next))  # revealed: int
    reveal_type(first(chain.next))  # revealed: str
```

## Type variables the solver cannot reach

A type variable that only influences which members exist (here, through `Attrs[T]`) cannot be
solved from an argument: that would require inverting the type operator. The formal parameter
type stays unevaluated while `T` is unknown, and the type variable is solved as `Unknown`.

```py
from ty_extensions import Attrs, Iter, Member, NewProtocol

type Keys[T] = NewProtocol[*[Member[c.name, int] for c in Iter[Attrs[T]]]]

class Point:
    x: int
    y: int

def keys_of[T](x: Keys[T]) -> T:
    raise NotImplementedError

def _(keys: Keys[Point]):
    reveal_type(keys_of(keys))  # revealed: Unknown
```
