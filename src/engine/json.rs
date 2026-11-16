//! A small JSON reader for the rules file (RFC 8259: objects, arrays,
//! strings with every escape, numbers, `true`, `false`, `null`). A native
//! mod builds against the SDK alone, so no serde; the file is small and read
//! rarely, so a plain recursive descent with a depth cap is enough.

/// A parsed value. An object keeps its members in file order.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

/// Deepest nesting accepted (the rules file nests three deep).
const MAX_DEPTH: usize = 32;

impl Json {
    /// Parses `text` as one JSON value, refusing anything after it.
    pub fn parse(text: &str) -> Result<Json, String> {
        let mut p = Parser {
            bytes: text.as_bytes(),
            at: 0,
        };
        let value = p.value(0)?;
        p.space();
        if p.at != p.bytes.len() {
            return Err(p.error("text after the value"));
        }
        Ok(value)
    }

    /// What the value is, for messages.
    pub fn kind(&self) -> &'static str {
        match self {
            Json::Null => "null",
            Json::Bool(_) => "a boolean",
            Json::Number(_) => "a number",
            Json::String(_) => "a string",
            Json::Array(_) => "an array",
            Json::Object(_) => "an object",
        }
    }
}

struct Parser<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Parser<'_> {
    fn error(&self, what: &str) -> String {
        let text = &self.bytes[..self.at.min(self.bytes.len())];
        let line = text.iter().filter(|&&b| b == b'\n').count() + 1;
        let column = text.iter().rev().take_while(|&&b| b != b'\n').count() + 1;
        format!("{what} at line {line}, column {column}")
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    fn space(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    fn eat(&mut self, b: u8) -> bool {
        self.space();
        if self.peek() == Some(b) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, b: u8) -> Result<(), String> {
        if self.eat(b) {
            Ok(())
        } else {
            Err(self.error(&format!("expected '{}'", b as char)))
        }
    }

    fn word(&mut self, word: &str, value: Json) -> Result<Json, String> {
        if self.bytes[self.at..].starts_with(word.as_bytes()) {
            self.at += word.len();
            Ok(value)
        } else {
            Err(self.error("expected a value"))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Json, String> {
        if depth > MAX_DEPTH {
            return Err(self.error("nested too deep"));
        }
        self.space();
        match self.peek() {
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => self.string().map(Json::String),
            Some(b't') => self.word("true", Json::Bool(true)),
            Some(b'f') => self.word("false", Json::Bool(false)),
            Some(b'n') => self.word("null", Json::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(self.error("expected a value")),
            None => Err(self.error("unexpected end")),
        }
    }

    fn object(&mut self, depth: usize) -> Result<Json, String> {
        self.at += 1;
        let mut members = Vec::new();
        if self.eat(b'}') {
            return Ok(Json::Object(members));
        }
        loop {
            self.space();
            if self.peek() != Some(b'"') {
                return Err(self.error("expected a member name"));
            }
            let name = self.string()?;
            if members.iter().any(|(n, _)| *n == name) {
                return Err(self.error(&format!("{name:?} given twice")));
            }
            self.expect(b':')?;
            members.push((name, self.value(depth + 1)?));
            if self.eat(b'}') {
                return Ok(Json::Object(members));
            }
            self.expect(b',')?;
        }
    }

    fn array(&mut self, depth: usize) -> Result<Json, String> {
        self.at += 1;
        let mut items = Vec::new();
        if self.eat(b']') {
            return Ok(Json::Array(items));
        }
        loop {
            items.push(self.value(depth + 1)?);
            if self.eat(b']') {
                return Ok(Json::Array(items));
            }
            self.expect(b',')?;
        }
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let digits = self
            .bytes
            .get(self.at..self.at + 4)
            .and_then(|d| std::str::from_utf8(d).ok())
            .filter(|d| d.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or_else(|| self.error("expected four hex digits"))?;
        self.at += 4;
        u32::from_str_radix(digits, 16).map_err(|_| self.error("expected four hex digits"))
    }

    fn string(&mut self) -> Result<String, String> {
        self.at += 1;
        let mut out = String::new();
        loop {
            let start = self.at;
            while let Some(b) = self.peek() {
                if b == b'"' || b == b'\\' || b < 0x20 {
                    break;
                }
                self.at += 1;
            }
            // The input is a &str and the run stops at ASCII bytes, so it is whole UTF-8.
            out.push_str(std::str::from_utf8(&self.bytes[start..self.at]).unwrap_or_default());
            match self.peek() {
                Some(b'"') => {
                    self.at += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.at += 1;
                    let escape = self.peek().ok_or_else(|| self.error("unexpected end"))?;
                    self.at += 1;
                    match escape {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let mut code = self.hex4()?;
                            if (0xD800..0xDC00).contains(&code) {
                                if !self.bytes[self.at..].starts_with(b"\\u") {
                                    return Err(self.error("a lone surrogate"));
                                }
                                self.at += 2;
                                let low = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&low) {
                                    return Err(self.error("a lone surrogate"));
                                }
                                code = 0x10000 + ((code - 0xD800) << 10) + (low - 0xDC00);
                            }
                            out.push(
                                char::from_u32(code)
                                    .ok_or_else(|| self.error("a lone surrogate"))?,
                            );
                        }
                        _ => return Err(self.error("an unknown escape")),
                    }
                }
                Some(_) => return Err(self.error("a control character in a string")),
                None => return Err(self.error("an unterminated string")),
            }
        }
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.at;
        let digits = |p: &mut Self| {
            let from = p.at;
            while matches!(p.peek(), Some(b'0'..=b'9')) {
                p.at += 1;
            }
            p.at > from
        };
        if self.peek() == Some(b'-') {
            self.at += 1;
        }
        if self.peek() == Some(b'0') {
            self.at += 1;
        } else if !digits(self) {
            return Err(self.error("expected a digit"));
        }
        if self.peek() == Some(b'.') {
            self.at += 1;
            if !digits(self) {
                return Err(self.error("expected a digit"));
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.at += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.at += 1;
            }
            if !digits(self) {
                return Err(self.error("expected a digit"));
            }
        }
        std::str::from_utf8(&self.bytes[start..self.at])
            .ok()
            .and_then(|n| n.parse().ok())
            .map(Json::Number)
            .ok_or_else(|| self.error("a bad number"))
    }
}

/// `text` as a JSON string literal.
pub fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_kind() {
        let text = r#" {"a": [1, -2.5e2, 0], "b": {"c": true, "d": false, "e": null},
            "s": "x\"\\\/\b\f\n\r\t\u00e9\ud83d\ude00", "raw": "é"} "#;
        let Json::Object(m) = Json::parse(text).unwrap() else {
            panic!()
        };
        assert_eq!(
            m[0].1,
            Json::Array(vec![
                Json::Number(1.0),
                Json::Number(-250.0),
                Json::Number(0.0)
            ])
        );
        assert_eq!(
            m[1].1,
            Json::Object(vec![
                ("c".into(), Json::Bool(true)),
                ("d".into(), Json::Bool(false)),
                ("e".into(), Json::Null),
            ])
        );
        assert_eq!(
            m[2].1,
            Json::String("x\"\\/\u{8}\u{c}\n\r\t\u{e9}\u{1F600}".into())
        );
        assert_eq!(m[3].1, Json::String("é".into()));
    }

    #[test]
    fn refuses_what_json_refuses() {
        for text in [
            "",
            "{",
            "[1,]",
            "{\"a\":1,}",
            "{a:1}",
            "01",
            "1.",
            "-",
            "+1",
            "'x'",
            "\"\\x\"",
            "\"\\ud800\"",
            "\"a\nb\"",
            "{} {}",
            "{\"a\":1,\"a\":2}",
            "nul",
            "not json",
        ] {
            assert!(Json::parse(text).is_err(), "{text:?}");
        }
        let deep = "[".repeat(40) + &"]".repeat(40);
        assert!(Json::parse(&deep).is_err());
    }

    #[test]
    fn errors_name_the_place() {
        let error = Json::parse("{\n  \"a\": tru\n}").unwrap_err();
        assert!(error.ends_with("line 2, column 8"), "{error}");
    }

    #[test]
    fn quoted_strings_read_back() {
        let text = "a\"b\\c\nd\u{1}é";
        assert_eq!(
            Json::parse(&quote(text)).unwrap(),
            Json::String(text.into())
        );
    }
}
