//! The small boolean/integer expression language of the plugin's JSON (`ExpressionParser`):
//! material `replacementCondition`s and tile-override `replacements` use it, for example
//! `"h >= 9 && s >= 4 || blendingColors"`.
//!
//! Supported: integer literals, `true`/`false`, identifiers (dots allowed, `SeasonalTheme.WINTER`),
//! `! - ( )`, `* / %`, `+ -`, `< <= > >=`, `== !=`, `&&`, `||`. A JSON array of strings is the `||`
//! of its entries. Upstream: `rs117.hd.utils.ExpressionParser` (BSD 2-Clause, see `THIRD_PARTY.md`).

use serde_json::Value as Json;
use std::{collections::HashMap, fmt};

/// The result of evaluating (part of) an expression.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value {
    Int(i64),
    Bool(bool),
}

impl Value {
    /// Booleans as themselves, integers as "non-zero".
    pub const fn truthy(self) -> bool {
        match self {
            Self::Bool(value) => value,
            Self::Int(value) => value != 0,
        }
    }

    const fn int(self) -> i64 {
        match self {
            Self::Int(value) => value,
            Self::Bool(value) => value as i64,
        }
    }
}

/// Where identifiers get their values.
pub trait Variables {
    fn get(&self, name: &str) -> Option<Value>;
}

impl Variables for HashMap<String, Value> {
    fn get(&self, name: &str) -> Option<Value> {
        HashMap::get(self, name).copied()
    }
}

/// A variable table that knows nothing: every identifier is undefined (and so evaluates to
/// `false`, like an unknown variable in a disabled condition).
pub struct NoVariables;

impl Variables for NoVariables {
    fn get(&self, _name: &str) -> Option<Value> {
        None
    }
}

/// Parse failure with the offending source text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub expression: String,
    pub detail: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "expression `{}`: {}", self.expression, self.detail)
    }
}

impl std::error::Error for ParseError {}

#[derive(Debug, Clone, PartialEq)]
enum Node {
    Literal(Value),
    Variable(String),
    Not(Box<Node>),
    Negate(Box<Node>),
    Binary(Op, Box<Node>, Box<Node>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Or,
    And,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

/// A parsed expression.
#[derive(Debug, Clone, PartialEq)]
pub struct Expression {
    root: Node,
}

impl Expression {
    pub fn parse(source: &str) -> Result<Self, ParseError> {
        let fail = |detail: &str| ParseError {
            expression: source.to_owned(),
            detail: detail.to_owned(),
        };
        let tokens = tokenize(source).map_err(|detail| fail(&detail))?;
        let mut parser = Parser { tokens, pos: 0 };
        let root = parser.or().map_err(|detail| fail(&detail))?;
        if parser.pos != parser.tokens.len() {
            return Err(fail("trailing tokens"));
        }
        Ok(Self { root })
    }

    /// Build the expression a JSON condition denotes: a string, a boolean, or an array of strings
    /// joined with `||` (`ExpressionParser.mergeJsonExpressions`). An empty array is `false`.
    pub fn from_json(value: &Json) -> Result<Self, ParseError> {
        let constant = |value: bool| Self {
            root: Node::Literal(Value::Bool(value)),
        };
        match value {
            Json::Bool(value) => Ok(constant(*value)),
            Json::String(text) => Self::parse(text),
            Json::Array(items) => {
                let mut root: Option<Node> = None;
                for item in items {
                    let Self { root: node } = Self::from_json(item)?;
                    root = Some(match root {
                        None => node,
                        Some(left) => Node::Binary(Op::Or, Box::new(left), Box::new(node)),
                    });
                }
                Ok(root.map_or_else(|| constant(false), |root| Self { root }))
            }
            other => Err(ParseError {
                expression: other.to_string(),
                detail: "expected a string, boolean or array".to_owned(),
            }),
        }
    }

    /// The constant value, when the expression has no variables.
    pub fn constant(&self) -> Option<bool> {
        match self.root {
            Node::Literal(value) => Some(value.truthy()),
            _ => None,
        }
    }

    pub fn eval(&self, vars: &dyn Variables) -> Value {
        eval(&self.root, vars)
    }

    pub fn test(&self, vars: &dyn Variables) -> bool {
        self.eval(vars).truthy()
    }
}

fn eval(node: &Node, vars: &dyn Variables) -> Value {
    match node {
        Node::Literal(value) => *value,
        Node::Variable(name) => vars.get(name).unwrap_or(Value::Bool(false)),
        Node::Not(inner) => Value::Bool(!eval(inner, vars).truthy()),
        Node::Negate(inner) => Value::Int(-eval(inner, vars).int()),
        Node::Binary(Op::And, left, right) => {
            Value::Bool(eval(left, vars).truthy() && eval(right, vars).truthy())
        }
        Node::Binary(Op::Or, left, right) => {
            Value::Bool(eval(left, vars).truthy() || eval(right, vars).truthy())
        }
        Node::Binary(op, left, right) => {
            let (a, b) = (eval(left, vars), eval(right, vars));
            let (x, y) = (a.int(), b.int());
            match op {
                Op::Eq => Value::Bool(match (a, b) {
                    (Value::Bool(a), Value::Bool(b)) => a == b,
                    _ => x == y,
                }),
                Op::Ne => Value::Bool(match (a, b) {
                    (Value::Bool(a), Value::Bool(b)) => a != b,
                    _ => x != y,
                }),
                Op::Lt => Value::Bool(x < y),
                Op::Le => Value::Bool(x <= y),
                Op::Gt => Value::Bool(x > y),
                Op::Ge => Value::Bool(x >= y),
                Op::Add => Value::Int(x.wrapping_add(y)),
                Op::Sub => Value::Int(x.wrapping_sub(y)),
                Op::Mul => Value::Int(x.wrapping_mul(y)),
                Op::Div => Value::Int(if y == 0 { 0 } else { x / y }),
                Op::Rem => Value::Int(if y == 0 { 0 } else { x % y }),
                Op::And | Op::Or => unreachable!("handled above"),
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(i64),
    Ident(String),
    Op(Op),
    Not,
    Open,
    Close,
}

fn tokenize(source: &str) -> Result<Vec<Token>, String> {
    let chars: Vec<char> = source.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        match c {
            c if c.is_whitespace() => i += 1,
            '(' => {
                tokens.push(Token::Open);
                i += 1;
            }
            ')' => {
                tokens.push(Token::Close);
                i += 1;
            }
            '0'..='9' => {
                let start = i;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
                let text: String = chars[start..i].iter().collect();
                tokens.push(Token::Number(text.parse().map_err(|_| "bad number")?));
            }
            c if c.is_alphabetic() || c == '_' => {
                let start = i;
                while i < chars.len()
                    && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '.')
                {
                    i += 1;
                }
                tokens.push(Token::Ident(chars[start..i].iter().collect()));
            }
            '&' if next == Some('&') => {
                tokens.push(Token::Op(Op::And));
                i += 2;
            }
            '|' if next == Some('|') => {
                tokens.push(Token::Op(Op::Or));
                i += 2;
            }
            '=' if next == Some('=') => {
                tokens.push(Token::Op(Op::Eq));
                i += 2;
            }
            '!' if next == Some('=') => {
                tokens.push(Token::Op(Op::Ne));
                i += 2;
            }
            '!' => {
                tokens.push(Token::Not);
                i += 1;
            }
            '<' if next == Some('=') => {
                tokens.push(Token::Op(Op::Le));
                i += 2;
            }
            '>' if next == Some('=') => {
                tokens.push(Token::Op(Op::Ge));
                i += 2;
            }
            '<' => {
                tokens.push(Token::Op(Op::Lt));
                i += 1;
            }
            '>' => {
                tokens.push(Token::Op(Op::Gt));
                i += 1;
            }
            '+' => {
                tokens.push(Token::Op(Op::Add));
                i += 1;
            }
            '-' => {
                tokens.push(Token::Op(Op::Sub));
                i += 1;
            }
            '*' => {
                tokens.push(Token::Op(Op::Mul));
                i += 1;
            }
            '/' => {
                tokens.push(Token::Op(Op::Div));
                i += 1;
            }
            '%' => {
                tokens.push(Token::Op(Op::Rem));
                i += 1;
            }
            other => return Err(format!("unexpected character `{other}`")),
        }
    }
    Ok(tokens)
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn peek_op(&self, ops: &[Op]) -> Option<Op> {
        match self.tokens.get(self.pos) {
            Some(Token::Op(op)) if ops.contains(op) => Some(*op),
            _ => None,
        }
    }

    fn binary(
        &mut self,
        ops: &[Op],
        next: fn(&mut Self) -> Result<Node, String>,
    ) -> Result<Node, String> {
        let mut left = next(self)?;
        while let Some(op) = self.peek_op(ops) {
            self.pos += 1;
            let right = next(self)?;
            left = Node::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn or(&mut self) -> Result<Node, String> {
        self.binary(&[Op::Or], Self::and)
    }

    fn and(&mut self) -> Result<Node, String> {
        self.binary(&[Op::And], Self::equality)
    }

    fn equality(&mut self) -> Result<Node, String> {
        self.binary(&[Op::Eq, Op::Ne], Self::relational)
    }

    fn relational(&mut self) -> Result<Node, String> {
        self.binary(&[Op::Lt, Op::Le, Op::Gt, Op::Ge], Self::additive)
    }

    fn additive(&mut self) -> Result<Node, String> {
        self.binary(&[Op::Add, Op::Sub], Self::multiplicative)
    }

    fn multiplicative(&mut self) -> Result<Node, String> {
        self.binary(&[Op::Mul, Op::Div, Op::Rem], Self::unary)
    }

    fn unary(&mut self) -> Result<Node, String> {
        match self.tokens.get(self.pos) {
            Some(Token::Not) => {
                self.pos += 1;
                Ok(Node::Not(Box::new(self.unary()?)))
            }
            Some(Token::Op(Op::Sub)) => {
                self.pos += 1;
                Ok(Node::Negate(Box::new(self.unary()?)))
            }
            _ => self.primary(),
        }
    }

    fn primary(&mut self) -> Result<Node, String> {
        let token = self.tokens.get(self.pos).cloned().ok_or("unexpected end")?;
        self.pos += 1;
        match token {
            Token::Number(value) => Ok(Node::Literal(Value::Int(value))),
            Token::Ident(name) => Ok(match name.as_str() {
                "true" | "True" => Node::Literal(Value::Bool(true)),
                "false" | "False" => Node::Literal(Value::Bool(false)),
                _ => Node::Variable(name),
            }),
            Token::Open => {
                let inner = self.or()?;
                match self.tokens.get(self.pos) {
                    Some(Token::Close) => {
                        self.pos += 1;
                        Ok(inner)
                    }
                    _ => Err("missing `)`".to_owned()),
                }
            }
            other => Err(format!("unexpected token {other:?}")),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn vars(entries: &[(&str, Value)]) -> HashMap<String, Value> {
        entries
            .iter()
            .map(|(name, value)| ((*name).to_owned(), *value))
            .collect()
    }

    fn check(source: &str, entries: &[(&str, Value)]) -> bool {
        Expression::parse(source).unwrap().test(&vars(entries))
    }

    #[test]
    fn hsl_conditions_from_the_tile_overrides() {
        let tile = [
            ("h", Value::Int(8)),
            ("s", Value::Int(3)),
            ("l", Value::Int(40)),
            ("blendingColors", Value::Bool(true)),
        ];
        assert!(check("h == 8 && s == 3 && l > 30", &tile));
        assert!(!check("h == 8 && s == 3 && l > 50", &tile));
        assert!(check("h < 8 || !blendingColors || s == 3", &tile));
        assert!(check(
            "h == 9 && (s == 3 || s == 4) && l >= 34 || h == 8",
            &tile
        ));
        assert!(!check("!blendingColors", &tile));
    }

    #[test]
    fn arrays_are_or_of_their_entries() {
        let expression =
            Expression::from_json(&serde_json::json!(["h >= 10 && s >= 3", "h == 6"])).unwrap();
        assert!(expression.test(&vars(&[("h", Value::Int(6)), ("s", Value::Int(0))])));
        assert!(expression.test(&vars(&[("h", Value::Int(11)), ("s", Value::Int(3))])));
        assert!(!expression.test(&vars(&[("h", Value::Int(9)), ("s", Value::Int(3))])));
        assert_eq!(
            Expression::from_json(&serde_json::json!(true))
                .unwrap()
                .constant(),
            Some(true)
        );
    }

    #[test]
    fn enum_constants_and_unknown_variables() {
        let winter = [
            ("season", Value::Int(3)),
            ("SeasonalTheme.WINTER", Value::Int(3)),
        ];
        assert!(check("season == SeasonalTheme.WINTER", &winter));
        assert!(!check("season == SeasonalTheme.AUTUMN", &winter));
        // An undefined identifier is false, so a stray variable disables a condition.
        assert!(!check("neverDefined", &[]));
        assert!(Expression::parse("h >=").is_err());
        assert!(Expression::parse("(h").is_err());
    }
}
