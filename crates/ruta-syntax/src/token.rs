//! Tokens and source spans.

/// A byte range in the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Token<'a> {
    pub kind: TokenKind<'a>,
    pub span: Span,
}

/// `global` is deliberately absent. `LUA_COMPAT_GLOBAL` is on, which makes it an ordinary name,
/// and the parser separates the two readings with one token of lookahead.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TokenKind<'a> {
    Name(&'a [u8]),
    Int(i64),
    Float(f64),
    Str(Box<[u8]>),

    And,
    Break,
    Do,
    Else,
    Elseif,
    End,
    False,
    For,
    Function,
    Goto,
    If,
    In,
    Local,
    Nil,
    Not,
    Or,
    Repeat,
    Return,
    Then,
    True,
    Until,
    While,

    IDiv,
    Concat,
    Dots,
    Eq,
    Ge,
    Le,
    Ne,
    Shl,
    Shr,
    DbColon,

    /// Every symbol Lua treats as a token of its own bytes: `+`, `{`, `#`, and the rest.
    Byte(u8),
    Eof,
}

impl TokenKind<'_> {
    /// How a token is named inside an error message.
    pub(crate) fn describe(&self) -> String {
        match self {
            TokenKind::Name(_)
            | TokenKind::Int(_)
            | TokenKind::Float(_)
            | TokenKind::Str(_)
            | TokenKind::Eof => self.spelling().to_owned(),
            written => format!("'{}'", String::from_utf8_lossy(&written.symbol())),
        }
    }

    /// The characters a token is written with, which is what a `near` clause quotes.
    pub(crate) fn symbol(&self) -> Vec<u8> {
        match self {
            TokenKind::Byte(byte) if !(0x20..=0x7e).contains(byte) => {
                format!("<\\{byte}>").into_bytes()
            }
            TokenKind::Byte(byte) => vec![*byte],
            named => named.spelling().as_bytes().to_vec(),
        }
    }

    fn spelling(&self) -> &'static str {
        match self {
            TokenKind::Name(_) => "<name>",
            TokenKind::Int(_) => "<integer>",
            TokenKind::Float(_) => "<number>",
            TokenKind::Str(_) => "<string>",
            TokenKind::Eof => "<eof>",
            TokenKind::And => "and",
            TokenKind::Break => "break",
            TokenKind::Do => "do",
            TokenKind::Else => "else",
            TokenKind::Elseif => "elseif",
            TokenKind::End => "end",
            TokenKind::False => "false",
            TokenKind::For => "for",
            TokenKind::Function => "function",
            TokenKind::Goto => "goto",
            TokenKind::If => "if",
            TokenKind::In => "in",
            TokenKind::Local => "local",
            TokenKind::Nil => "nil",
            TokenKind::Not => "not",
            TokenKind::Or => "or",
            TokenKind::Repeat => "repeat",
            TokenKind::Return => "return",
            TokenKind::Then => "then",
            TokenKind::True => "true",
            TokenKind::Until => "until",
            TokenKind::While => "while",
            TokenKind::IDiv => "//",
            TokenKind::Concat => "..",
            TokenKind::Dots => "...",
            TokenKind::Eq => "==",
            TokenKind::Ge => ">=",
            TokenKind::Le => "<=",
            TokenKind::Ne => "~=",
            TokenKind::Shl => "<<",
            TokenKind::Shr => ">>",
            TokenKind::DbColon => "::",
            TokenKind::Byte(_) => "",
        }
    }
}

pub(crate) fn keyword(word: &[u8]) -> Option<TokenKind<'static>> {
    Some(match word {
        b"and" => TokenKind::And,
        b"break" => TokenKind::Break,
        b"do" => TokenKind::Do,
        b"else" => TokenKind::Else,
        b"elseif" => TokenKind::Elseif,
        b"end" => TokenKind::End,
        b"false" => TokenKind::False,
        b"for" => TokenKind::For,
        b"function" => TokenKind::Function,
        b"goto" => TokenKind::Goto,
        b"if" => TokenKind::If,
        b"in" => TokenKind::In,
        b"local" => TokenKind::Local,
        b"nil" => TokenKind::Nil,
        b"not" => TokenKind::Not,
        b"or" => TokenKind::Or,
        b"repeat" => TokenKind::Repeat,
        b"return" => TokenKind::Return,
        b"then" => TokenKind::Then,
        b"true" => TokenKind::True,
        b"until" => TokenKind::Until,
        b"while" => TokenKind::While,
        _ => return None,
    })
}
