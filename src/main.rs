use anyhow::{ensure, Context, Result};
use sha1::{Digest, Sha1};
use std::collections::BTreeMap;
use std::fmt;
use std::fs::File;
use std::io::{BufReader, Bytes, Read};
use structopt::StructOpt;
use thiserror::Error;

#[derive(Error, Debug)]
enum ParseError {
    #[error("unexpected byte: {0}")]
    UnexpectedByte(u8),
    #[error("Unexpected EOF")]
    UnexpectedEOF,
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
                        x += i64::from(val);
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
                    let mut len = a - 48; // convert from ascii to decimal
                    let val = loop {
                        let val = bytes
                            .next()
                            .transpose()
                            .map_err(|err| anyhow::Error::from(err))?
                            .ok_or(ParseError::UnexpectedEOF)?;
                        // if the next byte is still a decimal number
                        if 48 <= val && val <= 57 {
                            len *= 10;
                            len += val - 48;
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
                    Value::ByteString(len, s)
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

#[derive(StructOpt)]
struct Cli {
    /// The input torrent file
    #[structopt(parse(from_os_str))]
    path: std::path::PathBuf,
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum Value {
    /// any integer value
    Integer(i64),
    /// A Sequence of bytes. First parameter represents the length
    ByteString(u8, Vec<u8>),
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
            Value::ByteString(len, x) => f
                .debug_tuple("ByteString")
                .field(&len)
                .field(&String::from_utf8_lossy(x))
                .finish(),
            Value::List(items) => f.debug_list().entries(items.iter()).finish(),
            Value::Dictionary(d) => f.debug_map().entries(d.iter()).finish(),
            Value::End => f.write_str("End"),
        }
    }
}

fn main() -> Result<()> {
    let args = Cli::from_args();
    ensure!(
        args.path.extension().is_some() && args.path.extension().unwrap() == "torrent",
        format!("{:#?} is not a torrent (.torrent) file", &args.path)
    );
    let f =
        File::open(&args.path).with_context(|| format!("could not open file {:#?}", &args.path))?;
    let reader = BufReader::new(f);

    let torrent_file = parse_benc_value(&mut reader.bytes())
        .with_context(|| format!("failed to parse {:#?}", &args.path))?;
    println!("{:?}", torrent_file);

    // Calculate the infohash (SHA-1 of the contents of the "info" dictionary)
    let mut hasher = Sha1::new();
    hasher.update(b"Hello World");
    let result = hasher.finalize();
    println!("infohash: {:?}", result);

    Ok(())
}
