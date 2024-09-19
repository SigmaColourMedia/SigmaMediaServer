use std::io::{BufReader, Read};

use byteorder::{BigEndian, ReadBytesExt};

/**
https://datatracker.ietf.org/doc/html/rfc3550#section-5.1
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|V=2|P|X|  CC   |M|     PT      |       sequence number         |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                           timestamp                           |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|           synchronization source (SSRC) identifier            |
+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+
|            contributing source (CSRC) identifiers             |
|                             ....                              |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
*/

#[derive(Debug, Clone)]
pub struct RTPPacket {
    pub marker: bool,
    version: u8,
    padding: bool,
    extension: bool,
    csrc_count: u8,
    payload_type: u8,
    pub sequence_number: u16,
    pub timestamp: u32,
    ssrc: u32,
    csrc: Vec<u32>,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum ParseError {
    PacketShort,
    MalformedPacket,
}

impl TryFrom<&[u8]> for RTPPacket {
    type Error = ParseError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let mut reader = BufReader::new(value);
        let first_octet = reader.read_u8().map_err(|_| Self::Error::PacketShort)?;
        let version = (first_octet & 0b1100_0000) >> 6;
        let is_padding_set = (first_octet & 0b0010_0000) == 0b0010_0000;
        let is_extension_set = (first_octet & 0b0001_0000) == 0b0001_0000;
        let csrc_count = first_octet & 0b0000_1111;

        let second_octet = reader.read_u8().map_err(|_| Self::Error::PacketShort)?;
        let marker = (second_octet & 0b1000_0000) == 0b1000_0000;
        let payload_type = second_octet & 0b0111_1111;

        let sequence_number = reader
            .read_u16::<BigEndian>()
            .map_err(|_| Self::Error::PacketShort)?;
        let timestamp = reader
            .read_u32::<BigEndian>()
            .map_err(|_| Self::Error::PacketShort)?;
        let ssrc = reader
            .read_u32::<BigEndian>()
            .map_err(|_| Self::Error::PacketShort)?;

        let csrc = (0..csrc_count.clone())
            .map(|_| {
                reader
                    .read_u32::<BigEndian>()
                    .map_err(|_| Self::Error::PacketShort)
            })
            .collect::<Result<Vec<u32>, ParseError>>()?;

        let mut payload_buffer = [0u8; 3000];
        let bytes_read = reader
            .read(&mut payload_buffer)
            .map_err(|_| Self::Error::MalformedPacket)?;

        let payload = Vec::from(&payload_buffer[..bytes_read]);

        Ok(Self {
            marker,
            version,
            padding: is_padding_set,
            extension: is_extension_set,
            csrc_count,
            csrc,
            payload_type,
            sequence_number,
            ssrc,
            timestamp,
            payload,
        })
    }
}
