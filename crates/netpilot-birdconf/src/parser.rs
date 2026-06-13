use crate::lexer::Token;
use netpilot_config::*;

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("unexpected token at position {pos}: expected {expected}, got {got:?}")]
    UnexpectedToken {
        pos: usize,
        expected: String,
        got: Token,
    },
    #[error("parse error: {0}")]
    Message(String),
}

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn advance(&mut self) -> &Token {
        let t = &self.tokens[self.pos];
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        t
    }

    fn expect_ident(&mut self) -> Result<String, ParseError> {
        let pos = self.pos;
        match self.advance() {
            Token::Identifier(s) => Ok(s.clone()),
            Token::String(s) => Ok(s.clone()),
            Token::IpAddr(s) => Ok(s.clone()),
            Token::Number(n) => Ok(n.to_string()),
            t => Err(ParseError::UnexpectedToken {
                pos,
                expected: "identifier".into(),
                got: t.clone(),
            }),
        }
    }

    fn expect_number(&mut self) -> Result<u64, ParseError> {
        let pos = self.pos;
        match self.advance() {
            Token::Number(n) => Ok(*n),
            t => Err(ParseError::UnexpectedToken {
                pos,
                expected: "number".into(),
                got: t.clone(),
            }),
        }
    }

    fn expect_string(&mut self) -> Result<String, ParseError> {
        let pos = self.pos;
        match self.advance() {
            Token::String(s) => Ok(s.clone()),
            Token::Identifier(s) => Ok(s.clone()),
            t => Err(ParseError::UnexpectedToken {
                pos,
                expected: "string".into(),
                got: t.clone(),
            }),
        }
    }

    fn skip_semicolons(&mut self) {
        while matches!(self.peek(), Token::Semicolon) {
            self.advance();
        }
    }

    /// Parse a complete RoutePlaneConfig from tokens.
    pub fn parse_config(&mut self) -> Result<RoutePlaneConfig, ParseError> {
        let mut config = RoutePlaneConfig::default();
        let mut defines = Vec::new();
        let mut protocols = Vec::new();

        while !matches!(self.peek(), Token::Eof) {
            self.skip_semicolons();
            match self.peek() {
                Token::Define => {
                    self.advance(); // skip 'define'
                    let name = self.expect_ident()?;
                    if matches!(self.peek(), Token::Equals) {
                        self.advance();
                    }
                    let value = self.parse_value()?;
                    defines.push(ConstantDef { name, value });
                }
                Token::Table => {
                    self.advance();
                    let name = self.expect_ident()?;
                    // skip table body for now
                    if matches!(self.peek(), Token::LBrace) {
                        self.advance();
                        self.skip_block()?;
                    }
                    if name != "master" {
                        config.tables.push(TableConfig {
                            name: name.clone(),
                            nettype: None,
                            kernel_table: None,
                            gc_threshold: None,
                            gc_period_secs: None,
                            sorted: None,
                            trie: None,
                            min_settle_time_secs: None,
                            max_settle_time_secs: None,
                        });
                    }
                }
                Token::Protocol => {
                    self.advance();
                    if let Some(proto) = self.parse_protocol_block()? {
                        protocols.push(proto);
                    }
                }
                Token::RouterId => {
                    self.advance(); // skip 'router'
                    // skip 'id' keyword
                    if matches!(self.peek(), Token::Identifier(s) if s == "id") {
                        self.advance();
                    }
                    config.identity.router_id = self.expect_ident()?;
                }
                Token::Identifier(_) => {
                    // Try key=value global options
                    let key = self.expect_ident()?;
                    if matches!(self.peek(), Token::Equals) {
                        self.advance();
                        let _val = self.parse_value()?;
                        // Store global option — skipped for now
                    } else {
                        return Err(ParseError::Message(format!("unexpected identifier: {key}")));
                    }
                }
                _ => {
                    self.advance();
                }
            }
        }

        config.defines = if defines.is_empty() {
            None
        } else {
            Some(defines)
        };
        config.protocols = protocols;
        Ok(config)
    }

    fn parse_protocol_block(&mut self) -> Result<Option<ProtocolConfig>, ParseError> {
        match self.peek() {
            Token::Static => {
                self.advance();
                self.parse_static_protocol()
            }
            Token::Bgp => {
                self.advance();
                self.parse_bgp_protocol()
            }
            Token::Ospf => {
                self.advance();
                self.parse_ospf_protocol()
            }
            _ => {
                let proto_type = self.expect_ident()?;
                match proto_type.as_str() {
                    "static" | "bgp" | "ospf" => {
                        // Handled above; should not reach here for keywords
                        self.skip_block()?;
                        Ok(None)
                    }
                    _ => {
                        self.skip_block()?;
                        Ok(None)
                    }
                }
            }
        }
    }

    fn parse_static_protocol(&mut self) -> Result<Option<ProtocolConfig>, ParseError> {
        let mut name = String::new();
        let mut table = "master".to_string();
        let mut routes = Vec::new();

        if matches!(self.peek(), Token::LBrace) {
            self.advance();
            while !matches!(self.peek(), Token::RBrace | Token::Eof) {
                match self.peek() {
                    Token::Identifier(key) => {
                        let key_str = key.clone();
                        self.advance();
                        if matches!(self.peek(), Token::Equals) {
                            self.advance();
                        }
                        match key_str.as_str() {
                            "name" => {
                                name = self.expect_string()?;
                            }
                            "table" => {
                                table = self.expect_string()?;
                            }
                            _ => {
                                let _ = self.parse_value()?;
                            }
                        }
                    }
                    Token::Route | Token::Prefix => {
                        if matches!(self.peek(), Token::Route) {
                            self.advance();
                        }
                        let prefix = self.expect_ident()?;
                        let mut next_hop = None;
                        let mut blackhole = false;
                        if matches!(self.peek(), Token::Via) {
                            self.advance();
                            next_hop = Some(self.expect_ident()?);
                        }
                        if matches!(self.peek(), Token::Blackhole) {
                            self.advance();
                            blackhole = true;
                        }
                        routes.push(StaticRoute {
                            prefix,
                            next_hop,
                            blackhole,
                            address_family: AddressFamily::Ipv4,
                            nexthop_type: None,
                            mpls_label: None,
                            igp_metric: None,
                        });
                    }
                    _ => {
                        self.advance();
                    }
                }
                self.skip_semicolons();
            }
            self.advance(); // skip }
        }

        Ok(Some(ProtocolConfig::Static {
            name,
            table,
            routes,
            limits: None,
            import_keep_filtered: None,
            rpki_reload: None,
            passwords: None,
            password: None,
            tx_class: None,
            tx_priority: None,
            description: None,
            mpls_channel: None,
        }))
    }

    fn parse_bgp_protocol(&mut self) -> Result<Option<ProtocolConfig>, ParseError> {
        let mut name = String::new();
        let mut local_asn = 0u32;
        let mut _router_id = String::new();
        let mut neighbors = Vec::new();
        let table = "master".to_string();

        if matches!(self.peek(), Token::LBrace) {
            self.advance();
            while !matches!(self.peek(), Token::RBrace | Token::Eof) {
                self.skip_semicolons();
                match self.peek() {
                    Token::LocalAs => {
                        self.advance(); // skip 'local'
                        // skip 'as' keyword
                        if matches!(self.peek(), Token::Identifier(s) if s == "as") {
                            self.advance();
                        }
                        local_asn = self.expect_number()? as u32;
                    }
                    Token::RouterId => {
                        self.advance(); // skip 'router'
                        // skip 'id' keyword
                        if matches!(self.peek(), Token::Identifier(s) if s == "id") {
                            self.advance();
                        }
                        _router_id = self.expect_ident()?;
                    }
                    Token::Neighbor => {
                        self.advance();
                        let addr = self.expect_ident()?;
                        let mut asn = 0u32;
                        if matches!(self.peek(), Token::Identifier(s) if s == "as") {
                            self.advance();
                            asn = self.expect_number()? as u32;
                        }
                        neighbors.push(BgpNeighbor {
                            name: addr.clone(),
                            remote_address: addr,
                            remote_asn: asn,
                            address_families: vec![AddressFamily::Ipv4],
                            long_lived_graceful_restart: None,
                            llgr_stale_time_secs: None,
                            graceful_restart_mode: None,
                            link_bandwidth: None,
                        });
                    }
                    Token::Identifier(key) => {
                        let key_str = key.clone();
                        self.advance();
                        if matches!(self.peek(), Token::Equals) {
                            self.advance();
                        }
                        match key_str.as_str() {
                            "name" => {
                                name = self.expect_string()?;
                            }
                            "table" => {
                                let _ = self.expect_string()?;
                            }
                            _ => {
                                let _ = self.parse_value()?;
                            }
                        }
                    }
                    Token::RBrace => break,
                    _ => {
                        self.advance();
                    }
                }
                self.skip_semicolons();
            }
            if matches!(self.peek(), Token::RBrace) {
                self.advance();
            }
        }

        if local_asn == 0 {
            local_asn = 64512;
        }
        Ok(Some(ProtocolConfig::Bgp {
            name,
            table,
            local_asn,
            neighbors,
            import_table: None,
            export_table: None,
            update_delay_secs: None,
            advertisement_delay_secs: None,
            coalesce_time_millis: None,
            listen_range: None,
            vrf: None,
            view: None,
            from_template: None,
            aspa_downstream_check: None,
            aspa_upstream_check: None,
            limits: None,
            import_keep_filtered: None,
            rpki_reload: None,
            passwords: None,
            password: None,
            tx_class: None,
            tx_priority: None,
            description: None,
            mpls_channel: None,
            bgp_ls: None,
            bgpsec: None,
            flowspec: None,
        }))
    }

    fn parse_ospf_protocol(&mut self) -> Result<Option<ProtocolConfig>, ParseError> {
        let name = String::new();
        let table = "master".to_string();
        let mut areas = Vec::new();
        let mut router_id = None;

        if matches!(self.peek(), Token::LBrace) {
            self.advance();
            while !matches!(self.peek(), Token::RBrace | Token::Eof) {
                self.skip_semicolons();
                match self.peek() {
                    Token::RouterId => {
                        self.advance(); // skip 'router'
                        // skip 'id' keyword
                        if matches!(self.peek(), Token::Identifier(s) if s == "id") {
                            self.advance();
                        }
                        router_id = Some(self.expect_ident()?);
                    }
                    Token::Area => {
                        self.advance();
                        let area_id = self.expect_ident()?;
                        if matches!(self.peek(), Token::LBrace) {
                            self.advance();
                            while !matches!(self.peek(), Token::RBrace | Token::Eof) {
                                // Both Token::Stub and any other token currently
                                // result in a single advance(); the explicit Stub
                                // arm is a placeholder for future stub-specific
                                // handling.
                                self.advance();
                                self.skip_semicolons();
                            }
                            self.advance();
                        }
                        areas.push(OspfAreaConfig {
                            area_id,
                            nssa: None,
                            nssa_translator: None,
                            nssa_translator_stability_secs: None,
                            default_cost: None,
                            default_cost2: None,
                        });
                    }
                    Token::Identifier(key) => {
                        let key_str = key.clone();
                        self.advance();
                        if matches!(self.peek(), Token::Equals) {
                            self.advance();
                        }
                        let _val: Option<()> = match key_str.as_str() {
                            "name" => {
                                let _ = self.expect_string()?;
                                None
                            }
                            "table" => {
                                let _ = self.expect_string()?;
                                None
                            }
                            "instance" => {
                                let _ = self.expect_ident()?;
                                self.expect_number()?;
                                None
                            }
                            _ => {
                                let _ = self.parse_value()?;
                                None
                            }
                        };
                    }
                    Token::RBrace => break,
                    _ => {
                        self.advance();
                    }
                }
                self.skip_semicolons();
            }
            if matches!(self.peek(), Token::RBrace) {
                self.advance();
            }
        }

        Ok(Some(ProtocolConfig::Ospf {
            name,
            table,
            router_id,
            instance_id: None,
            ecmp: None,
            ecmp_limit: None,
            areas,
            stub_router: None,
            rfc1583_compat: None,
            merge_external: None,
            tick_secs: None,
            from_template: None,
            limits: None,
            import_keep_filtered: None,
            rpki_reload: None,
            passwords: None,
            tx_class: None,
            tx_priority: None,
            description: None,
            mpls_channel: None,
        }))
    }

    fn parse_value(&mut self) -> Result<serde_json::Value, ParseError> {
        match self.advance() {
            Token::Number(n) => Ok(serde_json::Value::Number((*n).into())),
            Token::String(s) => Ok(serde_json::Value::String(s.clone())),
            Token::Identifier(s) => Ok(serde_json::Value::String(s.clone())),
            Token::True | Token::Yes | Token::On => Ok(serde_json::Value::Bool(true)),
            Token::False | Token::No | Token::Off => Ok(serde_json::Value::Bool(false)),
            t => Err(ParseError::Message(format!(
                "unexpected value token: {t:?}"
            ))),
        }
    }

    #[allow(dead_code)]
    fn expect(&mut self, expected: Token) -> Result<(), ParseError> {
        let pos = self.pos;
        let got = self.advance().clone();
        if got == expected {
            Ok(())
        } else {
            Err(ParseError::UnexpectedToken {
                pos,
                expected: format!("{expected:?}"),
                got,
            })
        }
    }

    fn skip_block(&mut self) -> Result<(), ParseError> {
        if matches!(self.peek(), Token::LBrace) {
            self.advance();
            let mut depth = 1;
            while depth > 0 && !matches!(self.peek(), Token::Eof) {
                match self.advance() {
                    Token::LBrace => depth += 1,
                    Token::RBrace => depth -= 1,
                    _ => {}
                }
            }
        }
        Ok(())
    }
}

/// Parse a BIRD2 config string into a RoutePlaneConfig.
pub fn parse_bird_config(input: &str) -> Result<RoutePlaneConfig, ParseError> {
    let tokens = crate::lexer::tokenize(input);
    let mut parser = Parser::new(tokens);
    parser.parse_config()
}
