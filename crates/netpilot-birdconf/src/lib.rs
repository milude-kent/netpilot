pub mod lexer;
pub mod parser;

pub use lexer::{Token, tokenize};
pub use parser::{ParseError, Parser, parse_bird_config};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_bird_config() {
        let input = r#"
            router id 192.0.2.1;

            protocol static {
                route 10.0.0.0/8 via 192.0.2.254;
            }
        "#;
        let config = parse_bird_config(input).expect("should parse");
        assert_eq!(config.identity.router_id, "192.0.2.1");
        assert_eq!(config.protocols.len(), 1);
    }

    #[test]
    fn parse_bgp_neighbor() {
        let input = r#"
            protocol bgp {
                local as 65001;
                router id 192.0.2.1;
                neighbor 192.0.2.2 as 65002;
            }
        "#;
        let config = parse_bird_config(input).expect("should parse");
        assert_eq!(config.protocols.len(), 1);
    }

    #[test]
    fn lexer_tokenizes_keywords() {
        let tokens = tokenize("protocol bgp { }");
        assert!(tokens.iter().any(|t| matches!(t, Token::Protocol)));
        assert!(tokens.iter().any(|t| matches!(t, Token::Bgp)));
    }

    #[test]
    fn lexer_handles_comments() {
        let tokens = tokenize("# this is a comment\nprotocol static {}");
        assert!(tokens.iter().any(|t| matches!(t, Token::Static)));
        assert!(
            !tokens
                .iter()
                .any(|t| matches!(t, Token::Identifier(s) if s == "this"))
        );
    }

    #[test]
    fn parse_ospf_protocol() {
        let input = r#"
            router id 10.0.0.1;

            protocol ospf {
                area 0.0.0.0 {
                    stub;
                };
            }
        "#;
        let config = parse_bird_config(input).expect("should parse");
        assert_eq!(config.protocols.len(), 1);
    }

    #[test]
    fn lexer_simple_tokens() {
        let tokens = tokenize("router id 192.0.2.1;");
        assert!(tokens.iter().any(|t| matches!(t, Token::RouterId)));
        assert!(
            tokens
                .iter()
                .any(|t| matches!(t, Token::IpAddr(a) if a == "192.0.2.1"))
        );
        assert_eq!(tokens.last(), Some(&Token::Eof));
    }
}
