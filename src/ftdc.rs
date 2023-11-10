use std::collections::HashMap;
use std::io::{Cursor, Read, Seek, SeekFrom};

use bson::document::ValueAccessError;
use bson::spec::BinarySubtype;
use bson::{Binary, Bson, Document};
use flate2::bufread::ZlibDecoder;
use lebe::io::ReadEndian;

mod decode;
mod error;

use crate::metric::{MetricKey, Timestamp};

use self::decode::MetricsDecoder;
pub use self::error::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Chunk {
    Metadata(Document),
    Data(MetricsChunk),
}

#[derive(Debug)]
pub struct MetricsChunk {
    pub timestamps: Vec<Timestamp>,
    pub metrics: HashMap<MetricKey, Vec<i64>>,
}

pub fn read_chunk<R: Read>(reader: &mut R) -> Result<Chunk> {
    let chunk_buf = {
        let len = read_chunk_len(reader)?;
        let mut buf = vec![0u8; len as _];
        buf[0..4].copy_from_slice(&u32::to_le_bytes(len));
        reader.read_exact(&mut buf[4..])?;
        buf
    };
    let chunk_doc = Document::from_reader(&mut chunk_buf.as_slice())?;
    match chunk_doc.get_i32("type")? {
        0 => extract_metadata(chunk_doc),
        1 => extract_data(chunk_doc),
        unk => Err(Error::UnknownChunkType(unk)),
    }
}

pub fn skip_chunk<R: Read + Seek>(reader: &mut R) -> Result<()> {
    let len = read_chunk_len(reader)?;
    reader.seek(SeekFrom::Current((len - 4) as i64))?;
    Ok(())
}

fn read_chunk_len<R: Read>(reader: &mut R) -> Result<u32> {
    match reader.read_from_little_endian() {
        Ok(len) => Ok(len),
        Err(err) => match err.kind() {
            std::io::ErrorKind::UnexpectedEof => Err(Error::EOF),
            _ => Err(Error::from(err)),
        },
    }
}

fn extract_metadata(mut doc: Document) -> Result<Chunk> {
    match doc.remove("doc") {
        Some(Bson::Document(doc)) => Ok(Chunk::Metadata(doc)),
        Some(_) => Err(Error::InvalidDocumentFormat(
            ValueAccessError::UnexpectedType,
        )),
        None => Err(Error::InvalidDocumentFormat(ValueAccessError::NotPresent)),
    }
}

fn extract_data(mut doc: Document) -> Result<Chunk> {
    let compressed = match doc.remove("data") {
        Some(Bson::Binary(Binary { subtype: BinarySubtype::Generic, bytes })) => bytes,
        Some(_) => {
            return Err(Error::InvalidDocumentFormat(
                ValueAccessError::UnexpectedType,
            ))
        }
        None => return Err(Error::InvalidDocumentFormat(ValueAccessError::NotPresent)),
    };

    let uncompressed_len: u32 = Cursor::new(compressed.as_slice()).read_from_little_endian()?;
    let mut uncompressed = vec![0; uncompressed_len as _];
    ZlibDecoder::new(&compressed[4..]).read_exact(&mut uncompressed)?;

    let doc = Document::from_reader(uncompressed.as_slice())?;

    let mut cursor = Cursor::new(uncompressed.as_slice());

    let doc_len: u32 = cursor.read_from_little_endian()?;
    cursor.seek(SeekFrom::Start(doc_len as _))?;

    let num_keys: u32 = cursor.read_from_little_endian()?;
    let num_deltas: u32 = cursor.read_from_little_endian()?;

    let mut decoder = MetricsDecoder::new(num_keys as usize, num_deltas as usize);
    decoder.collect_metrics(doc);
    decoder.decode_deltas(&mut cursor)?;

    Ok(Chunk::Data(decoder.finish()))
}
