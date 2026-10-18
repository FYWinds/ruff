# `GetMember` and `GetMemberType`

`GetMember[T, S]` is the `Member` named `S` of the class `T`, looked up among the same members as
`Members[T]` (attributes and methods). `GetMemberType[T, S]` is just the type of that member.

If there is no such member, `GetMember` is an error, while `GetMemberType` is `Never`.

```toml
[environment]
python-version = "3.14"
```

## Attributes

```py
from typing import ClassVar, Literal
from ty_extensions import GetMember, GetMemberType

class A:
    x: int
    y: str = "a"
    z: ClassVar[bytes] = b""

def _(
    x: GetMember[A, Literal["x"]],
    y: GetMember[A, Literal["y"]],
    z: GetMember[A, Literal["z"]],
    x_type: GetMemberType[A, Literal["x"]],
    z_type: GetMemberType[A, Literal["z"]],
):
    reveal_type(x)  # revealed: Member[Literal["x"], int, Never, Never, A]
    reveal_type(y)  # revealed: Member[Literal["y"], str, Never, Literal["a"], A]
    reveal_type(z)  # revealed: Member[Literal["z"], bytes, Literal["ClassVar"], Literal[b""], A]
    reveal_type(x_type)  # revealed: int
    reveal_type(z_type)  # revealed: bytes
```

## Inherited and generic members

```py
from typing import Literal
from ty_extensions import GetMember, GetMemberType

class Box[T]:
    item: T

class IntBox(Box[int]):
    label: str

def _(
    a: GetMemberType[Box[str], Literal["item"]],
    b: GetMemberType[IntBox, Literal["item"]],
    c: GetMember[IntBox, Literal["item"]],
):
    reveal_type(a)  # revealed: str
    reveal_type(b)  # revealed: int
    reveal_type(c)  # revealed: Member[Literal["item"], int, Never, Never, Box[int]]
```

Classes from standard library modules other than `builtins` contribute their annotated members:

```py
from collections import UserDict
from typing import Literal
from ty_extensions import GetMemberType

class Registry(UserDict[str, int]): ...

def _(data: GetMemberType[Registry, Literal["data"]]):
    reveal_type(data)  # revealed: dict[str, int]
```

## Methods and properties

Methods are found too. Their type is the callable type of the function, including `self`. A
property's type is the return type of its getter.

```py
from typing import Literal
from ty_extensions import GetMember, GetMemberType

class Greeter:
    prefix: str

    def greet(self, name: str) -> str:
        return self.prefix + name

    @property
    def loud(self) -> bool:
        return True

def _(
    greet: GetMemberType[Greeter, Literal["greet"]],
    member: GetMember[Greeter, Literal["greet"]],
    loud: GetMemberType[Greeter, Literal["loud"]],
):
    reveal_type(greet(Greeter(), "world"))  # revealed: str
    reveal_type(member.name)  # revealed: Literal["greet"]
    reveal_type(member.quals)  # revealed: Literal["ClassVar"]
    reveal_type(member.definer)  # revealed: Greeter
    reveal_type(loud)  # revealed: bool
    # `init` is `Never`, which makes the rest of the function unreachable, so it is revealed last.
    reveal_type(member.init)  # revealed: Never
```

## TypedDict items

`GetMemberType` gives the value type of a key. Whether the key is required is a qualifier, which only
`GetMember` exposes.

```py
from typing import Literal, NotRequired, TypedDict
from ty_extensions import GetMember, GetMemberType

class Movie(TypedDict):
    name: str
    year: NotRequired[int]

def _(
    year_type: GetMemberType[Movie, Literal["year"]],
    year: GetMember[Movie, Literal["year"]],
):
    reveal_type(year_type)  # revealed: int
    reveal_type(year)  # revealed: Member[Literal["year"], int, Literal["NotRequired"], Never, Never]
```

## Missing members

```py
from typing import Literal
from ty_extensions import GetMember, GetMemberType

class A:
    x: int

def _(
    a: GetMemberType[A, Literal["missing"]],
    b: GetMember[A, Literal["missing"]],  # error: [invalid-type-form]
):
    reveal_type(b)  # revealed: Unknown
    reveal_type(a)  # revealed: Never
```

## `Any`

If the class is `Any`, both operators give `Any`.

```py
from typing import Any, Literal
from ty_extensions import GetMember, GetMemberType

def _(a: GetMember[Any, Literal["x"]], b: GetMemberType[Any, Literal["x"]]):
    reveal_type(a)  # revealed: Any
    reveal_type(b)  # revealed: Any
```

## Deferred evaluation

A typed getter: the key is inferred from the argument, and the return type is looked up when `get`
is called.

```py
from typing import Literal
from ty_extensions import GetMemberType

class User:
    name: str
    age: int

def get[T, K: str](obj: T, key: K) -> GetMemberType[T, K]:
    raise NotImplementedError

def _(user: User):
    reveal_type(get(user, "name"))  # revealed: str
    reveal_type(get(user, "age"))  # revealed: int

def f[T](obj: T, value: GetMemberType[T, Literal["name"]]):
    reveal_type(value)  # revealed: GetMemberType[T@f, Literal["name"]]
```
