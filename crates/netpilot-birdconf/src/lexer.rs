/// Token types produced by the BIRD2 lexer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Token {
    // Keywords
    Define,
    Table,
    Protocol,
    Template,
    Static,
    Bgp,
    Ospf,
    Rip,
    Direct,
    Kernel,
    RouterId,
    LocalAs,
    Neighbor,
    Interface,
    Import,
    Export,
    Where,
    Accept,
    Reject,
    Yes,
    No,
    On,
    Off,
    True,
    False,
    Prefix,
    Via,
    Blackhole,
    Route,
    Unreachable,
    Prohibit,
    Function,
    Return,
    If,
    Else,
    Case,
    For,
    In,
    Area,
    Stub,
    Password,
    Algorithm,
    GracefulRestart,
    LongLived,

    // Syntax
    Semicolon,
    LBrace,
    RBrace,
    LParen,
    RParen,
    Comma,
    Colon,
    Equals,
    Dot,
    Arrow, // ->

    // Values
    Identifier(String),
    String(String),
    Number(u64),
    IpAddr(String),
    PrefixValue(String),
    AsnValue(u32),

    // Special
    Eof,
    Unknown(char),
}

/// Tokenize a BIRD2 config string into tokens.
pub fn tokenize(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        // Skip whitespace
        if c.is_whitespace() {
            i += 1;
            continue;
        }

        // Skip comments (# to end of line, /* block */)
        if c == '#' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i += 2;
            continue;
        }

        match c {
            ';' => {
                tokens.push(Token::Semicolon);
                i += 1;
            }
            '{' => {
                tokens.push(Token::LBrace);
                i += 1;
            }
            '}' => {
                tokens.push(Token::RBrace);
                i += 1;
            }
            '(' => {
                tokens.push(Token::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            ',' => {
                tokens.push(Token::Comma);
                i += 1;
            }
            ':' => {
                tokens.push(Token::Colon);
                i += 1;
            }
            '=' => {
                tokens.push(Token::Equals);
                i += 1;
            }
            '.' => {
                tokens.push(Token::Dot);
                i += 1;
            }
            '-' if i + 1 < chars.len() && chars[i + 1] == '>' => {
                tokens.push(Token::Arrow);
                i += 2;
            }
            '"' => {
                i += 1; // skip opening quote
                let mut s = String::new();
                while i < chars.len() && chars[i] != '"' {
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        i += 1;
                    }
                    s.push(chars[i]);
                    i += 1;
                }
                i += 1; // skip closing quote
                tokens.push(Token::String(s));
            }
            _ if c.is_alphabetic() || c == '_' => {
                let mut word = String::new();
                while i < chars.len()
                    && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '-')
                {
                    word.push(chars[i]);
                    i += 1;
                }
                tokens.push(match word.as_str() {
                    "define" => Token::Define,
                    "table" => Token::Table,
                    "protocol" => Token::Protocol,
                    "template" => Token::Template,
                    "static" => Token::Static,
                    "bgp" => Token::Bgp,
                    "ospf" => Token::Ospf,
                    "rip" => Token::Rip,
                    "direct" => Token::Direct,
                    "kernel" => Token::Kernel,
                    "router" => Token::RouterId,
                    "id" => Token::Identifier("id".into()),
                    "as" => Token::Identifier("as".into()),
                    "local" => Token::LocalAs,
                    "neighbor" => Token::Neighbor,
                    "interface" => Token::Interface,
                    "import" => Token::Import,
                    "export" => Token::Export,
                    "where" => Token::Where,
                    "accept" => Token::Accept,
                    "reject" => Token::Reject,
                    "yes" | "on" => Token::Yes,
                    "no" | "off" => Token::No,
                    "true" => Token::True,
                    "false" => Token::False,
                    "prefix" => Token::Prefix,
                    "via" => Token::Via,
                    "route" => Token::Route,
                    "blackhole" => Token::Blackhole,
                    "unreachable" => Token::Unreachable,
                    "prohibit" => Token::Prohibit,
                    "function" => Token::Function,
                    "return" => Token::Return,
                    "if" => Token::If,
                    "else" => Token::Else,
                    "case" => Token::Case,
                    "for" => Token::For,
                    "in" => Token::In,
                    "area" => Token::Area,
                    "stub" => Token::Stub,
                    "password" => Token::Password,
                    "algorithm" => Token::Algorithm,
                    "graceful" => Token::GracefulRestart,
                    "restart" => Token::Identifier("restart".into()),
                    "long" => Token::Identifier("long".into()),
                    "lived" => Token::LongLived,
                    _ => Token::Identifier(word),
                });
            }
            _ if c.is_ascii_digit() => {
                let mut num = String::new();
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    num.push(chars[i]);
                    i += 1;
                }
                if num.contains('.') {
                    tokens.push(Token::IpAddr(num));
                } else {
                    tokens.push(Token::Number(num.parse().unwrap_or(0)));
                }
            }
            _ => {
                tokens.push(Token::Unknown(c));
                i += 1;
            }
        }
    }
    tokens.push(Token::Eof);
    tokens
}
