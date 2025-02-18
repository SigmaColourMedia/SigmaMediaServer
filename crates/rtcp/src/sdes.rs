use std::f32::consts::E;
use std::os::unix::raw::mode_t;
use byteorder::ReadBytesExt;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use crate::header::{Header, PayloadType};
use crate::{Marshall, MarshallError, Unmarshall, UnmarshallError};

#[derive(Debug, Clone, PartialEq)]
pub struct SourceDescriptor {
    header: Header,
    chunks: Vec<Chunk>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    ssrc: u32,
    items: Vec<SDES>,
}

// impl SourceDescriptor {
//     pub fn new(chunks: Vec<Chunk>) {
//         let header = Header {
//             payload_type: PayloadType::SDES,
//             length: 10,
//             padding: false,
//             feedback_message_type: chunks.len() as u8,
//         };
//     }
// }

impl Unmarshall for SourceDescriptor {
    fn unmarshall(bytes: Bytes) -> Result<Self, UnmarshallError>
    where
        Self: Sized,
    {
        let header = Header::unmarshall(bytes.clone())?;
        if bytes.len() < header.length as usize {
            return Err(UnmarshallError::InvalidLength);
        }

        let chunk_count = header.feedback_message_type;

        let mut chunks: Vec<Chunk> = vec![];
        let mut curr_item = bytes.slice(4..);

        for _ in 0..chunk_count {
            let mut items = vec![];
            let ssrc = curr_item.get_u32();
            loop {
                let item_type = curr_item.get_u8();
                match item_type {
                    0 => {
                        if curr_item.get_u8() != 0 {
                            return Err(UnmarshallError::UnexpectedFrame);
                        }
                        let empty_list_byte_len = 2;
                        let sdes_item_header_len = 2;

                        // Get all items length and move pointer by padding if any
                        let items_len = items.iter().map(|item| match item {
                            SDES::CName(cname) => {
                                cname.domain_name.len() + sdes_item_header_len
                            }
                        }).sum::<usize>() + empty_list_byte_len;
                        let modulo = items_len % 4;
                        curr_item = curr_item.slice(modulo..);
                        break;
                    }
                    1 => {
                        let length = curr_item.get_u8();
                        let payload = String::from_utf8_lossy(&curr_item.slice(..length as usize)).to_string();
                        // Advance pointer by payload length
                        curr_item = curr_item.slice(length as usize..);

                        items.push(SDES::CName(CNameSDES { domain_name: payload }))
                    }
                    _ => return Err(UnmarshallError::UnexpectedFrame)
                }
            }
            chunks.push(Chunk {
                items,
                ssrc,
            });
        };
        Ok(Self {
            header,
            chunks,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SDES {
    CName(CNameSDES)
}

impl Marshall for SDES {
    fn marshall(self) -> Result<Bytes, MarshallError>
    where
        Self: Sized,
    {
        match self { SDES::CName(cname) => cname.marshall() }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CNameSDES {
    domain_name: String,
}

impl Marshall for CNameSDES {
    fn marshall(self) -> Result<Bytes, MarshallError>
    where
        Self: Sized,
    {
        if self.domain_name.len() > 255 {
            return Err(MarshallError::InvalidLength);
        }

        let mut bytes = BytesMut::new();
        bytes.put_u8(CNAME_CODE);
        bytes.put_u8(self.domain_name.len() as u8);
        bytes.put(self.domain_name.as_bytes());
        Ok(bytes.freeze())
    }
}

static CNAME_CODE: u8 = 1;


#[cfg(test)]
mod marshall_cname {
    use bytes::Bytes;
    use crate::Marshall;
    use crate::sdes::{CNameSDES};

    #[test]
    fn marshall_ok() {
        let input = CNameSDES {
            domain_name: "smid".to_string()
        };

        assert_eq!(input.marshall().unwrap(), Bytes::from_static(&[
            1, // Type CNAME
            4, // 4 8-bit word length
            115, 109, 105, 100 // payload = "smid"
        ]))
    }
}

#[cfg(test)]
mod unmarshall_sdes {
    use bytes::Bytes;
    use crate::header::{Header, PayloadType};
    use crate::sdes::{Chunk, CNameSDES, SDES, SourceDescriptor};
    use crate::Unmarshall;

    #[test]
    fn sdes_one_item_ok() {
        let input = Bytes::from_static(&[
            129, 202, 0, 6, // Header, len=6, 1 chunk
            29, 71, 245, 255, // Sender SSRC = 491255295
            1, 16, 120, 53, // CNAME=1, len=16, payload=
            114, 51, 52, 49,
            51, 81, 102, 122,
            68, 86, 79, 84,
            79, 56, 0, 0]);

        let output = SourceDescriptor::unmarshall(input).unwrap();

        assert_eq!(output, SourceDescriptor {
            header: Header {
                padding: false,
                length: 6,
                feedback_message_type: 1,
                payload_type: PayloadType::SDES,
            },
            chunks: vec![Chunk {
                ssrc: 491255295,
                items: vec![SDES::CName(CNameSDES { domain_name: "x5r3413QfzDVOTO8".to_string() })],
            }],
        })
    }
}