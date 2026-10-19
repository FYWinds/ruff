# Regressions: recursive synthesized types

```toml
[environment]
python-version = "3.14"
```

A protocol or `TypedDict` synthesized by `NewProtocol`/`NewTypedDict` may refer to itself through
its type alias. The reference stays a lazy alias inside the synthesized members, the same way a
member of a recursive protocol class refers to its class, so checking it does not recurse through
the cycle.

## A self-referential `NewProtocol`

```py
from typing import Literal, Protocol
from ty_extensions import Member, NewProtocol, static_assert
from ty_extensions._internal import is_assignable_to, is_equivalent_to, is_subtype_of

type Linked = NewProtocol[Member[Literal["next"], Linked], Member[Literal["value"], int]]
type Same = NewProtocol[Member[Literal["next"], Same], Member[Literal["value"], int]]
type Other = NewProtocol[Member[Literal["next"], Other], Member[Literal["value"], str]]

class LinkedClass(Protocol):
    next: LinkedClass
    value: int

def _(x: Linked):
    reveal_type(x)  # revealed: <Protocol with members 'next', 'value'>
    reveal_type(x.next)  # revealed: <Protocol with members 'next', 'value'>
    reveal_type(x.next.next.value)  # revealed: int
    # error: [unresolved-attribute]
    x.next.missing

static_assert(is_equivalent_to(Linked, Same))
static_assert(is_subtype_of(Linked, Same))
static_assert(is_equivalent_to(Linked, LinkedClass))
static_assert(not is_assignable_to(Linked, Other))
```

## Assigning class instances

The members are mutable attributes, so a class matches them only if its attribute types are
equivalent to the member types, as for a recursive protocol class. With `Final` members the
attributes are read-only, and their types only need to be assignable.

```py
from typing import Literal, Protocol
from ty_extensions import Member, NewProtocol

type Linked = NewProtocol[Member[Literal["next"], Linked], Member[Literal["value"], int]]
type ReadOnlyLinked = NewProtocol[
    Member[Literal["next"], ReadOnlyLinked, Literal["Final"]],
    Member[Literal["value"], int, Literal["Final"]],
]

class LinkedClass(Protocol):
    next: LinkedClass
    value: int

class Node:
    next: Node
    value: int

class Holder:
    next: Linked
    value: int

class Bad:
    next: int
    value: int

def _(node: Node, holder: Holder, bad: Bad):
    a: Linked = holder
    b: ReadOnlyLinked = node
    reveal_type(b.next.next.value)  # revealed: int

    # error: [invalid-assignment] "Object of type `Node` is not assignable to `Linked`"
    c: Linked = node
    # error: [invalid-assignment] "Object of type `Node` is not assignable to `LinkedClass`"
    d: LinkedClass = node
    # error: [invalid-assignment] "Object of type `Bad` is not assignable to `Linked`"
    e: Linked = bad
    # error: [invalid-assignment] "Object of type `Literal[1]` is not assignable to `Linked`"
    f: Linked = 1
```

## A comprehension over a self-referential class

```py
from ty_extensions import Attrs, IsAssignable, Iter, Member, NewProtocol

class Node:
    next: Node
    value: int

type Tree[T] = NewProtocol[
    *[Member[c.name, Tree[c.type] if IsAssignable[c.type, Node] else c.type] for c in Iter[Attrs[T]]]
]

def _(tree: Tree[Node]):
    reveal_type(tree)  # revealed: <Protocol with members 'next', 'value'>
    reveal_type(tree.next)  # revealed: <Protocol with members 'next', 'value'>
    reveal_type(tree.next.next.value)  # revealed: int
```

## A self-referential `NewTypedDict`

```py
from typing import Literal
from ty_extensions import Member, NewTypedDict, static_assert
from ty_extensions._internal import is_equivalent_to

type Dict = NewTypedDict[Member[Literal["next"], Dict], Member[Literal["value"], int]]
type SameDict = NewTypedDict[Member[Literal["next"], SameDict], Member[Literal["value"], int]]

static_assert(is_equivalent_to(Dict, SameDict))

def _(d: Dict):
    reveal_type(d)  # revealed: <TypedDict with items 'next', 'value'>
    reveal_type(d["next"])  # revealed: <TypedDict with items 'next', 'value'>
    reveal_type(d["next"]["next"]["value"])  # revealed: int

    same: SameDict = d
    nested: Dict = {"next": d, "value": 1}
    # error: [invalid-argument-type]
    bad: Dict = {"next": {"next": 1, "value": 2}, "value": 3}
```

## Mutual recursion

```py
from typing import Literal
from ty_extensions import Member, NewProtocol

type Ping = NewProtocol[Member[Literal["pong"], Pong], Member[Literal["value"], int]]
type Pong = NewProtocol[Member[Literal["ping"], Ping], Member[Literal["name"], str]]

def _(ping: Ping):
    reveal_type(ping.pong)  # revealed: <Protocol with members 'name', 'ping'>
    reveal_type(ping.pong.name)  # revealed: str
    reveal_type(ping.pong.ping.pong.ping.value)  # revealed: int
```

## Generic recursion

A generic self-referential alias can be solved from an argument, and an alias that grows its
arguments at each level is only expanded as far as it is accessed:

```py
from typing import Literal
from ty_extensions import Member, NewProtocol, NewTypedDict

type Chain[T] = NewProtocol[Member[Literal["next"], Chain[T]], Member[Literal["value"], T]]

class IntChain:
    next: Chain[int]
    value: int

def first[T](chain: Chain[T]) -> T:
    return chain.next.value

def _(chain: Chain[str], int_chain: IntChain):
    reveal_type(chain.next.next.value)  # revealed: str
    reveal_type(first(chain))  # revealed: str
    reveal_type(first(int_chain))  # revealed: int

type Grow[T] = NewProtocol[Member[Literal["next"], Grow[list[T]]], Member[Literal["value"], T]]
type GrowDict[T] = NewTypedDict[Member[Literal["next"], GrowDict[list[T]]], Member[Literal["value"], T]]

def _(grow: Grow[int], grow_dict: GrowDict[int]):
    reveal_type(grow.next.value)  # revealed: list[int]
    reveal_type(grow.next.next.value)  # revealed: list[list[int]]
    reveal_type(grow_dict["next"]["next"]["value"])  # revealed: list[list[int]]
```

## Type variables in a recursive synthesized protocol

Searching a signature for legacy type variables visits the protocol's members once:

```py
from typing import Literal, TypeVar
from ty_extensions import Member, NewProtocol

S = TypeVar("S")

type Linked = NewProtocol[Member[Literal["next"], Linked], Member[Literal["value"], int]]
type Chain[T] = NewProtocol[Member[Literal["next"], Chain[T]], Member[Literal["value"], T]]

def keep(x: Linked, y: S) -> S:
    return y

def value(x: Chain[S]) -> S:
    return x.next.value

reveal_type(keep)  # revealed: def keep[S](x: Linked, y: S) -> S
reveal_type(value)  # revealed: def value[S](x: Chain[S]) -> S
```

## Errors raised in a nested level

A recursive alias over a chain of classes refers to the alias of the next level lazily. An error
raised at a deeper level is still reported where the outermost alias is used:

```py
from typing import Literal
from ty_extensions import Attrs, IsAssignable, Iter, Member, NewProtocol, RaiseError

class Leaf:
    name: str

class Middle:
    leaf: Leaf
    value: int

class Top:
    middle: Middle
    value: int

type NoStrings[T] = NewProtocol[
    *[
        Member[
            c.name,
            RaiseError[Literal["strings are not allowed"], c.name]
            if IsAssignable[c.type, str]
            else c.type
            if IsAssignable[c.type, int]
            else NoStrings[c.type],
        ]
        for c in Iter[Attrs[T]]
    ]
]

def _(
    middle: NoStrings[Middle],  # error: [raised-type-error] "strings are not allowed: Literal["name"]"
    top: NoStrings[Top],  # error: [raised-type-error] "strings are not allowed: Literal["name"]"
):
    reveal_type(top.middle.value)  # revealed: int
    reveal_type(top.middle.leaf.name)  # revealed: Unknown
```
