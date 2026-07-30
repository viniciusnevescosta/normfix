//! Conservative semantic classification for C facts.
//!
//! This layer deliberately answers only questions it can prove from one
//! translation unit. In particular, it resolves integer enum constants so an
//! enum-sized array is not confused with a variable-length array. Unsupported
//! expressions remain [`ArrayBoundKind::Unknown`] instead of being guessed.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use normfix_c_syntax::{ArrayDeclaratorFact, ParsedFile, SyntaxFacts};
use normfix_core::TextRange;

/// Semantic facts derived without exposing the parser backend.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SemanticFacts {
    /// Proven integer values for enumerator identifiers.
    pub enum_values: BTreeMap<String, i128>,
    /// Classified array declarators in source order.
    pub arrays: Vec<ClassifiedArray>,
}

/// One classified array declarator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassifiedArray {
    /// Recovered declarator identifier.
    pub name: Option<String>,
    /// Complete array declarator range.
    pub range: TextRange,
    /// Bound expression range.
    pub bound_range: Option<TextRange>,
    /// Original bound expression.
    pub expression: Option<String>,
    /// Proven classification.
    pub kind: ArrayBoundKind,
}

/// Semantic classification of an array bound.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArrayBoundKind {
    /// An integer constant expression with its proven value.
    Constant(i128),
    /// An expression containing an ordinary identifier that is not a known
    /// enum constant in this translation unit.
    Variable,
    /// An incomplete `[]` declarator.
    Incomplete,
    /// The expression used syntax outside the deliberately small evaluator.
    Unknown(String),
}

/// Builds conservative semantic facts from one clean parse.
#[must_use]
pub fn analyze(parsed: &ParsedFile) -> SemanticFacts {
    analyze_facts(parsed.facts())
}

/// Builds conservative semantic facts from backend-neutral syntax facts.
#[must_use]
pub fn analyze_facts(facts: &SyntaxFacts) -> SemanticFacts {
    let enum_values = resolve_enum_values(facts);
    let arrays = facts
        .arrays
        .iter()
        .map(|array| classify_array(array, &enum_values))
        .collect();
    SemanticFacts {
        enum_values,
        arrays,
    }
}

fn resolve_enum_values(facts: &SyntaxFacts) -> BTreeMap<String, i128> {
    let mut values = BTreeMap::new();
    let mut active_enum = None;
    let mut previous = None;
    for item in &facts.enum_constants {
        if active_enum != Some(item.enum_range) {
            active_enum = Some(item.enum_range);
            previous = None;
        }
        let value = match item.explicit_value.as_deref() {
            Some(expression) => evaluate_integer_constant(expression, &values).ok(),
            None => previous
                .and_then(|value: i128| value.checked_add(1))
                .or(Some(0)),
        };
        if let Some(value) = value {
            values.insert(item.name.clone(), value);
        }
        previous = value;
    }
    values
}

fn classify_array(
    array: &ArrayDeclaratorFact,
    constants: &BTreeMap<String, i128>,
) -> ClassifiedArray {
    let kind = match array.bound.as_deref() {
        None => ArrayBoundKind::Incomplete,
        Some(expression) => match evaluate_integer_constant(expression, constants) {
            Ok(value) => ArrayBoundKind::Constant(value),
            Err(EvalError::OrdinaryIdentifier) => ArrayBoundKind::Variable,
            Err(error) => ArrayBoundKind::Unknown(error.explanation().to_owned()),
        },
    };
    ClassifiedArray {
        name: array.name.clone(),
        range: array.range,
        bound_range: array.bound_range,
        expression: array.bound.clone(),
        kind,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EvalError {
    InvalidToken,
    UnsupportedOperator,
    OrdinaryIdentifier,
    Arithmetic,
    TrailingInput,
}

impl EvalError {
    const fn explanation(self) -> &'static str {
        match self {
            Self::InvalidToken => "the bound contains an unsupported token",
            Self::UnsupportedOperator => "the bound uses an unsupported constant operator",
            Self::OrdinaryIdentifier => "the bound depends on a non-enum identifier",
            Self::Arithmetic => "constant evaluation overflowed or divided by zero",
            Self::TrailingInput => "the bound was not a complete constant expression",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Token {
    Number(i128),
    Identifier(String),
    Operator(&'static str),
    LeftParen,
    RightParen,
}

fn evaluate_integer_constant(
    expression: &str,
    constants: &BTreeMap<String, i128>,
) -> Result<i128, EvalError> {
    let tokens = tokenize(expression)?;
    let mut parser = ExpressionParser {
        tokens: &tokens,
        index: 0,
        constants,
    };
    let value = parser.parse_expression(0)?;
    if parser.index == tokens.len() {
        Ok(value)
    } else {
        Err(EvalError::TrailingInput)
    }
}

fn tokenize(expression: &str) -> Result<Vec<Token>, EvalError> {
    let bytes = expression.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index].is_ascii_whitespace() {
            index += 1;
            continue;
        }
        if bytes[index].is_ascii_digit() {
            let start = index;
            index += 1;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
            {
                index += 1;
            }
            let text = expression
                .get(start..index)
                .ok_or(EvalError::InvalidToken)?;
            tokens.push(Token::Number(parse_integer_literal(text)?));
            continue;
        }
        if bytes[index] == b'_' || bytes[index].is_ascii_alphabetic() {
            let start = index;
            index += 1;
            while index < bytes.len()
                && (bytes[index] == b'_' || bytes[index].is_ascii_alphanumeric())
            {
                index += 1;
            }
            tokens.push(Token::Identifier(
                expression
                    .get(start..index)
                    .ok_or(EvalError::InvalidToken)?
                    .to_owned(),
            ));
            continue;
        }
        let two = expression.get(index..index.saturating_add(2));
        if matches!(two, Some("<<" | ">>" | "&&" | "||")) {
            let operator = match two {
                Some("<<") => "<<",
                Some(">>") => ">>",
                Some("&&") => "&&",
                Some("||") => "||",
                _ => unreachable!("matched two-character operator"),
            };
            tokens.push(Token::Operator(operator));
            index += 2;
            continue;
        }
        match bytes[index] {
            b'(' => tokens.push(Token::LeftParen),
            b')' => tokens.push(Token::RightParen),
            b'+' => tokens.push(Token::Operator("+")),
            b'-' => tokens.push(Token::Operator("-")),
            b'*' => tokens.push(Token::Operator("*")),
            b'/' => tokens.push(Token::Operator("/")),
            b'%' => tokens.push(Token::Operator("%")),
            b'&' => tokens.push(Token::Operator("&")),
            b'|' => tokens.push(Token::Operator("|")),
            b'^' => tokens.push(Token::Operator("^")),
            b'~' => tokens.push(Token::Operator("~")),
            b'!' => tokens.push(Token::Operator("!")),
            _ => return Err(EvalError::InvalidToken),
        }
        index += 1;
    }
    Ok(tokens)
}

fn parse_integer_literal(text: &str) -> Result<i128, EvalError> {
    let digits = text.trim_end_matches(['u', 'U', 'l', 'L']);
    let (radix, digits) = if let Some(value) = digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0X"))
    {
        (16, value)
    } else if let Some(value) = digits
        .strip_prefix("0b")
        .or_else(|| digits.strip_prefix("0B"))
    {
        (2, value)
    } else if digits.len() > 1 && digits.starts_with('0') {
        (8, &digits[1..])
    } else {
        (10, digits)
    };
    if digits.is_empty() {
        return Ok(0);
    }
    i128::from_str_radix(digits, radix).map_err(|_| EvalError::InvalidToken)
}

struct ExpressionParser<'a> {
    tokens: &'a [Token],
    index: usize,
    constants: &'a BTreeMap<String, i128>,
}

impl ExpressionParser<'_> {
    fn parse_expression(&mut self, minimum_binding: u8) -> Result<i128, EvalError> {
        let mut left = self.parse_prefix()?;
        while let Some(Token::Operator(operator)) = self.tokens.get(self.index) {
            let Some((left_binding, right_binding)) = infix_binding(operator) else {
                break;
            };
            if left_binding < minimum_binding {
                break;
            }
            self.index += 1;
            let right = self.parse_expression(right_binding)?;
            left = apply_infix(operator, left, right)?;
        }
        Ok(left)
    }

    fn parse_prefix(&mut self) -> Result<i128, EvalError> {
        let token = self
            .tokens
            .get(self.index)
            .ok_or(EvalError::InvalidToken)?
            .clone();
        self.index += 1;
        match token {
            Token::Number(value) => Ok(value),
            Token::Identifier(name) if matches!(name.as_str(), "sizeof" | "_Alignof") => {
                Err(EvalError::UnsupportedOperator)
            }
            Token::Identifier(name) => self
                .constants
                .get(&name)
                .copied()
                .ok_or(EvalError::OrdinaryIdentifier),
            Token::LeftParen => {
                let value = self.parse_expression(0)?;
                if self.tokens.get(self.index) != Some(&Token::RightParen) {
                    return Err(EvalError::InvalidToken);
                }
                self.index += 1;
                Ok(value)
            }
            Token::Operator("+") => self.parse_expression(13),
            Token::Operator("-") => self
                .parse_expression(13)?
                .checked_neg()
                .ok_or(EvalError::Arithmetic),
            Token::Operator("~") => Ok(!self.parse_expression(13)?),
            Token::Operator("!") => Ok(i128::from(self.parse_expression(13)? == 0)),
            Token::Operator(_) => Err(EvalError::UnsupportedOperator),
            Token::RightParen => Err(EvalError::InvalidToken),
        }
    }
}

const fn infix_binding(operator: &str) -> Option<(u8, u8)> {
    match operator.as_bytes() {
        b"||" => Some((1, 2)),
        b"&&" => Some((3, 4)),
        b"|" => Some((5, 6)),
        b"^" => Some((7, 8)),
        b"&" => Some((9, 10)),
        b"<<" | b">>" => Some((11, 12)),
        b"+" | b"-" => Some((13, 14)),
        b"*" | b"/" | b"%" => Some((15, 16)),
        _ => None,
    }
}

fn apply_infix(operator: &str, left: i128, right: i128) -> Result<i128, EvalError> {
    match operator {
        "||" => Ok(i128::from(left != 0 || right != 0)),
        "&&" => Ok(i128::from(left != 0 && right != 0)),
        "|" => Ok(left | right),
        "^" => Ok(left ^ right),
        "&" => Ok(left & right),
        "<<" => u32::try_from(right)
            .ok()
            .and_then(|amount| left.checked_shl(amount))
            .ok_or(EvalError::Arithmetic),
        ">>" => u32::try_from(right)
            .ok()
            .and_then(|amount| left.checked_shr(amount))
            .ok_or(EvalError::Arithmetic),
        "+" => left.checked_add(right).ok_or(EvalError::Arithmetic),
        "-" => left.checked_sub(right).ok_or(EvalError::Arithmetic),
        "*" => left.checked_mul(right).ok_or(EvalError::Arithmetic),
        "/" => left.checked_div(right).ok_or(EvalError::Arithmetic),
        "%" => left.checked_rem(right).ok_or(EvalError::Arithmetic),
        _ => Err(EvalError::UnsupportedOperator),
    }
}

#[cfg(test)]
mod tests {
    use normfix_c_syntax::CParser;

    use super::{ArrayBoundKind, analyze};

    #[test]
    fn resolves_implicit_enum_array_bound_without_calling_it_a_vla() {
        let mut parser = CParser::new().expect("parser");
        let parsed = parser
            .parse(concat!(
                "typedef enum e_op {\n",
                "\top_sa, op_sb, op_ss, op_pa, op_pb, op_ra,\n",
                "\top_rb, op_rr, op_rra, op_rrb, op_rrr, op_total\n",
                "} t_op;\n",
                "typedef struct s_context { int count[op_total]; } t_context;\n",
            ))
            .expect("parse");
        let semantic = analyze(&parsed);

        assert_eq!(semantic.enum_values["op_total"], 11);
        assert_eq!(semantic.arrays.len(), 1);
        assert_eq!(semantic.arrays[0].kind, ArrayBoundKind::Constant(11));
    }

    #[test]
    fn evaluates_integer_enum_expressions_and_keeps_unknowns_conservative() {
        let mut parser = CParser::new().expect("parser");
        let parsed = parser
            .parse(concat!(
                "enum e_size { base = 0x4U, shifted = base << 2, total };\n",
                "void f(int n) { int fixed[total]; int variable[n]; int odd[sizeof(int)]; }\n",
            ))
            .expect("parse");
        let semantic = analyze(&parsed);

        assert_eq!(semantic.enum_values["shifted"], 16);
        assert_eq!(semantic.enum_values["total"], 17);
        assert_eq!(semantic.arrays[0].kind, ArrayBoundKind::Constant(17));
        assert_eq!(semantic.arrays[1].kind, ArrayBoundKind::Variable);
        assert!(matches!(
            semantic.arrays[2].kind,
            ArrayBoundKind::Unknown(_)
        ));
    }

    #[test]
    fn separate_enums_restart_implicit_values() {
        let mut parser = CParser::new().expect("parser");
        let parsed = parser
            .parse("enum first { a = 8, b }; enum second { c, d };\n")
            .expect("parse");
        let semantic = analyze(&parsed);

        assert_eq!(semantic.enum_values["b"], 9);
        assert_eq!(semantic.enum_values["c"], 0);
        assert_eq!(semantic.enum_values["d"], 1);
    }
}
