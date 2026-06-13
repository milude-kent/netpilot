# NetPilot M3 — BIRD2 Config Parser Design

Date: 2026-06-13

## Goal

Build a BIRD2-compatible configuration parser that reads `bird.conf` syntax and produces the structured `NetpilotConfig` used by the rest of the platform. The parser enables existing BIRD2 users to migrate configurations without rewriting them as JSON.

## Scope

### In scope

| Component | Description |
|-----------|-------------|
| Lexer | Tokenize `bird.conf` into BIRD tokens (keywords, IDs, strings, numbers, IPs, prefixes) |
| Parser | Recursive-descent parser: top-level blocks → protocol blocks → sub-blocks → expressions |
| Static protocol block | Parse `protocol static { ... }` into `ProtocolConfig::Static` |
| BGP protocol block | Parse `protocol bgp { ... }` with neighbor sub-blocks |
| OSPF protocol block | Parse `protocol ospf { ... }` with area sub-blocks |
| Filter functions | Parse `filter name { ... }` function definitions |
| Tables | Parse `table { ... }` and `ipv4/ipv6 table { ... }` definitions |
| AST → Config conversion | Map parsed AST nodes to `NetpilotConfig` struct values |
| Error reporting | Line/column information with descriptive messages |

### Out of scope

- Full BIRD2 filter expression syntax (partial — basic if/else, accept/reject)
- `include` directive / multi-file parsing
- BIRD2 template inheritance / protocol from template
- Kernel protocol, Device protocol parsing
- Pipe protocol parsing
- RADV protocol parsing

## Architecture

```
netpilot-birdconf/
  src/lib.rs              ← re-exports, parse_config() entry point
  src/lexer.rs            ← Lexer: str → Vec<Token>, line:col tracking
  src/token.rs            ← Token enum: 40+ BIRD token types
  src/parser.rs           ← Parser: TokenStream → ConfigAst
  src/ast.rs              ← ConfigAst, ProtocolBlock, TableBlock, FilterBlock
  src/builder.rs          ← ConfigBuilder: ConfigAst → NetpilotConfig
  src/error.rs            ← ParseError with span (line, col, snippet)
```

Dependencies: `netpilot-config` (for output type), no external parser framework (hand-written recursive descent).

## Key Types

**Token enum:** `KwProtocol, KwStatic, KwBgp, KwOspf, KwTable, KwFilter, KwIf, KwElse, KwAccept, KwReject, KwLocal, KwNeighbor, KwArea, OpenBrace, CloseBrace, Semicolon, Colon, StringLit, NumberLit, IpAddr, Prefix, Ident, ...`

**ConfigAst:** `Vec<AstItem>` where AstItem is `TableDef { name, nettype, options }, ProtocolDef { variant_tag, name, options, subblocks }, FilterDef { name, body }, Define { name, value }`

**ProtocolBlock:** `StaticBlock { table, routes[] }, BgpBlock { local_asn, table, option_map, neighbors[] }, OspfBlock { table, router_id, option_map, areas[] }`

**Builder:** Walks AST, constructs `NetpilotConfig`. Handles type coercion (string numbers to u32), default value application, and cross-reference resolution.

## Parsing Strategy

### Lexer phase
- Whitespace/newline insensitive, tracks line:col for errors
- Identifiers: `[a-zA-Z_][a-zA-Z0-9_]*`
- IP addresses: dotted-quad and colon-hex
- Prefixes: `ip/len` notation
- Strings: double-quoted with backslash escapes
- Comments: `#` to end of line, `/* ... */` block comments
- Numbers: decimal, hex (`0x...`)

### Parser phase (recursive descent)
```
config     = item*
item       = protocol_def | table_def | filter_def | define
protocol_def = "protocol" type (name)? "{" protocol_options "}"
type       = "static" | "bgp" | "ospf" | ... (validated from ProtocolKind)
protocol_options = option* subblock*
option     = keyword value ";"
subblock   = "neighbor" name "{" neighbor_options "}"
           | "area" id "{" area_options "}"
           | "ipv4" "{" option* "}"     // address-family sub-block
```

### ~40% syntax coverage
The parser currently handles the most common BIRD2 constructs used in production: static routes, basic BGP peers, OSPF areas. Missing: filter expressions with arithmetic, prefix sets, community lists, BGP template inheritance, graceful restart options, BFD integration options, device protocol, kernel protocol, pipe protocol, and many advanced `bgp` sub-options.

## Files Changed

| File | Approx lines |
|------|-------------|
| `crates/netpilot-birdconf/Cargo.toml` | +12 |
| `crates/netpilot-birdconf/src/lib.rs` | +40 |
| `crates/netpilot-birdconf/src/token.rs` | +80 |
| `crates/netpilot-birdconf/src/lexer.rs` | +200 |
| `crates/netpilot-birdconf/src/ast.rs` | +100 |
| `crates/netpilot-birdconf/src/parser.rs` | +250 |
| `crates/netpilot-birdconf/src/builder.rs` | +200 |
| `crates/netpilot-birdconf/src/error.rs` | +40 |
| Tests | +150 |
| **Total** | **~1072** |

## Design Decisions

1. **Hand-written recursive descent, not parser generator**: BIRD2 config syntax is line-oriented and keyword-heavy, making it a better fit for hand-rolled parsing than a grammar-based generator. Error messages are more descriptive with custom parser logic.

2. **Lexer + Parser separation**: Clean separation allows testing the lexer independently (input → token stream) and the parser independently (token stream → AST). Makes error recovery simpler.

3. **AST as separate from config**: The AST mirrors BIRD2 syntax structure. The Builder step maps BIRD2 concepts to NetPilot's internal schema. This two-pass design allows the parser to be syntax-complete even when internal config doesn't support a feature (graceful degradation + warnings).

4. **Span metadata on all AST nodes**: Every AST node carries source location (file, start line:col, end line:col) for error reporting. No node is anonymous.

5. **Strict vs lenient mode**: Parser has a `strict` flag. In strict mode, unknown options are errors. In lenient mode (default for migration), unknown options produce warnings and are skipped. This supports gradual migration from BIRD2.

For the canonical implementation, see `crates/netpilot-birdconf/`.
