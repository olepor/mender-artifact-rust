use flate2::read::GzDecoder;
use log::{debug, info, trace, warn};
use serde::{Deserialize, Serialize};
// use serde_json::{Result, Value};
use std::fs::File;
use std::io::prelude::*;
use tar::Archive;


#[derive(Debug)]
enum ParseError {
    IOError(std::io::Error),
    ParseError(String),
    JSONParseError(serde_json::Error),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SuperError is here!")
    }
}

impl std::error::Error for ParseError {
    fn description(&self) -> &str {
        match &self {
            &IOError => "IOError",
            &ParseError => "ParseError",
            &JSONParseError => "JSONParrseError",
        }
    }

    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self {
            &IOError => None,
            &ParseError => None,
            &JSONParseError => None,
        }
    }
}
impl From<std::io::Error> for ParseError {
    fn from(error: std::io::Error) -> Self {
        ParseError::IOError(error)
    }
}

impl From<serde_json::Error> for ParseError {
    fn from(error: serde_json::Error) -> Self {
        ParseError::JSONParseError(error)
    }
}


// impl From<> for ParseError {
//     fn from(error: std::io::Error) -> Self {
//         ParseError::IOError(error)
//     }
// }


#[derive(Deserialize, Debug)]
struct Version {
    format: String,
    version: i32,
}

#[derive(Debug)]
struct Manifest {
    data: std::collections::HashMap<String, String>, // Name : Hash
}

use std::io::{BufRead, BufReader};
impl Manifest {
    fn parse<R>(r: R) -> std::io::Result<Manifest>
    where
        R: Read,
    {
        let mut m = Manifest {
            data: std::collections::HashMap::new(),
        };
        for line in BufReader::new(r).lines() {
            let line = line?.clone();
            debug!("Manifest line: {}", line);
            let mut line_it = line.split_ascii_whitespace();
            m.data.insert(
                line_it.next().unwrap().to_string(),
                line_it.next().unwrap().to_string(),
            );
        }
        Ok(m)
    }
}

#[derive(Deserialize, Debug)]
struct ArtifactDepends {
    artifact_name: Option<Vec<String>>,
    device_type: Vec<String>,
    artifact_group: Option<String>,
}


#[derive(Deserialize, Debug)]
struct ArtifactProvides {
    artifact_name: String,
    artifact_group: Option<String>,
}

#[derive(Deserialize, Debug)]
struct HeaderInfo {
    payloads: Vec<std::collections::HashMap<String, String>>,
    artifact_provides: ArtifactProvides,
    artifact_depends: ArtifactDepends,
}

// {
//     "type": "rootfs-image"
//         "artifact_provides": {
//             "rootfs_image_checksum": "4d480539cdb23a4aee6330ff80673a5af92b7793eb1c57c4694532f96383b619"
//         },
//     "artifact_depends": {
//         "rootfs_image_checksum": "4d480539cdb23a4aee6330ff80673a5af92b7793eb1c57c4694532f96383b619"
//     },
// }
#[derive(Deserialize, Debug)]
struct TypeInfo {
    r#type: String,
    artifact_provides: Option<TypeInfoArtifactProvides>,
    artifact_depends: Option<TypeInfoArtifactDepends>,
}

#[derive(Deserialize, Debug)]
struct TypeInfoArtifactProvides {
    rootfs_image_checksum: String,
}

#[derive(Deserialize, Debug)]
struct TypeInfoArtifactDepends {
    rootfs_image_checksum: String,
}

#[derive(Deserialize, Debug)]
struct SubHeader {
    type_info: TypeInfo,
    meta_data: Option<std::collections::HashMap<String, String>>,
}

struct Header {
    header_info: HeaderInfo,
    // Scripts are ignored for now
    headers: Vec<SubHeader>,
}

impl Header {
    fn parse<R>(tar: R) -> std::result::Result<Header, ParseError> where R: Read + Sized {
        let mut archive = Archive::new(tar);
        let mut entries = archive.entries()?.peekable();
        let entry = entries
            .next()
            .expect("Failed to get the header-info from header.tar").unwrap();
        let hdr_info = entry.header();
        println!("Header Info: {:?}", hdr_info);
        let header_info: HeaderInfo =
            serde_json::from_reader(entry).expect("Failed to parse the header-info json");
        let mut header = Header {
            header_info: header_info,
            headers: Vec::new(),
        };
        // Parse all the headers
        for entry in entries {
            let entry_owned = entry.expect("Failed to get the tar header");
            let t_hdr = entry_owned.header();
            println!("subheader: {:?}", t_hdr);
            if entry_owned.path()?.starts_with("scripts") {
                continue; // Discard scripts
            }
            // First entry should be the type-info
            let type_info: TypeInfo = serde_json::from_reader(entry_owned).expect("failed to parse the type-info");
            // TODO -- Parse metadata
            let subheader: SubHeader =
                SubHeader{type_info: type_info, meta_data: None};
            header.headers.push(subheader);
        }
        Ok(header)
    }
}

fn main() {
    simple_logger::init_with_level(log::Level::Debug).unwrap();
    debug!("Starting Mender artifact...");

    let file = File::open("mender-demo-artifact.mender").expect("Failed to open file");
    // let mut zip = zip::ZipArchive::new(file).expect("Failed to unzip the file");
    // let zipfile = zip.by_index(0).unwrap();
    let mut a = Archive::new(file);
    let mut entries = a.entries().unwrap();

    // let mut parser = Parser::new("foo.tar");
    // Expect Version
    let mut entry = entries.next().unwrap().unwrap();
    let header_info = entry.header();
    println!("{:?}", header_info);

    let version: Version =
        serde_json::from_reader(entry).expect("Failed to parse the Version header");
    println!("{:?}", version);

    entry = entries.next().unwrap().unwrap();
    println!("{:?}", entry.header());

    let manifest = Manifest::parse(entry);

    entry = entries.next().unwrap().unwrap();
    let header_info = entry.header();
    println!("Header info: {:?}", header_info);
    // Expect `header.tar.gz`
    // assert_eq!(header_info.path()?.to_string(), "header.tar.gz");
    // Unzip the header
    let tar = GzDecoder::new(entry);
    let header = Header::parse(tar).expect("Failed to parse the `header.tar`");
    // let manifest: Manifest = serde_json::from_reader(entry).expect("Failed to read manifest");
    // for file in a.entries().unwrap() {
    //     // Make sure there wasn't an I/O error
    //     let mut file = file.unwrap();

    //     // Inspect metadata about the file
    //     println!("{:?}", file.header().path().unwrap());
    //     println!("{}", file.header().size().unwrap());

    //     // files implement the Read trait
    //     let mut s = String::new();
    //     file.read_to_string(&mut s).unwrap();
    //     println!("{}", s);
    // }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_read_version() {
        assert_eq!(1, 1);
    }
}
