mod parser;

use crate::parser::*;
use anyhow::{ensure, Context, Result};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use reqwest;
use sha1::{Digest, Sha1};
use std::fs::File;
use std::io::{BufReader, Read};
use structopt::StructOpt;

#[derive(StructOpt)]
struct Cli {
    /// The input torrent file
    #[structopt(parse(from_os_str))]
    path: std::path::PathBuf,
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
    let info_dir: Vec<u8> = (&torrent_file[&Value::from("info")]).into();
    let mut hasher = Sha1::new();
    hasher.update(&info_dir);
    let info_hash = hex::encode(hasher.finalize());

    println!("{}", info_hash);
    println!("974668f694948d065530cdfedb1eabfeb32f2bc7");
    // let client = reqwest::blocking::Client::new();

    // // TODO: make this part prettier
    // let tracker_url = String::from(&torrent_file[&Value::from("announce")]);
    // let res = client
    //     .post(tracker_url.split_once(':').unwrap().1)
    //     .query(&[
    //         (
    //             "info_hash",
    //             info_hash.as_str()
    //         ),
    //         ("peer_id", &peer_id),
    //         ("event", "started"),
    //         ("port", "6881"),
    //         ("uploaded", "0"),
    //         ("downloaded", "0"),
    //     ])
    //     .send()?;

    // println!("tracker returned Code {}: {:?}", res.status(), res.text());

    Ok(())
}
