use flate2::read::GzDecoder;
use log::{debug, info, trace, warn};
use serde::{Deserialize, Serialize};
// use serde_json::{Result, Value};
use std::fs::File;
use std::io::prelude::*;
use tar::Archive;

#[derive(Debug)]
pub enum ParseError {
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

#[derive(Deserialize, Debug)]
pub struct Version {
    format: String,
    version: i32,
}

#[derive(Debug)]
pub struct Manifest {
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
pub struct ArtifactDepends {
    artifact_name: Option<Vec<String>>,
    device_type: Vec<String>,
    artifact_group: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct ArtifactProvides {
    artifact_name: String,
    artifact_group: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct HeaderInfo {
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
pub struct TypeInfo {
    r#type: String,
    artifact_provides: Option<TypeInfoArtifactProvides>,
    artifact_depends: Option<TypeInfoArtifactDepends>,
}

#[derive(Deserialize, Debug)]
pub struct TypeInfoArtifactProvides {
    rootfs_image_checksum: String,
}

#[derive(Deserialize, Debug)]
pub struct TypeInfoArtifactDepends {
    rootfs_image_checksum: String,
}

#[derive(Deserialize, Debug)]
pub struct SubHeader {
    type_info: TypeInfo,
    meta_data: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug)]
pub struct Header {
    header_info: HeaderInfo,
    // Scripts are ignored for now
    headers: Vec<SubHeader>,
}

impl Header {
    fn parse<R>(tar: R) -> std::result::Result<Header, ParseError>
    where
        R: Read + Sized,
    {
        let mut archive = Archive::new(tar);
        let mut entries = archive.entries()?.peekable();
        let entry = entries
            .next()
            .expect("Failed to get the header-info from header.tar")
            .unwrap();
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
            let type_info: TypeInfo =
                serde_json::from_reader(entry_owned).expect("failed to parse the type-info");
            // TODO -- Parse metadata
            let subheader: SubHeader = SubHeader {
                type_info: type_info,
                meta_data: None,
            };
            header.headers.push(subheader);
        }
        Ok(header)
    }
}

pub struct Payload<'a> {
    name: String,
    reader: flate2::read::GzDecoder<tar::Entry<'a, &'a mut std::fs::File>>,
}

// impl Read for Payload {
//     fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
//         return self.reader.read(buf);
//     }
// }

pub struct MenderArtifact<'a> {
    version: Option<Version>,
    manifest: Option<Manifest>,
    header: Option<Header>,
    archive: tar::Archive<&'a mut std::io::Read>,
}

impl<'a> MenderArtifact<'a> {
    pub fn new(reader: &'a mut std::io::Read) -> MenderArtifact {
        let mut archive = Archive::new(reader);
        let mut m = MenderArtifact {
            archive: archive,
            version: None,
            manifest: None,
            header: None,
        };
        m
    }

    pub fn parse(
        &'a mut self,
        filename: &str,
    ) -> Result<tar::Entries<'a, &'a mut std::io::Read>, ParseError> {
        let mut entries = self.archive.entries().unwrap();

        let mut entry: tar::Entry<'a, &'a mut std::io::Read> = entries.next().unwrap().unwrap();
        // Check that the entry base path name is the same as the one we are expecting
        let path = entry.header().path().expect("Failed to get the header");
        if !path.ends_with("version") {
            return Err(ParseError::ParseError(String::from("Unexpected header")));
        }


        let version: Version =
            serde_json::from_reader(entry).expect("Failed to parse the Version header");
        println!("{:?}", version);


        entry = entries.next().unwrap().unwrap();
        // Check that the entry base path name is the same as the one we are expecting
        let path = entry.header().path().expect("Failed to get the header");
        if !path.ends_with("manifest") {
            return Err(ParseError::ParseError(String::from("Unexpected header")));
        }


        let manifest = Manifest::parse(entry);
        println!("{:?}", manifest);

        entry = entries.next().unwrap().unwrap();
        // Check that the entry base path name is the same as the one we are expecting
        let path = entry.header().path().expect("Failed to get the header");
        if !path.ends_with("header.tar.gz") {
            return Err(ParseError::ParseError(String::from("Unexpected header")));
        }


        let tar = GzDecoder::new(entry);
        let header = Header::parse(tar).expect("Failed to parse the `header.tar`");
        println!("{:?}", header);

        // TODO -- Wrap the remaining entries in a `Payload` which can be read from, and later,
        // checks the checksum for the payload once it is finished.
        // entry = entries.next().unwrap().unwrap();
        // // Check that the entry base path name is the same as the one we are expecting
        // let path = entry.header().path().expect("Failed to get the header");
        // if !path.starts_with("data") {
        //     return Err(ParseError::ParseError(String::from("Unexpected header")));
        // }

        return Ok(entries); // Return the paylaods
    }

}
