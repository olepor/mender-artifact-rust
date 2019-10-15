use flate2::read::GzDecoder;
use log::{debug, info, trace, warn};
use serde::{Deserialize, Serialize};
// use serde_json::{Result, Value};
use std::fs::File;
use std::io::prelude::*;
use tar::Archive;

mod lib;


fn main() {
    simple_logger::init_with_level(log::Level::Debug).unwrap();
    debug!("Starting Mender artifact...");

    let filepath = std::env::args().nth(1).expect("No artifact path given");
    let mut file = File::open(filepath).expect("Failed to open file");
    let mut ma = lib::MenderArtifact::new(&mut file);
    let mut payloads = ma.parse("booboo");


    let entry = payloads.unwrap().next().unwrap().unwrap();
    // Check that the entry base path name is the same as the one we are expecting
    let path = entry.header().path().expect("Failed to get the header");
    if !path.starts_with("data") {
        eprintln!("No data found in artifact");
    }

    // // Unzip the data
    // tar = GzDecoder::new(entry);
    // let payload = Payload {
    //     name: "foobar".to_string(),
    //     reader: Box::new(tar),
    // };
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_read_version() {
        assert_eq!(1, 1);
    }
}
