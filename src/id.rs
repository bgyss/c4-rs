use crate::sha512::sha512;
use std::cmp::Ordering;
use std::fmt;
use std::io::{self, Read};

const CHARSET: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
pub const ID_LEN: usize = 90;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Id(pub [u8; 64]);

#[derive(Debug, Eq, PartialEq)]
pub enum ParseError {
    BadLength(usize),
    BadChar(usize),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::BadLength(len) => {
                write!(f, "c4 ids must be 90 characters long, input length {len}")
            }
            ParseError::BadChar(pos) => write!(f, "non c4 id character at position {pos}"),
        }
    }
}

impl std::error::Error for ParseError {}

impl Id {
    pub const fn nil() -> Self {
        Self([0; 64])
    }

    pub fn is_nil(&self) -> bool {
        self.0.iter().all(|b| *b == 0)
    }

    pub fn digest(&self) -> &[u8; 64] {
        &self.0
    }

    pub fn cmp_id(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }

    pub fn less(&self, other: &Self) -> bool {
        self.cmp_id(other) == Ordering::Less
    }

    pub fn sum(self, other: Self) -> Self {
        if self == other {
            return self;
        }
        let (left, right) = if self.0 <= other.0 {
            (self, other)
        } else {
            (other, self)
        };
        let mut data = [0u8; 128];
        data[..64].copy_from_slice(&left.0);
        data[64..].copy_from_slice(&right.0);
        Self(sha512(&data))
    }
}

impl Ord for Id {
    fn cmp(&self, other: &Self) -> Ordering {
        self.cmp_id(other)
    }
}

impl PartialOrd for Id {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut number = self.0;
        let mut encoded = [b'1'; ID_LEN];
        encoded[0] = b'c';
        encoded[1] = b'4';

        for slot in (2..ID_LEN).rev() {
            let mut rem: u16 = 0;
            let mut any = false;
            for byte in &mut number {
                let value = (rem << 8) + (*byte as u16);
                let q = value / 58;
                rem = value % 58;
                *byte = q as u8;
                any |= *byte != 0;
            }
            encoded[slot] = CHARSET[rem as usize];
            if !any {
                break;
            }
        }

        f.write_str(std::str::from_utf8(&encoded).expect("base58 output is utf8"))
    }
}

pub fn identify_bytes(data: &[u8]) -> Id {
    Id(sha512(data))
}

pub fn identify<R: Read>(mut reader: R) -> io::Result<Id> {
    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;
    Ok(identify_bytes(&data))
}

pub fn parse(source: &str) -> Result<Id, ParseError> {
    let bytes = source.as_bytes();
    if bytes.len() != ID_LEN {
        return Err(ParseError::BadLength(bytes.len()));
    }

    let mut out = [0u8; 64];
    for (i, byte) in bytes.iter().enumerate().skip(2) {
        let value = CHARSET
            .iter()
            .position(|candidate| candidate == byte)
            .ok_or(ParseError::BadChar(i))? as u16;
        let mut carry = value;
        for slot in out.iter_mut().rev() {
            let n = (*slot as u16) * 58 + carry;
            *slot = (n & 0xff) as u8;
            carry = n >> 8;
        }
    }
    Ok(Id(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_empty_input() {
        let id = identify_bytes(b"");
        assert_eq!(
            id.to_string(),
            "c459dsjfscH38cYeXXYogktxf4Cd9ibshE3BHUo6a58hBXmRQdZrAkZzsWcbWtDg5oQstpDuni4Hirj75GEmTc1sFT"
        );
    }

    #[test]
    fn round_trips_boundaries() {
        let zero = Id([0; 64]);
        assert_eq!(
            zero.to_string(),
            "c41111111111111111111111111111111111111111111111111111111111111111111111111111111111111111"
        );
        assert_eq!(parse(&zero.to_string()).unwrap(), zero);

        let max = Id([0xff; 64]);
        assert_eq!(
            max.to_string(),
            "c467rpwLCuS5DGA8KGZXKsVQ7dnPb9goRLoKfgGbLfQg9WoLUgNY77E2jT11fem3coV9nAkguBACzrU1iyZM4B8roQ"
        );
        assert_eq!(parse(&max.to_string()).unwrap(), max);
    }

    #[test]
    fn rejects_bad_input_like_go() {
        assert_eq!(
            parse("c430cjRutKqZSCrW43QGU1uwRZTGoVD7A7kPHKQ1z4X1Ge8mhW4Q1gk48Ld8VFpprQBfUC8JNvHYVgq453hCFrgf9D")
                .unwrap_err()
                .to_string(),
            "non c4 id character at position 3"
        );
        assert_eq!(
            parse("c430").unwrap_err().to_string(),
            "c4 ids must be 90 characters long, input length 4"
        );
    }
}
