# String case operators

[PEP 827] proposes four operators that change the case of a string literal type: `Uppercase[S]`,
`Lowercase[S]`, `Capitalize[S]` and `Uncapitalize[S]`. ty provides them (for now) from
`ty_extensions`. They follow Python's string methods, including full Unicode case mapping, so the
result is the same as what a runtime evaluator of the annotation computes.

```toml
[environment]
python-version = "3.14"
```

## Basic usage

The argument is a string literal type, and so is the result.

```py
from typing import Literal
from ty_extensions import Capitalize, Lowercase, Uncapitalize, Uppercase

def _(
    a: Uppercase[Literal["hello"]],
    b: Lowercase[Literal["HeLLo"]],
    c: Capitalize[Literal["hello"]],
    d: Uncapitalize[Literal["Hello"]],
):
    reveal_type(a)  # revealed: Literal["HELLO"]
    reveal_type(b)  # revealed: Literal["hello"]
    reveal_type(c)  # revealed: Literal["Hello"]
    reveal_type(d)  # revealed: Literal["hello"]
```

## `Capitalize` lowercases the rest of the string

`Capitalize` behaves like `str.capitalize`: the first character is converted to title case and all
remaining characters are converted to lower case. This differs from TypeScript's `Capitalize`, which
only changes the first character.

`Uncapitalize` only lowercases the first character and leaves the rest of the string unchanged, like
TypeScript's `Uncapitalize`.

```py
from typing import Literal
from ty_extensions import Capitalize, Uncapitalize

def _(
    a: Capitalize[Literal["fooBar"]],
    b: Capitalize[Literal["FOO"]],
    c: Capitalize[Literal["user_name"]],
    d: Uncapitalize[Literal["FOO"]],
    e: Uncapitalize[Literal["FooBar"]],
):
    reveal_type(a)  # revealed: Literal["Foobar"]
    reveal_type(b)  # revealed: Literal["Foo"]
    reveal_type(c)  # revealed: Literal["User_name"]
    reveal_type(d)  # revealed: Literal["fOO"]
    reveal_type(e)  # revealed: Literal["fooBar"]
```

## Characters without case

Digits, punctuation and whitespace are left alone. If the first character has no case,
`Capitalize` and `Uncapitalize` leave it unchanged (but `Capitalize` still lowercases the rest).

```py
from typing import Literal
from ty_extensions import Capitalize, Uncapitalize, Uppercase

def _(
    a: Uppercase[Literal["a1_b-c d"]],
    b: Capitalize[Literal["1abc"]],
    c: Capitalize[Literal["_Private"]],
    d: Uncapitalize[Literal["_Private"]],
):
    reveal_type(a)  # revealed: Literal["A1_B-C D"]
    reveal_type(b)  # revealed: Literal["1abc"]
    reveal_type(c)  # revealed: Literal["_private"]
    reveal_type(d)  # revealed: Literal["_Private"]
```

## Empty strings

```py
from typing import Literal
from ty_extensions import Capitalize, Lowercase, Uncapitalize, Uppercase

def _(
    a: Uppercase[Literal[""]],
    b: Lowercase[Literal[""]],
    c: Capitalize[Literal[""]],
    d: Uncapitalize[Literal[""]],
):
    reveal_type(a)  # revealed: Literal[""]
    reveal_type(b)  # revealed: Literal[""]
    reveal_type(c)  # revealed: Literal[""]
    reveal_type(d)  # revealed: Literal[""]
```

## Unicode

Case mapping uses the full Unicode mappings, exactly like Python's `str` methods:

- some characters expand to several characters when uppercased (`ß` becomes `SS`, `ﬁ` becomes
  `FI`);
- a Greek capital sigma at the end of a word lowercases to the final form `ς`;
- `Capitalize` converts the first character to *title case*, which differs from upper case for
  digraphs such as `ǆ` (title case `ǅ`, upper case `Ǆ`) and for ligatures.

```py
from typing import Literal
from ty_extensions import Capitalize, Lowercase, Uncapitalize, Uppercase

def _(
    a: Uppercase[Literal["straße"]],
    b: Uppercase[Literal["ﬁle"]],
    c: Lowercase[Literal["ΟΔΟΣ"]],
    d: Capitalize[Literal["ǆungla"]],
    e: Uppercase[Literal["ǆungla"]],
    f: Capitalize[Literal["ﬁle"]],
    g: Capitalize[Literal["straße"]],
):
    reveal_type(a)  # revealed: Literal["STRASSE"]
    reveal_type(b)  # revealed: Literal["FILE"]
    reveal_type(c)  # revealed: Literal["οδος"]
    reveal_type(d)  # revealed: Literal["ǅungla"]
    reveal_type(e)  # revealed: Literal["ǄUNGLA"]
    reveal_type(f)  # revealed: Literal["File"]
    reveal_type(g)  # revealed: Literal["Straße"]
```

Whether a sigma is word-final depends on the whole string, also for `Capitalize`. `Uncapitalize`
only looks at the first character in isolation, like `s[0:1].lower()`.

```py
from typing import Literal
from ty_extensions import Capitalize, Uncapitalize

def _(
    a: Capitalize[Literal["ΑΣ"]],
    b: Capitalize[Literal["ΣΑΣ"]],
    c: Uncapitalize[Literal["ΣΑΣ"]],
):
    reveal_type(a)  # revealed: Literal["Ας"]
    reveal_type(b)  # revealed: Literal["Σας"]
    reveal_type(c)  # revealed: Literal["σΑΣ"]
```

## Results are ordinary literal types

The result is a plain string literal type. It can be nested inside other types, and it is not
promoted to `str` (just like a `Literal[...]` written directly in an annotation).

```py
from typing import Literal
from ty_extensions import Uppercase

def _(
    a: list[Uppercase[Literal["a"]]],
    b: Uppercase[Literal["a"]] | None,
    c: dict[Uppercase[Literal["key"]], int],
):
    reveal_type(a)  # revealed: list[Literal["A"]]
    reveal_type(b)  # revealed: Literal["A"] | None
    reveal_type(c)  # revealed: dict[Literal["KEY"], int]

def _(x: Uppercase[Literal["a"]]):
    y: Literal["A"] = x
    z: Literal["a"] = x  # error: [invalid-assignment]
    reveal_type([x])  # revealed: list[Literal["A"]]
```

Operators can be nested:

```py
from typing import Literal
from ty_extensions import Capitalize, Lowercase, Uppercase

def _(
    a: Lowercase[Uppercase[Literal["MiXeD"]]],
    b: Capitalize[Lowercase[Literal["HELLO"]]],
):
    reveal_type(a)  # revealed: Literal["mixed"]
    reveal_type(b)  # revealed: Literal["Hello"]
```

## String annotations

The operators work inside string (forward-reference) annotations. Note that the argument still has
to be written as `Literal[...]`.

```py
from typing import Literal
from ty_extensions import Uppercase

def _(a: "Uppercase[Literal['abc']]", b: list["Uppercase[Literal['x']]"]):
    reveal_type(a)  # revealed: Literal["ABC"]
    reveal_type(b)  # revealed: list[Literal["X"]]
```

[PEP 827]: https://peps.python.org/pep-0827/
