mod parser;

use crate::parser::*;
use anyhow::{bail, ensure, Context, Result};
use sha1::{Digest, Sha1};
use std::fs::File;
use std::io::{BufReader, Read};
use structopt::StructOpt;
use rand::{thread_rng, Rng};
use rand::distributions::Alphanumeric;
use reqwest;

#[derive(StructOpt)]
struct Cli {
    /// The input torrent file
    #[structopt(parse(from_os_str))]
    path: std::path::PathBuf,
}

fn to_byte_string(i: &str) -> Value {
    let bytes: Vec<u8> = i.chars().map(|x| x as u8).collect();
    Value::ByteString(bytes.len().try_into().unwrap(), bytes)
}

fn from_byte_string(i: &Value) -> Result<String> {
    if let Value::ByteString(_, s) = i {
        return Ok(String::from_utf8_lossy(&s).to_string())
    }
    bail!("Expected a ByteString, found {:?}", i);
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

    let torrent_file = parse_torrent_file(&mut reader.bytes())
        .with_context(|| format!("failed to parse torrent file: {:#?}", &args.path))?;


    // Generate a random 20 byte ascii peer id 
    let peer_id: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(20)
        .map(char::from)
        .collect();   

    // Calculate the infohash (SHA-1 of the contents of the "info" dictionary)
    let mut hasher = Sha1::new();
    hasher.update(b"Hello World");
    let info_hash = hasher.finalize();

    let client = reqwest::blocking::Client::new();
    let res = client.post(&from_byte_string(&torrent_file[&to_byte_string("announce")])?)
        .query(&[("info_hash", "info_hash"), ("peer_id", "peer_id"), ("port", "6881"), ("uploaded", "0"), ("downloaded", "0")])
        .send()?;
    println!("tracker returned: {:?}", res);

    Ok(())
}
