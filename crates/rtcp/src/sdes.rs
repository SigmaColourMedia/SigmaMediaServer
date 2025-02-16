use byteorder::ReadBytesExt;
use bytes::{Buf, Bytes};
use crate::header::Header;
use crate::{Marshall, Unmarshall, UnmarshallError};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SourceDescriptor {
    header: Header,
    items: Vec<SDES>,
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

        let sdes_count = header.feedback_message_type;

        let mut items = vec![];
        let mut curr_item = bytes.slice(8..);
        for _ in 0..sdes_count {
            let item_type = curr_item.get_u8();
            let length = curr_item.get_u8();
            let payload = String::from_utf8_lossy(&curr_item.slice(..length as usize)).to_string();

            match item_type {
                1 => {
                    items.push(SDES::CName(CNameSDES { domain_name: payload }))
                }
                _ => {
                    return Err(UnmarshallError::UnexpectedFrame)
                }
            }
            // Move curr_item cursor by length + padding
            let padding = (length - 2) % 4;
            curr_item = curr_item.slice((length + padding) as usize..)
        };
        Ok(Self {
            header,
            items,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
enum SDES {
    CName(CNameSDES)
}

#[derive(Debug, Clone, PartialEq)]
struct CNameSDES {
    domain_name: String,
}


#[cfg(test)]
mod unmarshall_sdes {
    use bytes::Bytes;
    use crate::sdes::SourceDescriptor;
    use crate::Unmarshall;

    #[test]
    fn sdes_one_item_ok() {
        let input = Bytes::from_static(&[
            129, 202, 0, 6, // Header, len=6
            29, 71, 245, 255, // Sender SSRC = 491255295
            1, 16, 120, 53, // CNAME=1, len=16, payload=
            114, 51, 52, 49,
            51, 81, 102, 122,
            68, 86, 79, 84,
            79, 56, 0, 0]);

        let output = SourceDescriptor::unmarshall(input).unwrap();

        println!("output {:?}", output)
    }
}