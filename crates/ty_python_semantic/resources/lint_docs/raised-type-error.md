## What it does

Reports type-level errors raised with the experimental [PEP 827] `RaiseError[S, *Ts]` operator.

## Why is this bad?

`RaiseError` represents an explicit request from the author of a type to report an error when the
type is evaluated with arguments it does not support. The error is reported where the type is
used: in an annotation or another type expression, or at a call whose specialized return type
raises the error. A `RaiseError` in a branch of a conditional type that is not taken, or in the
definition of a generic type alias, is not reported.

The type that raised the error evaluates to `Any`.

## Examples

```toml
[environment]
python-version = "3.12"
```

```python
from typing import Literal
from ty_extensions import IsAssignable, RaiseError

type OnlyInts[T] = T if IsAssignable[T, int] else RaiseError[Literal["expected an int"], T]

def f(x: OnlyInts[str]): ...  # error: "expected an int: str"
```

[PEP 827]: https://peps.python.org/pep-0827/
