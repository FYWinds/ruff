//! The string transformations behind the PEP 827 string operators (`Uppercase`, `Lowercase`,
//! `Capitalize`, `Uncapitalize`, `Concat` and the string form of `Slice`), following Python's
//! `str` semantics with full Unicode case mappings, and the classification of their operands.

mod titlecase;

use titlecase::TITLECASE_EXCEPTIONS;

use super::{KnownClass, Type};
use crate::Db;
use crate::ProgramEnvironment;

/// The category of a (non-union) operand of a string operator.
#[derive(Copy, Clone, Debug)]
pub(super) enum StringOperand<'db> {
    /// A string literal type with a known value.
    Literal(&'db str),
    /// `LiteralString`.
    LiteralString,
    /// A dynamic type (`Any`, `Unknown`, a raised error, ...), which is the result.
    Dynamic(Type<'db>),
    /// Any other type assignable to `str` (`str`, its subclasses, `StrEnum` members, ...). The
    /// result widens to `str`.
    Str,
    /// A type that is not assignable to `str`.
    Invalid,
}

impl<'db> StringOperand<'db> {
    /// Classifies `ty`. Since the categories are ordered by assignability to `str`, string
    /// operators are monotonic in their operands.
    pub(super) fn classify(db: &'db dyn Db, env: &ProgramEnvironment<'db>, ty: Type<'db>) -> Self {
        if ty.is_dynamic() {
            return Self::Dynamic(ty);
        }
        if let Type::LiteralValue(literal) = ty {
            if let Some(string) = literal.as_string() {
                return Self::Literal(string.value(db));
            }
            if literal.is_literal_string() {
                return Self::LiteralString;
            }
        }
        if ty.is_assignable_to(db, env, KnownClass::Str.to_instance(db, env)) {
            Self::Str
        } else {
            Self::Invalid
        }
    }
}

/// The case transformation applied by a string case operator.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, get_size2::GetSize, salsa::SalsaValue)]
pub enum StringCaseOp {
    /// `Uppercase[S]`: `str.upper`.
    Upper,
    /// `Lowercase[S]`: `str.lower`.
    Lower,
    /// `Capitalize[S]`: `str.capitalize` (the first character is titlecased, the rest is
    /// lowercased).
    Capitalize,
    /// `Uncapitalize[S]`: `s[0:1].lower() + s[1:]` (only the first character changes).
    Uncapitalize,
}

impl StringCaseOp {
    /// Applies this transformation to `value`.
    pub(crate) fn apply(self, value: &str) -> String {
        match self {
            Self::Upper => value.to_uppercase(),
            Self::Lower => value.to_lowercase(),
            Self::Capitalize => capitalize(value),
            Self::Uncapitalize => uncapitalize(value),
        }
    }

    /// The name of the special form for this operation.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Upper => "Uppercase",
            Self::Lower => "Lowercase",
            Self::Capitalize => "Capitalize",
            Self::Uncapitalize => "Uncapitalize",
        }
    }
}

/// Python's `str.capitalize`.
///
/// The whole string is lowercased first, so that the final-sigma rule sees the context of the
/// entire string (`"ΑΣ"` becomes `"Ας"`); the lowercased first character is then replaced by its
/// titlecase form. The first character can never be a final sigma (no cased letter precedes it),
/// so the lowercased prefix is exactly `first.to_lowercase()`.
fn capitalize(value: &str) -> String {
    let Some(first) = value.chars().next() else {
        return String::new();
    };
    let lower = value.to_lowercase();
    let prefix_len: usize = first.to_lowercase().map(char::len_utf8).sum();
    let mut result = String::with_capacity(lower.len() + 2);
    push_titlecase(&mut result, first);
    result.push_str(&lower[prefix_len..]);
    result
}

/// `s[0:1].lower() + s[1:]`: lowercases the first character only.
fn uncapitalize(value: &str) -> String {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut result = String::with_capacity(value.len() + 2);
    result.extend(first.to_lowercase());
    result.push_str(chars.as_str());
    result
}

/// Appends the (full) titlecase mapping of `c` to `out`.
fn push_titlecase(out: &mut String, c: char) {
    match TITLECASE_EXCEPTIONS.binary_search_by_key(&c, |(c, _)| *c) {
        Ok(index) => out.push_str(TITLECASE_EXCEPTIONS[index].1),
        Err(_) => out.extend(c.to_uppercase()),
    }
}

#[cfg(test)]
mod tests {
    use super::StringCaseOp::{Capitalize, Lower, Uncapitalize, Upper};
    use super::TITLECASE_EXCEPTIONS;

    #[test]
    fn titlecase_table_is_sorted() {
        assert!(TITLECASE_EXCEPTIONS.windows(2).all(|w| w[0].0 < w[1].0));
        assert_eq!(TITLECASE_EXCEPTIONS.len(), 135);
    }

    #[test]
    fn eszett() {
        assert_eq!(Upper.apply("straße"), "STRASSE");
        assert_eq!(Lower.apply("straße"), "straße");
        assert_eq!(Capitalize.apply("straße"), "Straße");
        assert_eq!(Uncapitalize.apply("straße"), "straße");
        assert_eq!(Capitalize.apply("ß"), "Ss");
    }

    #[test]
    fn final_sigma() {
        assert_eq!(Upper.apply("ΟΔΟΣ"), "ΟΔΟΣ");
        assert_eq!(Lower.apply("ΟΔΟΣ"), "οδος");
        assert_eq!(Capitalize.apply("ΟΔΟΣ"), "Οδος");
        assert_eq!(Uncapitalize.apply("ΟΔΟΣ"), "οΔΟΣ");
        assert_eq!(Lower.apply("ΑΣ"), "ας");
        assert_eq!(Capitalize.apply("ΑΣ"), "Ας");
        assert_eq!(Uncapitalize.apply("Σ"), "σ");
        assert_eq!(Lower.apply("Σ"), "σ");
    }

    #[test]
    fn titlecase_digraphs_and_ligatures() {
        assert_eq!(Upper.apply("ǆungla"), "ǄUNGLA");
        assert_eq!(Capitalize.apply("ǆungla"), "ǅungla");
        assert_eq!(Capitalize.apply("ǄUNGLA"), "ǅungla");
        assert_eq!(Upper.apply("ﬁle"), "FILE");
        assert_eq!(Capitalize.apply("ﬁle"), "File");
    }

    #[test]
    fn ascii() {
        assert_eq!(Upper.apply("fooBar"), "FOOBAR");
        assert_eq!(Lower.apply("fooBar"), "foobar");
        assert_eq!(Capitalize.apply("fooBar"), "Foobar");
        assert_eq!(Uncapitalize.apply("fooBar"), "fooBar");
        assert_eq!(Uncapitalize.apply("FOO"), "fOO");
        assert_eq!(Upper.apply("1abc"), "1ABC");
        assert_eq!(Capitalize.apply("1abc"), "1abc");
        assert_eq!(Uncapitalize.apply("1abc"), "1abc");
    }

    #[test]
    fn empty() {
        for op in [Upper, Lower, Capitalize, Uncapitalize] {
            assert_eq!(op.apply(""), "");
        }
    }
}
