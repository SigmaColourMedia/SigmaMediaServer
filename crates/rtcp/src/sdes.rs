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

impl SourceDescriptor {
    pub fn new(chunks: Vec<Chunk>) -> Self {
        let chunks_len = chunks.clone().into_iter().map(|chunk| chunk.marshall().unwrap().len()).sum::<usize>();
        if chunks_len % 4 != 0 {
            panic!("Chunks should match 32-bit word boundaries")
        }
        let header = Header {
            payload_type: PayloadType::SDES,
            length: (chunks_len / 4) as u16,
            padding: false,
            feedback_message_type: 1,
        };

        SourceDescriptor {
            chunks,
            header,
        }
    }
}

impl Marshall for SourceDescriptor {
    fn marshall(self) -> Result<Bytes, MarshallError>
    where
        Self: Sized,
    {
        let mut bytes = BytesMut::new();
        bytes.put(self.header.marshall()?);
        for chunk in self.chunks {
            bytes.put(chunk.marshall()?)
        }
        Ok(bytes.freeze())
    }
}

impl Marshall for Chunk {
    fn marshall(self) -> Result<Bytes, MarshallError>
    where
        Self: Sized,
    {
        let mut bytes = BytesMut::new();
        bytes.put_u32(self.ssrc);
        for item in self.items {
            bytes.put(item.marshall()?)
        }

        let extra_padding = bytes.len() % 4;
        if extra_padding > 0 {
            bytes.put_bytes(0, extra_padding)
        }
        Ok(bytes.freeze())
    }
}

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
mod source_descriptor_new_constructor {
    use crate::header::{Header, PayloadType};
    use crate::sdes::{Chunk, CNameSDES, SDES, SourceDescriptor};

    #[test]
    fn constructs_source_descriptor_with_one_chunk() {
        let chunks = vec![
            Chunk {
                ssrc: 1,
                items: vec![SDES::CName(CNameSDES {
                    domain_name: "smid".to_string()
                })],
            }
        ];
        let output = SourceDescriptor::new(chunks.clone());

        assert_eq!(output, SourceDescriptor {
            header: Header {
                padding: false,
                length: 3,
                payload_type: PayloadType::SDES,
                feedback_message_type: 1,
            },
            chunks,
        })
    }

    #[test]
    fn constructs_source_descriptor_with_one_chunk_and_multiple_items() {
        let chunks = vec![
            Chunk {
                ssrc: 1,
                items: vec![
                    SDES::CName(CNameSDES {
                        domain_name: "smid".to_string()
                    }),
                    SDES::CName(CNameSDES {
                        domain_name: "test".to_string()
                    }),
                ],
            }
        ];
        let output = SourceDescriptor::new(chunks.clone());

        assert_eq!(output, SourceDescriptor {
            header: Header {
                padding: false,
                length: 4,
                payload_type: PayloadType::SDES,
                feedback_message_type: 1,
            },
            chunks,
        })
    }

    #[test]
    fn constructs_source_descriptor_with_two_chunks() {
        let chunks = vec![
            Chunk {
                ssrc: 1,
                items: vec![SDES::CName(CNameSDES {
                    domain_name: "smid".to_string()
                })],
            },
            Chunk {
                ssrc: 2,
                items: vec![SDES::CName(CNameSDES {
                    domain_name: "test".to_string()
                })],
            },
        ];
        let output = SourceDescriptor::new(chunks.clone());

        assert_eq!(output, SourceDescriptor {
            header: Header {
                padding: false,
                length: 6,
                payload_type: PayloadType::SDES,
                feedback_message_type: 1,
            },
            chunks,
        })
    }
}

#[cfg(test)]
mod marshall_sdes {
    use bytes::Bytes;
    use crate::header::{Header, PayloadType};
    use crate::Marshall;
    use crate::sdes::{Chunk, CNameSDES, SDES, SourceDescriptor};

    #[test]
    fn marshall_single_chunk_sdes() {
        let sdes = SourceDescriptor {
            header: Header {
                payload_type: PayloadType::SDES,
                length: 2,
                feedback_message_type: 1,
                padding: false,
            },
            chunks: vec![Chunk {
                ssrc: 1,
                items: vec![SDES::CName(CNameSDES {
                    domain_name: "sm".to_string()
                })],
            }],
        };

        assert_eq!(sdes.marshall().unwrap(), Bytes::from_static(&[
            129, 202, 0, 2, // SDES header, len = 2
            // First chunk
            0, 0, 0, 1, // SSRC = 1
            1, 2, 115, 109 // SDES CNAME, len = 2, domain = "sm"
        ]));
    }

    #[test]
    fn marshall_multiple_items_chunk_sdes() {
        let sdes = SourceDescriptor {
            header: Header {
                payload_type: PayloadType::SDES,
                length: 4,
                feedback_message_type: 1,
                padding: false,
            },
            chunks: vec![Chunk {
                ssrc: 1,
                items: vec![
                    SDES::CName(CNameSDES {
                        domain_name: "sm".to_string()
                    }),
                    SDES::CName(CNameSDES {
                        domain_name: "test".to_string()
                    }),
                ],
            }],
        };

        assert_eq!(sdes.marshall().unwrap(), Bytes::from_static(&[
            129, 202, 0, 4, // SDES header, len = 2
            // First chunk
            0, 0, 0, 1, // SSRC = 1
            1, 2, 115, 109, // SDES CNAME, len = 2, domain = "sm"
            1, 4, 116, 101, // SDES CNAME, len = 2, domain = "test"
            115, 116, 0, 0
        ]));
    }

    #[test]
    fn marshall_chunk_with_multiple_padded_items() {
        let sdes = SourceDescriptor {
            header: Header {
                payload_type: PayloadType::SDES,
                length: 4,
                feedback_message_type: 1,
                padding: false,
            },
            chunks: vec![Chunk {
                ssrc: 1,
                items: vec![
                    SDES::CName(CNameSDES {
                        domain_name: "test".to_string()
                    }),
                    SDES::CName(CNameSDES {
                        domain_name: "sm".to_string()
                    }),
                ],
            }],
        };

        assert_eq!(sdes.marshall().unwrap(), Bytes::from_static(&[
            129, 202, 0, 4, // SDES header, len = 2
            // First chunk
            0, 0, 0, 1, // SSRC = 1
            1, 4, 116, 101, // SDES CNAME, len = 2, domain = "test"
            115, 116, 1, 2, // domain = "st", SDES CNAME, len = 4
            115, 109, 0, 0 // SDES CNAME, len = 2, domain = "sm"
        ]));
    }

    #[test]
    fn marshall_single_chunk_with_one_item_padded() {
        let sdes = SourceDescriptor {
            header: Header {
                payload_type: PayloadType::SDES,
                length: 3,
                feedback_message_type: 1,
                padding: false,
            },
            chunks: vec![Chunk {
                ssrc: 1,
                items: vec![SDES::CName(CNameSDES {
                    domain_name: "smid".to_string()
                })],
            }],
        };

        assert_eq!(sdes.marshall().unwrap(), Bytes::from_static(&[
            129, 202, 0, 3, // SDES header, len = 2
            // First chunk
            0, 0, 0, 1, // SSRC = 1
            1, 4, 115, 109, // SDES CNAME, len = 2, domain = "smid"
            105, 100, 0, 0 // 2 bytes padding
        ]));
    }

    #[test]
    fn marshall_two_chunk_sdes_with_items_padded() {
        let sdes = SourceDescriptor {
            header: Header {
                payload_type: PayloadType::SDES,
                length: 3,
                feedback_message_type: 1,
                padding: false,
            },
            chunks: vec![
                Chunk {
                    ssrc: 1,
                    items: vec![SDES::CName(CNameSDES {
                        domain_name: "smid".to_string()
                    })],
                },
                Chunk {
                    ssrc: 2,
                    items: vec![SDES::CName(CNameSDES {
                        domain_name: "test".to_string()
                    })],
                }],
        };

        assert_eq!(sdes.marshall().unwrap(), Bytes::from_static(&[
            129, 202, 0, 3, // SDES header, len = 2
            // First chunk
            0, 0, 0, 1, // SSRC = 1
            1, 4, 115, 109, // SDES CNAME, len = 2, domain = "smid"
            105, 100, 0, 0, // 2 bytes padding
            // Second chunk
            0, 0, 0, 2, // SSRC = 2
            1, 4, 116, 101, // SDES CNAME, len = 2, domain = "test"
            115, 116, 0, 0 // 2 bytes padding
        ]));
    }
}

#[cfg(test)]
mod marshall_chunk {
    use bytes::Bytes;
    use crate::Marshall;
    use crate::sdes::{Chunk, CNameSDES, SDES};

    #[test]
    fn marshall_chunk_with_two_padding_bytes() {
        let chunk = Chunk {
            ssrc: 1,
            items: vec![SDES::CName(CNameSDES { domain_name: "smid".to_string() })],
        };

        assert_eq!(chunk.marshall().unwrap(), Bytes::from_static(&[
            0, 0, 0, 1, // SSRC = 1
            1, 4, 115, 109, // CNAME, len = 4, domain = "smid"
            105, 100, 0, 0 // 2 padding bytes
        ]))
    }

    #[test]
    fn marshall_chunk_with_no_padding_bytes() {
        let chunk = Chunk {
            ssrc: 1,
            items: vec![SDES::CName(CNameSDES { domain_name: "sm".to_string() })],
        };

        assert_eq!(chunk.marshall().unwrap(), Bytes::from_static(&[
            0, 0, 0, 1, // SSRC = 1
            1, 2, 115, 109, // CNAME, len = 2, domain = "sm"
        ]))
    }
}


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