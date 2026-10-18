# `NewProtocol`

`NewProtocol[*Ms]` creates a structural protocol with the given `Member`s. A member is a method if it
has the `ClassVar` qualifier and a callable type; otherwise it is an attribute, with the `ClassVar`
and `Final` qualifiers preserved. `NotRequired`, `ReadOnly`, the initializer and the definer are
ignored.

The synthesized protocol has no name. ty displays it as `<Protocol with members ...>`, listing the
members alphabetically, so the tests below check its structure through attribute access and
assignability instead.

```toml
[environment]
python-version = "3.14"
```

## Attributes

```py
from typing import Literal
from ty_extensions import Member, NewProtocol

type P = NewProtocol[Member[Literal["b"], str], Member[Literal["a"], int]]

def _(p: P):
    reveal_type(p)  # revealed: <Protocol with members 'a', 'b'>
    reveal_type(p.a)  # revealed: int
    reveal_type(p.b)  # revealed: str
    p.c  # error: [unresolved-attribute]
```

Like `Union`, `NewProtocol` accepts unpacked tuples of members as well as comprehensions.

```py
from typing import Literal
from ty_extensions import Member, NewProtocol

def _(p: NewProtocol[*tuple[Member[Literal["x"], int]], Member[Literal["y"], str]]):
    reveal_type(p.x)  # revealed: int
    reveal_type(p.y)  # revealed: str
```

## Assignability of nominal classes

A class is assignable to the synthesized protocol if it has all the members. Attribute members are
mutable, so their types are invariant: an attribute of type `bool` does not satisfy an attribute
member of type `int`.

```py
from typing import Literal
from ty_extensions import Member, NewProtocol, static_assert
from ty_extensions._internal import is_assignable_to

type P = NewProtocol[Member[Literal["a"], int], Member[Literal["b"], str]]

class Good:
    a: int
    b: str

class Extra:
    a: int
    b: str
    c: bytes

class Missing:
    a: int

class Narrower:
    a: bool
    b: str

static_assert(is_assignable_to(Good, P))
static_assert(is_assignable_to(Extra, P))
static_assert(not is_assignable_to(Missing, P))
static_assert(not is_assignable_to(Narrower, P))

def _(good: Good, missing: Missing):
    x: P = good
    y: P = missing  # error: [invalid-assignment]
```

A protocol class written by hand with the same members is mutually assignable with the synthesized
one.

```py
from typing import Literal, Protocol
from ty_extensions import Member, NewProtocol, static_assert
from ty_extensions._internal import is_assignable_to

class HasAB(Protocol):
    a: int
    b: str

type Q = NewProtocol[Member[Literal["a"], int], Member[Literal["b"], str]]

static_assert(is_assignable_to(HasAB, Q))
static_assert(is_assignable_to(Q, HasAB))
```

## Methods and callable attributes

Members with the `ClassVar` qualifier and a callable type become methods. The callable includes the
receiver as its first parameter, which is how `Members` reports methods, so the protocol of all
members of a class accepts the class itself.

```py
from typing import Literal
from ty_extensions import Iter, Members, NewProtocol, static_assert
from ty_extensions._internal import is_assignable_to

class Greeter:
    prefix: str

    def greet(self, name: str) -> str:
        return self.prefix + name

class WrongGreeter:
    prefix: str

    def greet(self, name: int) -> str:
        return self.prefix

type Interface[T] = NewProtocol[*[m for m in Iter[Members[T]]]]

def _(g: Interface[Greeter]):
    reveal_type(g.greet("world"))  # revealed: str
    reveal_type(g.prefix)  # revealed: str

static_assert(is_assignable_to(Greeter, Interface[Greeter]))
static_assert(not is_assignable_to(WrongGreeter, Interface[Greeter]))
```

Without `ClassVar`, a callable member is an attribute holding a callable, which is not bound to the
instance on access:

```py
from typing import Callable, Literal
from ty_extensions import Member, NewProtocol

type HasCallback = NewProtocol[Member[Literal["callback"], Callable[[int], str]]]

def _(h: HasCallback):
    reveal_type(h.callback(1))  # revealed: str
```

## `ClassVar` and `Final` attributes

```py
from typing import ClassVar, Literal
from ty_extensions import Member, NewProtocol, static_assert
from ty_extensions._internal import is_assignable_to

type Limited = NewProtocol[Member[Literal["limit"], int, Literal["ClassVar"]]]
type Identified = NewProtocol[Member[Literal["id"], int, Literal["Final"]]]

class WithLimit:
    limit: ClassVar[int] = 10

static_assert(is_assignable_to(WithLimit, Limited))

def _(limited: Limited, identified: Identified):
    reveal_type(limited.limit)  # revealed: int
    reveal_type(identified.id)  # revealed: int
```

## Ignored member components

`NotRequired`, `ReadOnly`, the initializer and the definer do not change the protocol.

```py
from typing import Literal, Never
from ty_extensions import IsEquivalent, Member, NewProtocol

class A: ...

type Plain = NewProtocol[Member[Literal["a"], int]]

def _(
    a: IsEquivalent[NewProtocol[Member[Literal["a"], int, Literal["NotRequired", "ReadOnly"]]], Plain],
    b: IsEquivalent[NewProtocol[Member[Literal["a"], int, Never, Literal[1], A]], Plain],
):
    reveal_type(a)  # revealed: Literal[True]
    reveal_type(b)  # revealed: Literal[True]
```

## Duplicate names

If two members have the same name, the later one wins.

```py
from typing import Literal
from ty_extensions import Member, NewProtocol

def _(p: NewProtocol[Member[Literal["a"], int], Member[Literal["a"], str]]):
    reveal_type(p.a)  # revealed: str
```

## Empty protocols

A protocol without members is satisfied by every object.

```py
from ty_extensions import Iter, NewProtocol, static_assert
from ty_extensions._internal import is_assignable_to

type Empty = NewProtocol[*[m for m in Iter[tuple[()]]]]

static_assert(is_assignable_to(int, Empty))
static_assert(is_assignable_to(None, Empty))
```

## Dynamic members

If the members come from iterating `Any`, the protocol is `Any`.

```py
from typing import Any
from ty_extensions import Iter, NewProtocol

def _(p: NewProtocol[*[m for m in Iter[Any]]]):
    reveal_type(p)  # revealed: Any
```

## Deferred evaluation

```py
from ty_extensions import Attrs, Iter, NewProtocol

class A:
    x: int

type Copy[T] = NewProtocol[*[m for m in Iter[Attrs[T]]]]

def f[T](x: T, p: NewProtocol[*[m for m in Iter[Attrs[T]]]]):
    reveal_type(p)  # revealed: NewProtocol[*[m for m in Iter[Attrs[T@f]]]]

def _(c: Copy[A]):
    reveal_type(c.x)  # revealed: int
```
