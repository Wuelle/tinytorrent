use anyhow::{bail, ensure, Context, Result};
use ascii::{AsciiString, AsciiChar};
use std::collections::BTreeMap;
use std::fmt;
use std::fs::File;
use std::io::{BufReader, Bytes};
use thiserror::Error;

#[derive(Error, Debug)]
enum ParseError {
    #[error("unexpected byte: {0}")]
    UnexpectedByte(u8),
    #[error("Unexpected EOF")]
    UnexpectedEOF,
    #[error("Invalid Format")]
    InvalidFormat,
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub enum Value {
    /// any integer value
    Integer(i64),
    /// A Sequence of bytes.
    ByteString(Vec<u8>),
    /// a list of (possibly different) values
    List(Vec<Value>),
    /// Though this implementation allows otherwise, keys must always be Value::ByteString
    Dictionary(BTreeMap<Value, Value>),
    /// marks the end of items like lists or dictionaries
    End,
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Integer(x) => f.debug_tuple("Integer").field(&x).finish(),
            Value::ByteString(x) => f
                .debug_tuple("ByteString")
                .field(&String::from_utf8_lossy(x))
                .finish(),
            Value::List(items) => f.debug_list().entries(items.iter()).finish(),
            Value::Dictionary(d) => f.debug_map().entries(d.iter()).finish(),
            Value::End => f.write_str("End"),
        }
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        let bytes: Vec<u8> = s.chars().map(|x| x as u8).collect();
        Value::ByteString(bytes)
    }
}

impl From<&Value> for AsciiString {
    fn from(v: &Value) -> Self {
        let mut res = AsciiString::new();
        match v {
            Value::Integer(i) => {
                res.push(AsciiChar::new('i'));
                res.push_str(&AsciiString::from_ascii(i.to_string()).unwrap());
                res.push(AsciiChar::new('e'));
            }
            Value::ByteString(bytes) => {
                res.push_str(&AsciiString::from_ascii(bytes.len().to_string()).unwrap());
                res.push(AsciiChar::new(':'));
                res.push_str(&AsciiString::from_ascii(hex::encode(bytes.clone())).unwrap());
            }
            Value::List(l) => {
                res.push(AsciiChar::new('l'));
                for item in l {
                    res.push_str(&AsciiString::from(item));
                }
                res.push(AsciiChar::new('e'));
            }
            Value::Dictionary(map) => {
                res.push(AsciiChar::new('d'));
                // Values are, by default, sorted in lexicographical order
                for (key, value) in map {
                    res.push_str(&AsciiString::from(key));
                    res.push_str(&AsciiString::from(value));
                }
                res.push(AsciiChar::new('e'));
            }
            Value::End => res.push(AsciiChar::new('e')),
        }
        res
    }
}

impl Value {
    fn from_byte_string(i: &Value) -> Result<AsciiString> {
        if let Value::ByteString(s) = i {
            return Ok(AsciiString::from_ascii(s.clone())?);
        }
        bail!("Expected a ByteString, found {:?}", i);
    }
}

/// parse a single bencoded value from array of bytes
fn parse_benc_value(bytes: &mut Bytes<BufReader<File>>) -> Result<Option<Value>> {
    let val = match bytes.next() {
        Some(n) => {
            let a = n?;
            match a {
                b'i' => {
                    let mut x = 0_i64;
                    loop {
                        let val = bytes
                            .next()
                            .transpose()
                            .map_err(|err| anyhow::Error::from(err))?
                            .ok_or(ParseError::UnexpectedEOF)?;
                        if val == b'e' {
                            break;
                        }
                        x *= 10;
                        x += i64::from(val - 48);
                    }
                    Value::Integer(x)
                }
                b'l' => {
                    let mut items = vec![];
                    loop {
                        let val = parse_benc_value(bytes)?.ok_or(ParseError::UnexpectedEOF)?;
                        if let Value::End = val {
                            break;
                        }
                        items.push(val);
                    }
                    Value::List(items)
                }
                // [48, 57] is ascii for [0, 9]
                48..=57 => {
                    // read all decimals
                    let mut len = (a - 48) as usize; // convert from ascii to decimal
                    let val = loop {
                        let val = bytes
                            .next()
                            .transpose()
                            .map_err(|err| anyhow::Error::from(err))?
                            .ok_or(ParseError::UnexpectedEOF)?;
                        // if the next byte is still a decimal number
                        if 48 <= val && val <= 57 {
                            len *= 10;
                            len += (val as usize) - 48;
                        } else {
                            break val;
                        }
                    };

                    ensure!(val == b':', ParseError::UnexpectedByte(val));

                    let mut s = vec![];
                    for _ in 0..len {
                        s.push(
                            bytes
                                .next()
                                .transpose()
                                .map_err(|err| anyhow::Error::from(err))?
                                .ok_or(ParseError::UnexpectedEOF)?,
                        );
                    }
                    Value::ByteString(s)
                }
                b'd' => {
                    let mut map = BTreeMap::new();

                    loop {
                        let key = parse_benc_value(bytes)?.ok_or(ParseError::UnexpectedEOF)?;
                        if let Value::End = key {
                            break;
                        }
                        let value = parse_benc_value(bytes)?.ok_or(ParseError::UnexpectedEOF)?;
                        map.insert(key, value);
                    }
                    Value::Dictionary(map)
                }
                b'e' => Value::End,
                _ => return Err(ParseError::UnexpectedEOF.into()),
            }
        }
        None => {
            return Ok(None);
        }
    };
    Ok(Some(val))
}

/// Parse a bytestream into the root dictionary
pub fn parse_torrent_file(bytes: &mut Bytes<BufReader<File>>) -> Result<BTreeMap<Value, Value>> {
    let val = parse_benc_value(bytes).context("failed to parse benc value")?;
    if let Some(Value::Dictionary(map)) = val {
        return Ok(map);
    }
    Err(ParseError::InvalidFormat.into())
}
