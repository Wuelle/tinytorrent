use anyhow::{anyhow, ensure, Context, Result};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use reqwest;
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use sha1::{Digest, Sha1};
use std::io::{BufReader, Read};
use structopt::StructOpt;

#[derive(StructOpt)]
struct Cli {
    #[structopt(parse(from_os_str))]
    path: std::path::PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
struct Node(String, i64);

#[derive(Debug, Serialize, Deserialize)]
#[allow(dead_code)]
struct File {
    path: Vec<String>,
    length: i64,
    #[serde(default)]
    md5sum: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(dead_code)]
struct Info {
    name: String,
    pieces: ByteBuf,
    #[serde(rename = "piece length")]
    piece_length: i64,
    #[serde(default)]
    md5sum: Option<String>,
    #[serde(default)]
    length: Option<i64>,
    #[serde(default)]
    files: Option<Vec<File>>,
    #[serde(default)]
    private: Option<u8>,
    #[serde(default)]
    path: Option<Vec<String>>,
    #[serde(default)]
    #[serde(rename = "root hash")]
    root_hash: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(dead_code)]
struct Torrent {
    info: Info,
    #[serde(default)]
    announce: Option<String>,
    #[serde(default)]
    nodes: Option<Vec<Node>>,
    #[serde(default)]
    encoding: Option<String>,
    #[serde(default)]
    httpseeds: Option<Vec<String>>,
    #[serde(default)]
    #[serde(rename = "announce-list")]
    announce_list: Option<Vec<Vec<String>>>,
    #[serde(default)]
    #[serde(rename = "creation date")]
    creation_date: Option<i64>,
    #[serde(default)]
    comment: Option<String>,
    #[serde(default)]
    #[serde(rename = "created by")]
    created_by: Option<String>,
}

fn main() -> Result<()> {
    let args = Cli::from_args();
    ensure!(
        args.path.extension().is_some() && args.path.extension().unwrap() == "torrent",
        format!("{:#?} is not a torrent (.torrent) file", &args.path)
    );
    let f = std::fs::File::open(&args.path)
        .with_context(|| format!("could not open file {:#?}", &args.path))?;
    let mut reader = BufReader::new(f);
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer)?;
    println!("{} Bytes", buffer.len());

    let torrent: Torrent = serde_bencode::from_bytes(&buffer)
        .with_context(|| format!("failed to parse torrent file: {:#?}", &args.path))?;

    // Generate a random 20 byte ascii peer id
    let peer_id: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(20)
        .map(char::from)
        .collect();

    // Calculate the infohash (SHA-1 of the contents of the "info" dictionary)
    let mut hasher = Sha1::new();
    hasher.update(serde_bencode::to_bytes(&torrent.info)?);
    let info_hash = hex::encode(hasher.finalize());

    // Make an initial request to the tracker to get the peers
    let client = reqwest::blocking::Client::new();
    let tracker_url = torrent
        .announce
        .ok_or(anyhow!("Expected value for 'announce'"))?;
    println!("connecting to {}", tracker_url);
    let res = client
        .get(tracker_url)
        .query(&[
            ("info_hash", info_hash.as_str()),
            ("peer_id", &peer_id),
            ("event", "started"),
            ("port", "6881"),
            ("uploaded", "0"),
            ("downloaded", "0"),
            (
                "left",
                &torrent
                    .info
                    .length
                    .ok_or(anyhow!("Expected value for 'info.length'"))?
                    .to_string(),
            ),
            ("numwant", "50"),
        ])
        .send()?;

    println!("tracker returned Code {}: {:?}", res.status(), res.text());

    Ok(())
}
