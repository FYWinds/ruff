# Lifting over unions that contain aliases

In a type expression, `A | B` keeps type aliases as members of the union. Operators that are lifted
over unions, and `FromUnion`, look through such members: an alias whose value is a union contributes
each of its members.

```toml
[environment]
python-version = "3.14"
```

## Lifted operators

```py
from typing import Literal
from ty_extensions import GetArg, Length

type Lists = list[int] | list[str]
type Short = tuple[int] | tuple[int, str]

def _(
    a: GetArg[Lists | list[bytes], list, Literal[0]],
    b: Length[Short | tuple[()]],
    c: GetArg[Lists, list, Literal[0]],
):
    reveal_type(a)  # revealed: int | str | bytes
    reveal_type(b)  # revealed: Literal[1, 2, 0]
    reveal_type(c)  # revealed: int | str
```

## Phase 2 operators

The same applies to the phase 2 operators that are lifted.

```py
from typing import Literal
from ty_extensions import GetMemberType

class A:
    x: int

class B:
    x: str

type AorB = A | B

def _(a: GetMemberType[AorB | A, Literal["x"]]):
    reveal_type(a)  # revealed: int | str
```

## `FromUnion`

```py
from ty_extensions import FromUnion

type IntOrStr = int | str

def _(a: FromUnion[IntOrStr | bytes], b: FromUnion[IntOrStr]):
    reveal_type(a)  # revealed: tuple[int, str, bytes]
    reveal_type(b)  # revealed: tuple[int, str]
```

[PEP 827]: https://peps.python.org/pep-0827/
