# Regressions: comprehension binders

```toml
[environment]
python-version = "3.14"
```

## Substitution does not capture a comprehension binder

### A recursive alias applied to an expression with a binder

Comprehension binders with the same name share an identity. Here, the argument of the recursive
reference `Deep[GetMemberType[M, c.name]]` mentions the binder `c`, and the body of `Deep` iterates
with a binder `c` of its own. The `c` of the argument must refer to the outer comprehension, not be
captured by the inner one.

```py
from ty_extensions import Attrs, GetMemberType, IsAssignable, Iter, Member, NewProtocol

class Leaf:
    v: int

class Mid:
    leaf: Leaf

class Top:
    mid: Mid

type Deep[M] = NewProtocol[
    *[Member[c.name, Deep[GetMemberType[M, c.name]] if IsAssignable[c.type, Mid | Leaf] else c.type] for c in Iter[Attrs[M]]]
]

def _(a: Deep[Top]):
    reveal_type(a.mid.leaf.v)  # revealed: int
```

### An alias with a comprehension applied to a binder

The same holds for an alias with a comprehension that is applied to a binder of the same name, and
for binders with different names:

```py
from ty_extensions import Attrs, GetMemberType, Iter

class Leaf:
    v: int

class Mid:
    leaf: Leaf
    n: str

class Top:
    mid: Mid

type InnerSame[M] = tuple[*[GetMemberType[M, c.name] for c in Iter[Attrs[M]]]]
type OuterSame[M] = tuple[*[InnerSame[c.type] for c in Iter[Attrs[M]]]]

type InnerOther[M] = tuple[*[GetMemberType[M, d.name] for d in Iter[Attrs[M]]]]
type OuterOther[M] = tuple[*[InnerOther[c.type] for c in Iter[Attrs[M]]]]

def _(same: OuterSame[Top], other: OuterOther[Top]):
    reveal_type(same)  # revealed: tuple[tuple[Leaf, str]]
    reveal_type(other)  # revealed: tuple[tuple[Leaf, str]]
```

### Generic callables

The binders of a generic callable share identities with comprehension binders of the same name.
When the comprehension binder `T` is substituted for `R` in an unresolved generic callable body, the
lambda's own binder `T` is renamed, so that it does not capture it:

```py
from typing import Callable, Literal, TypeVar
from ty_extensions import GenericCallable, IsAssignable, Iter, Member, NewProtocol

T = TypeVar("T")

type Method[R] = Member[
    Literal["m"],
    GenericCallable[tuple[T], lambda T: Callable[[T], R] if IsAssignable[R, int] else Callable[[T], str]],
]
type WithMethods[Ts] = NewProtocol[*[Method[T] for T in Iter[Ts]]]

def _(p: WithMethods[tuple[int]], q: WithMethods[tuple[bytes]]):
    reveal_type(p.m)  # revealed: [T](T, /) -> int
    reveal_type(p.m("a"))  # revealed: int
    reveal_type(q.m("a"))  # revealed: str
```

## Nested comprehensions are unchanged

An inner binder with the same name as an outer one shadows it; binders with different names are
independent.

```py
from ty_extensions import Iter

type Shadowed[A, B] = tuple[*[tuple[x, *[x for x in Iter[B]]] for x in Iter[A]]]
type Product[A, B] = tuple[*[tuple[x, y] for x in Iter[A] for y in Iter[B]]]

def _(s: Shadowed[tuple[int, str], tuple[bytes]], p: Product[tuple[int, str], tuple[bytes]]):
    reveal_type(s)  # revealed: tuple[tuple[int, bytes], tuple[str, bytes]]
    reveal_type(p)  # revealed: tuple[tuple[int, bytes], tuple[str, bytes]]
```

## Recursive alias applied to a comprehension binder

Substituting an element for a comprehension binder replaces the arguments of an alias, so that a
recursive alias applied to the binder is evaluated with the element. (Mapping the expanded value of
the alias instead left its recursive references unmapped, so the comprehension was never
evaluated.)

```py
from typing import Literal
from ty_extensions import Concat, GetArg, IsAssignable, IsEquivalent, Iter, Length, Slice, Uppercase

type Head[S] = Slice[S, Literal[0], Literal[1]]
type Tail[S] = Slice[S, Literal[1], None]
type Camel[S] = (
    S
    if IsEquivalent[S, Literal[""]]
    else Concat[Uppercase[Head[Tail[S]]], Camel[Tail[Tail[S]]]]
    if IsEquivalent[Head[S], Literal["_"]]
    else Concat[Head[S], Camel[Tail[S]]]
)
type Rev[T] = tuple[()] if IsAssignable[Length[T], Literal[0]] else tuple[*Rev[Slice[T, Literal[1], None]], GetArg[T, tuple, Literal[0]]]

type A[T] = tuple[*[Camel[x] for x in Iter[T]]]
type B[T] = tuple[*[Rev[x] for x in Iter[T]]]

def _(a: A[tuple[Literal["a_b"], Literal["cd"]]], b: B[tuple[tuple[int, str]]]):
    reveal_type(a)  # revealed: tuple[Literal["aB"], Literal["cd"]]
    reveal_type(b)  # revealed: tuple[tuple[str, int]]

def _(b: tuple[*[Rev[x] for x in Iter[tuple[tuple[int, str]]]]]):
    reveal_type(b)  # revealed: tuple[tuple[str, int]]
```

## Recursive references to an alias that is being evaluated

A recursive reference to an alias that is being evaluated stays an alias:

```py
from ty_extensions import Attrs, IsAssignable, Iter

class Node:
    value: int
    next: "Node"

type Flat[T] = tuple[*[Flat[c.type] if IsAssignable[c.type, Node] else c.type for c in Iter[Attrs[T]]]]

def _(f: Flat[Node]):
    reveal_type(f)  # revealed: tuple[int, Flat[Node]]
    reveal_type(f[1])  # revealed: tuple[int, Flat[Node]]
```
