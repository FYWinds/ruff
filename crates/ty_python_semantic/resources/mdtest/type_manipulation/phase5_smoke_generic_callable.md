# Generic callables (phase 5 smoke tests)

`GenericCallable[Vs, lambda <vs>: Ty]` from [PEP 827] is a callable with its own type variables, like
the type of a generic function. `Vs` is a tuple of type variables, which only provides their bounds,
constraints and defaults; the lambda's parameters are new type variables that are bound by the
callable. Following the PEP, it may only be used directly as the type argument of `Member`.

```toml
[environment]
python-version = "3.14"
```

## Generic methods in a protocol

### A generic identity method

```py
from typing import Any, Callable, Literal, TypeVar
from ty_extensions import GenericCallable, Member, NewProtocol, Param, Params

T = TypeVar("T")

type Identity = NewProtocol[
    Member[
        Literal["ident"],
        GenericCallable[
            tuple[T],
            lambda T: Callable[Params[Param[Literal["self"], Any], Param[Literal["x"], T]], T],
        ],
        Literal["ClassVar"],
    ]
]

class Impl:
    def ident[U](self, x: U) -> U:
        return x

def _(p: Identity):
    reveal_type(p.ident)  # revealed: [T](x: T) -> T
    reveal_type(p.ident(1))  # revealed: Literal[1]
    reveal_type(p.ident("a"))  # revealed: Literal["a"]

def _(impl: Impl):
    p: Identity = impl
```

### Conditional return types

The body may use the type variables in conditional types; they are evaluated at each call.

```py
from typing import Any, Callable, Literal, TypeVar
from ty_extensions import GenericCallable, IsAssignable, Member, NewProtocol, Param, Params

T = TypeVar("T")

type Processor = NewProtocol[
    Member[
        Literal["process"],
        GenericCallable[
            tuple[T],
            lambda T: Callable[
                Params[Param[Literal["self"], Any], Param[Literal["x"], T]],
                T if IsAssignable[T, list[Any]] else list[T],
            ],
        ],
        Literal["ClassVar"],
    ]
]

class Def:
    def process[U](self, x: U) -> U if IsAssignable[U, list[Any]] else list[U]:
        raise NotImplementedError

# Calls behave as for the equivalent generic method.
def _(p: Processor, d: Def):
    reveal_type(p.process([1]))  # revealed: list[int]
    reveal_type(d.process([1]))  # revealed: list[int]
    reveal_type(p.process(1))  # revealed: list[Literal[1]]
    reveal_type(d.process(1))  # revealed: list[Literal[1]]
```

### List-form callables

```py
from typing import Callable, Literal, TypeVar
from ty_extensions import GenericCallable, Member, NewProtocol

T = TypeVar("T")
U = TypeVar("U")

type Pair = NewProtocol[
    Member[
        Literal["pair"],
        GenericCallable[tuple[T, U], lambda A, B: Callable[[A, B], tuple[A, B]]],
    ]
]

def pair[A, B](a: A, b: B) -> tuple[A, B]:
    return (a, b)

def _(p: Pair):
    reveal_type(p.pair)  # revealed: [A, B](A, B, /) -> tuple[A, B]
    reveal_type(p.pair(1, "a"))  # revealed: tuple[Literal[1], Literal["a"]]
    reveal_type(pair(1, "a"))  # revealed: tuple[Literal[1], Literal["a"]]
```

## Bounds come from `Vs`

```py
from typing import Any, Callable, Literal, TypeVar
from ty_extensions import GenericCallable, Member, NewProtocol, Param, Params

N = TypeVar("N", bound=int)

type Doubler = NewProtocol[
    Member[
        Literal["double"],
        GenericCallable[
            tuple[N],
            lambda N: Callable[Params[Param[Literal["self"], Any], Param[Literal["x"], N]], N],
        ],
        Literal["ClassVar"],
    ]
]

def _(p: Doubler):
    reveal_type(p.double(True))  # revealed: Literal[True]
    p.double("a")  # error: [invalid-argument-type]
```

## Generic methods from `Members`

```py
from ty_extensions import Iter, Members, NewProtocol

class Box:
    def wrap[T](self, x: T) -> list[T]:
        return [x]

type Copy[C] = NewProtocol[*[m for m in Iter[Members[C]]]]

def _(p: Copy[Box]):
    reveal_type(p.wrap(1))  # revealed: list[int]
```

## Bodies that depend on outer type variables

### Outer type variables only

A body that is an unresolved operator over outer type variables is evaluated once they are known.

```py
from typing import Callable, Literal, TypeVar
from ty_extensions import GenericCallable, IsAssignable, Member, NewProtocol

T = TypeVar("T")

type Converter[X] = NewProtocol[
    Member[
        Literal["convert"],
        GenericCallable[
            tuple[T],
            lambda T: Callable[[T], X] if IsAssignable[X, int] else Callable[[T], str],
        ],
    ]
]

def _(a: Converter[bool], b: Converter[bytes]):
    reveal_type(a.convert)  # revealed: [T](T, /) -> bool
    reveal_type(b.convert)  # revealed: [T](T, /) -> str
```

### The lambda's own type variables

A body that is an unresolved operator over the lambda's own type variables is not supported yet:

```py
from typing import Callable, Literal, TypeVar
from ty_extensions import GenericCallable, IsAssignable, Member

T = TypeVar("T")

type M = Member[
    Literal["m"],
    GenericCallable[
        tuple[T],
        # error: [invalid-type-form] "not supported yet"
        lambda T: Callable[[T], T] if IsAssignable[T, int] else Callable[[T], str],
    ],
]
```

## Only allowed as the type of a `Member`

```py
from typing import Callable, Literal, TypeVar
from ty_extensions import GenericCallable, Member

T = TypeVar("T")

def _(
    a: GenericCallable[tuple[T], lambda T: Callable[[T], T]],  # error: [invalid-type-form]
    b: list[GenericCallable[tuple[T], lambda T: Callable[[T], T]]],  # error: [invalid-type-form]
    # error: [invalid-type-form]
    c: Member[Literal["c"], list[GenericCallable[tuple[T], lambda T: Callable[[T], T]]]],
):
    reveal_type(a)  # revealed: Unknown
```

## Invalid forms

```py
from typing import Callable, Literal, TypeVar
from ty_extensions import GenericCallable, Member

T = TypeVar("T")
U = TypeVar("U")

type A = Member[Literal["a"], GenericCallable[tuple[int], lambda T: Callable[[T], T]]]  # error: [invalid-type-form]
type B = Member[Literal["b"], GenericCallable[tuple[T, U], lambda T: Callable[[T], T]]]  # error: [invalid-type-form]
type C = Member[Literal["c"], GenericCallable[tuple[T], Callable[[T], T]]]  # error: [invalid-type-form]
type D = Member[Literal["d"], GenericCallable[tuple[T], lambda T: list[T]]]  # error: [invalid-type-form]
type E = Member[Literal["e"], GenericCallable[tuple[T], lambda T=int: Callable[[T], T]]]  # error: [invalid-type-form]
type F = Member[Literal["f"], GenericCallable[tuple[T], lambda *T: Callable[[T], T]]]  # error: [invalid-type-form]
type G = Member[Literal["g"], GenericCallable[tuple[T]]]  # error: [invalid-type-form]
# error: [invalid-type-form] "can only be used directly as the type argument of a `Member`"
type H = Member[Literal["h"], "GenericCallable[tuple[T], lambda T: Callable[[T], T]]"]
```

A lambda anywhere else in a type expression is still an error:

```py
def f(x: lambda: int):  # error: [invalid-type-form]
    pass

def g(x: list[lambda y: y]):  # error: [invalid-type-form]
    pass

def h(x: "lambda y: y"):  # error: [invalid-type-form]
    pass

def i(x: lambda y, z=1: y):  # error: [invalid-type-form]
    pass

type L = lambda T: T  # error: [invalid-type-form]

# Only the lambda is reported, not its body.
def j(x: lambda: 1):  # error: [invalid-type-form] "`lambda` expressions are not allowed in parameter annotations"
    pass

def k(x: lambda: [y for y in range(3)]):  # error: [invalid-type-form]
    pass

def m(x: lambda: (lambda: undefined_name)):  # error: [invalid-type-form]
    pass

def n(x: lambda *args, **kwargs: args):  # error: [invalid-type-form]
    pass

def o() -> lambda: o:  # error: [invalid-type-form]
    pass
```

## String annotations

```py
from typing import Callable, Literal, TypeVar
from ty_extensions import GenericCallable, Member, NewProtocol

T = TypeVar("T")

def _(p: "NewProtocol[Member[Literal['f'], GenericCallable[tuple[T], lambda T: Callable[[T], T]]]]"):
    reveal_type(p.f)  # revealed: [T](T, /) -> T
```

## Value-indexed positions

```py
from typing import Callable, Literal, TypeAlias, TypeVar, cast
from ty_extensions import GenericCallable, Member

T = TypeVar("T")

# error: [invalid-type-form]
Implicit = Member[Literal["a"], GenericCallable[tuple[T], lambda T: Callable[[T], T]]]

# error: [invalid-type-form]
Explicit: TypeAlias = Member[Literal["a"], GenericCallable[tuple[T], lambda T: Callable[[T], T]]]

# error: [invalid-type-form]
B = TypeVar("B", bound=Member[Literal["a"], GenericCallable[tuple[T], lambda T: Callable[[T], T]]])

# error: [invalid-type-form]
Other = list[lambda x: x]

def _(x: object):
    # error: [invalid-type-form]
    y = cast(Member[Literal["a"], GenericCallable[tuple[T], lambda T: Callable[[T], T]]], x)
    # The rejected `GenericCallable` is `Unknown`; the `Member` around it is kept.
    reveal_type(y)  # revealed: Member[Literal["a"], Unknown, Never, Never, Never]
```

## Stubs

```pyi
from typing import Callable, Literal, TypeVar
from ty_extensions import GenericCallable, Member, NewProtocol

T = TypeVar("T")

# error: [invalid-type-form]
Implicit = Member[Literal["a"], GenericCallable[tuple[T], lambda T: Callable[[T], T]]]

def f(p: NewProtocol[Member[Literal["g"], GenericCallable[tuple[T], lambda T: Callable[[T], T]]]]) -> None: ...
def g(x: lambda: int) -> None: ...  # error: [invalid-type-form]
```

[PEP 827]: https://peps.python.org/pep-0827/
