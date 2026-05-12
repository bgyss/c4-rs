use crate::id::{identify_bytes, parse as parse_id, Id, ParseError as IdParseError};
use std::fmt::{self, Write};

pub const TIMESTAMP_FORMAT: &str = "%Y-%m-%dT%H:%M:%SZ";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FlowDirection {
    None,
    Outbound,
    Inbound,
    Bidirectional,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    pub mode: Option<u32>,
    pub timestamp: Option<String>,
    pub size: Option<i64>,
    pub name: String,
    pub target: Option<String>,
    pub c4id: Option<Id>,
    pub depth: usize,
    pub hard_link: i32,
    pub flow_direction: FlowDirection,
    pub flow_target: Option<String>,
    pub is_sequence: bool,
}

impl Entry {
    pub fn file(name: impl Into<String>, c4id: Id) -> Self {
        Self {
            mode: Some(0o100644),
            timestamp: None,
            size: None,
            name: name.into(),
            target: None,
            c4id: Some(c4id),
            depth: 0,
            hard_link: 0,
            flow_direction: FlowDirection::None,
            flow_target: None,
            is_sequence: false,
        }
    }

    pub fn is_dir(&self) -> bool {
        self.mode.map(|m| m & 0o040000 != 0).unwrap_or(false) || self.name.ends_with('/')
    }

    pub fn canonical(&self) -> String {
        self.format_with(0)
    }

    pub fn format(&self, indent_width: usize) -> String {
        self.format_with(indent_width)
    }

    fn format_with(&self, indent_width: usize) -> String {
        let mut out = String::with_capacity(128 + self.name.len());
        self.write_format_with(&mut out, indent_width);
        out
    }

    fn write_format_with(&self, out: &mut String, indent_width: usize) {
        for _ in 0..self.depth * indent_width {
            out.push(' ');
        }
        if let Some(mode) = self.mode {
            write_mode(out, mode);
        } else {
            out.push('-');
        }

        out.push(' ');
        out.push_str(self.timestamp.as_deref().unwrap_or("-"));

        out.push(' ');
        if let Some(size) = self.size {
            write!(out, "{size}").expect("write to string");
        } else {
            out.push('-');
        }

        out.push(' ');
        write_name(out, &self.name, self.is_sequence);

        if let Some(target) = &self.target {
            out.push_str(" -> ");
            write_name(out, target, false);
        } else if self.hard_link != 0 {
            out.push(' ');
            if self.hard_link < 0 {
                out.push_str("->");
            } else {
                write!(out, "->{}", self.hard_link).expect("write to string");
            }
        } else {
            match self.flow_direction {
                FlowDirection::Outbound => {
                    out.push_str(" -> ");
                    out.push_str(self.flow_target.as_deref().unwrap_or_default());
                }
                FlowDirection::Inbound => {
                    out.push_str(" <- ");
                    out.push_str(self.flow_target.as_deref().unwrap_or_default());
                }
                FlowDirection::Bidirectional => {
                    out.push_str(" <> ");
                    out.push_str(self.flow_target.as_deref().unwrap_or_default());
                }
                FlowDirection::None => {}
            }
        }

        out.push(' ');
        if let Some(id) = self.c4id {
            write!(out, "{id}").expect("write to string");
        } else {
            out.push('-');
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Manifest {
    pub entries: Vec<Entry>,
}

#[derive(Debug)]
pub enum ParseError {
    MissingField(usize),
    InvalidMode(String),
    InvalidSize(String),
    InvalidId(IdParseError),
    UnterminatedQuote,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingField(line) => write!(f, "line {line}: missing required field"),
            Self::InvalidMode(mode) => write!(f, "invalid mode {mode:?}"),
            Self::InvalidSize(size) => write!(f, "invalid size {size:?}"),
            Self::InvalidId(err) => write!(f, "invalid C4 ID: {err}"),
            Self::UnterminatedQuote => write!(f, "unterminated quoted field"),
        }
    }
}

impl std::error::Error for ParseError {}

impl Manifest {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn add_entry(&mut self, entry: Entry) {
        self.entries.push(entry);
    }

    pub fn canonical(&self) -> String {
        let min_depth = match self.entries.iter().map(|e| e.depth).min() {
            Some(depth) => depth,
            None => return String::new(),
        };
        let mut entries: Vec<&Entry> = self
            .entries
            .iter()
            .filter(|entry| entry.depth == min_depth)
            .collect();
        entries.sort_by_cached_key(|entry| natural_key(&entry.name));

        let mut out = String::new();
        for entry in entries {
            entry.write_format_with(&mut out, 0);
            out.push('\n');
        }
        out
    }

    pub fn compute_c4_id(&self) -> Id {
        identify_bytes(self.canonical().as_bytes())
    }

    pub fn entry_paths(&self) -> Vec<String> {
        let mut stack: Vec<String> = Vec::new();
        let mut paths = Vec::new();
        for entry in &self.entries {
            stack.truncate(entry.depth);
            let full = format!("{}{}", stack.concat(), entry.name);
            if entry.is_dir() {
                if stack.len() <= entry.depth {
                    stack.resize(entry.depth + 1, String::new());
                }
                stack[entry.depth] = entry.name.clone();
            }
            paths.push(full);
        }
        paths.sort();
        paths
    }
}

pub fn parse_manifest(input: &str) -> Result<Manifest, ParseError> {
    let mut manifest = Manifest::new();
    for (line_no, line) in input.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let trimmed = line.trim();
        if trimmed.starts_with("c4") && trimmed.len() == 90 {
            continue;
        }
        manifest.add_entry(parse_entry(line, line_no + 1)?);
    }
    Ok(manifest)
}

pub fn parse_manifest_chain(input: &str) -> Result<Vec<Manifest>, ParseError> {
    let mut manifests = Vec::new();
    let mut current = Manifest::new();
    for (line_no, line) in input.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("c4") && trimmed.len() == 90 {
            if !current.entries.is_empty() {
                manifests.push(current);
                current = Manifest::new();
            }
            continue;
        }
        current.add_entry(parse_entry(line, line_no + 1)?);
    }
    if !current.entries.is_empty() {
        manifests.push(current);
    }
    Ok(manifests)
}

pub fn format_manifest(input: &str) -> Result<String, ParseError> {
    Ok(parse_manifest(input)?.canonical())
}

pub fn parse_entry(line: &str, line_no: usize) -> Result<Entry, ParseError> {
    let leading_spaces = line.len() - line.trim_start_matches(' ').len();
    let depth = leading_spaces / 2;
    let fields = split_fields(line.trim_start())?;
    if fields.len() < 4 {
        return Err(ParseError::MissingField(line_no));
    }

    let mode = if fields[0] == "-" {
        None
    } else {
        Some(parse_mode(&fields[0])?)
    };
    let timestamp = if fields[1] == "-" {
        None
    } else {
        Some(fields[1].clone())
    };
    let size = if fields[2] == "-" {
        None
    } else {
        Some(
            fields[2]
                .parse::<i64>()
                .map_err(|_| ParseError::InvalidSize(fields[2].clone()))?,
        )
    };
    let mut entry = Entry {
        mode,
        timestamp,
        size,
        name: unsequence(&fields[3]),
        target: None,
        c4id: None,
        depth,
        hard_link: 0,
        flow_direction: FlowDirection::None,
        flow_target: None,
        is_sequence: fields[3].starts_with('[') && fields[3].ends_with(']'),
    };

    let mut id_field_index = 4;
    if fields.len() >= 6 {
        match fields[4].as_str() {
            "->" => {
                entry.target = Some(fields[5].clone());
                id_field_index = 6;
            }
            "<-" => {
                entry.flow_direction = FlowDirection::Inbound;
                entry.flow_target = Some(fields[5].clone());
                id_field_index = 6;
            }
            "<>" => {
                entry.flow_direction = FlowDirection::Bidirectional;
                entry.flow_target = Some(fields[5].clone());
                id_field_index = 6;
            }
            marker if marker.starts_with("->") && marker.len() > 2 => {
                entry.hard_link = marker[2..].parse().unwrap_or(-1);
                id_field_index = 5;
            }
            _ => {}
        }
    } else if fields.len() >= 5 && fields[4].starts_with("->") && fields[4] != "->" {
        entry.hard_link = fields[4][2..].parse().unwrap_or(-1);
        id_field_index = 5;
    }

    if let Some(id_field) = fields.get(id_field_index) {
        if id_field != "-" {
            entry.c4id = Some(parse_id(id_field).map_err(ParseError::InvalidId)?);
        }
    }
    Ok(entry)
}

pub fn format_mode(mode: u32) -> String {
    let mut out = String::with_capacity(10);
    write_mode(&mut out, mode);
    out
}

fn write_mode(out: &mut String, mode: u32) {
    let kind = match mode & 0o170000 {
        0o040000 => 'd',
        0o120000 => 'l',
        0o010000 => 'p',
        0o140000 => 's',
        0o060000 => 'b',
        0o020000 => 'c',
        _ => '-',
    };
    out.push(kind);
    for shift in [6, 3, 0] {
        out.push(if mode & (0o4 << shift) != 0 { 'r' } else { '-' });
        out.push(if mode & (0o2 << shift) != 0 { 'w' } else { '-' });
        out.push(if mode & (0o1 << shift) != 0 { 'x' } else { '-' });
    }
}

fn parse_mode(mode: &str) -> Result<u32, ParseError> {
    if mode.len() != 10 {
        return Err(ParseError::InvalidMode(mode.to_string()));
    }
    let bytes = mode.as_bytes();
    let mut out = match bytes[0] as char {
        'd' => 0o040000,
        'l' => 0o120000,
        'p' => 0o010000,
        's' => 0o140000,
        'b' => 0o060000,
        'c' => 0o020000,
        '-' => 0o100000,
        _ => return Err(ParseError::InvalidMode(mode.to_string())),
    };
    for (idx, bit) in [
        (1, 0o400),
        (2, 0o200),
        (3, 0o100),
        (4, 0o040),
        (5, 0o020),
        (6, 0o010),
        (7, 0o004),
        (8, 0o002),
        (9, 0o001),
    ] {
        if bytes[idx] != b'-' {
            out |= bit;
        }
    }
    Ok(out)
}

fn write_name(out: &mut String, name: &str, is_sequence: bool) {
    if is_sequence {
        out.push('[');
        out.push_str(name);
        out.push(']');
        return;
    }
    if name.is_empty()
        || name
            .bytes()
            .any(|b| b.is_ascii_whitespace() || matches!(b, b'"' | b'\\'))
    {
        out.push('"');
        for ch in name.chars() {
            if matches!(ch, '\\' | '"') {
                out.push('\\');
            }
            out.push(ch);
        }
        out.push('"');
    } else {
        out.push_str(name);
    }
}

fn split_fields(input: &str) -> Result<Vec<String>, ParseError> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars().peekable();
    let mut quoted = false;
    while let Some(ch) = chars.next() {
        match ch {
            '"' => quoted = !quoted,
            '\\' if quoted => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            c if c.is_whitespace() && !quoted => {
                if !current.is_empty() {
                    fields.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if quoted {
        return Err(ParseError::UnterminatedQuote);
    }
    if !current.is_empty() {
        fields.push(current);
    }
    Ok(fields)
}

fn unsequence(name: &str) -> String {
    if name.starts_with('[') && name.ends_with(']') && name.len() > 1 {
        name[1..name.len() - 1].to_string()
    } else {
        name.to_string()
    }
}

fn natural_key(input: &str) -> Vec<NaturalPart> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut digit_mode = None;
    for ch in input.chars() {
        let is_digit = ch.is_ascii_digit();
        if digit_mode == Some(is_digit) {
            current.push(ch);
        } else {
            if !current.is_empty() {
                parts.push(NaturalPart::from(&current, digit_mode.unwrap()));
            }
            current.clear();
            current.push(ch);
            digit_mode = Some(is_digit);
        }
    }
    if !current.is_empty() {
        parts.push(NaturalPart::from(&current, digit_mode.unwrap()));
    }
    parts
}

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
enum NaturalPart {
    Text(String),
    Number(u128, usize),
}

impl NaturalPart {
    fn from(s: &str, digits: bool) -> Self {
        if digits {
            Self::Number(s.parse().unwrap_or(0), s.len())
        } else {
            Self::Text(s.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::identify_bytes;

    #[test]
    fn formats_go_style_entry() {
        let entry = Entry {
            mode: Some(0o100644),
            timestamp: Some("2025-01-01T12:00:00Z".to_string()),
            size: Some(13),
            name: "hello.txt".to_string(),
            target: None,
            c4id: Some(identify_bytes(b"Hello, World!")),
            depth: 0,
            hard_link: 0,
            flow_direction: FlowDirection::None,
            flow_target: None,
            is_sequence: false,
        };
        assert_eq!(
            entry.format(2),
            "-rw-r--r-- 2025-01-01T12:00:00Z 13 hello.txt c4278VoUM5dXnzULoTV6JqiyoeyFaL4DZo2oDPTsmDAE4Ki4Uwe8PZyENUh9uBHhWQ5HCvgb72Emg4nSazsTRophmx"
        );
    }

    #[test]
    fn parses_and_formats_manifest() {
        let input = "-rw-r--r-- 2025-01-01T00:00:00Z 100 file.txt\n";
        let manifest = parse_manifest(input).unwrap();
        assert_eq!(manifest.entries.len(), 1);
        assert_eq!(manifest.entries[0].name, "file.txt");
        assert_eq!(
            format_manifest(input).unwrap(),
            "-rw-r--r-- 2025-01-01T00:00:00Z 100 file.txt -\n"
        );
    }
}
