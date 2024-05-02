use std::io::{BufReader, BufWriter, Error, Read, Write};
use std::net::SocketAddr;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

use crate::ice_registry::{SessionCredentials, SessionUsername};

pub fn parse_stun_packet(packet: &[u8]) -> Option<StunBindingRequest> {
    if packet.len() < STUN_HEADER_LEN {
        return None;
    }
    let mut reader = BufReader::new(packet);
    let message_type = reader.read_u16::<BigEndian>().ok()?;
    if message_type != StunType::BindingRequest as u16 {
        return None;
    }

    let length = reader.read_u16::<BigEndian>().ok()?;
    if length % 4 != 0 || STUN_HEADER_LEN + length as usize > packet.len() {
        return None;
    }

    let magic_cookie = reader.read_u32::<BigEndian>().ok()?;
    if magic_cookie != STUN_COOKIE {
        return None;
    }

    let mut transaction_id = [0; 12];
    reader.read(&mut transaction_id).ok()?;


    let mut attributes: Vec<StunAttribute> = Vec::new();

    while let Ok(attribute_type_key) = reader.read_u16::<BigEndian>() {
        let attribute_type: StunAttributeType = match attribute_type_key {
            0x6 => StunAttributeType::Username,
            0x8 => StunAttributeType::MessageIntegrity,
            0x802a => StunAttributeType::IceControlling,
            0x25 => StunAttributeType::UseCandidate,
            _ => StunAttributeType::Unknown
        };

        let mut length = reader.read_u16::<BigEndian>().unwrap();
        length = pad_to_4bytes(length);
        let mut value_buffer: Vec<u8> = vec![0; length as usize];
        reader.read_exact(&mut value_buffer).unwrap();

        match attribute_type {
            StunAttributeType::Username => {
                let username_string = String::from_utf8(value_buffer).unwrap();
                let (host_username, remote_username) = username_string.split_once(":").unwrap();
                attributes.push(StunAttribute::Username(SessionUsername {
                    host: host_username.trim_end_matches(char::from(0)).to_owned(), // Remove null chars
                    remote: remote_username.trim_end_matches(char::from(0)).to_owned(),
                }))
            }
            StunAttributeType::MessageIntegrity => {
                let mut buffer: [u8; STUN_MESSAGE_INTEGRITY_LEN] = [0; STUN_MESSAGE_INTEGRITY_LEN];
                buffer.copy_from_slice(&value_buffer[..STUN_MESSAGE_INTEGRITY_LEN]);
                attributes.push(StunAttribute::MessageIntegrity(buffer));
            }
            StunAttributeType::IceControlling => {
                attributes.push(StunAttribute::IceControlling)
            }
            StunAttributeType::UseCandidate => {
                attributes.push(StunAttribute::UseCandidate)
            }
            StunAttributeType::Unknown => {
                attributes.push(StunAttribute::Unknown)
            }
            StunAttributeType::XORMappedAddress => {
                attributes.push(StunAttribute::Unknown)
            }
        }
    }

    return Some(StunBindingRequest {
        transaction_id,
        attributes,
    });
}


pub fn parse_binding_request(stun_message: StunBindingRequest) -> Option<ICEStunMessageType> {
    let message_integrity = stun_message.attributes.iter().find_map(|attr| match attr {
        StunAttribute::MessageIntegrity(integrity) => Some(*integrity),
        _ => None,
    })?;

    let nominate_flag = stun_message.attributes.iter().find_map(|attr| match attr {
        StunAttribute::UseCandidate => Some(()),
        _ => None,
    });
    let session_username = stun_message.attributes.into_iter().find_map(|attr| match attr {
        StunAttribute::Username(username_session) => Some(username_session),
        _ => None,
    })?;

    match nominate_flag {
        None => {
            Some(ICEStunMessageType::LiveCheck(ICEStunPacket {
                message_integrity,
                username_attribute: session_username,
                transaction_id: stun_message.transaction_id,
            }))
        }
        Some(_) => {
            Some(ICEStunMessageType::Nomination(ICEStunPacket {
                message_integrity,
                username_attribute: session_username,
                transaction_id: stun_message.transaction_id,
            }))
        }
    }
}

pub fn create_stun_success(credentials: &SessionCredentials, transaction_id: [u8; STUN_TRANSACTION_ID_LEN], remote: &SocketAddr, mut buffer: &mut [u8]) -> Result<usize, Error> {
    let mut writer = BufWriter::new(buffer);
    writer.write_u16::<BigEndian>(StunType::SuccessResponse as u16)?; // Success Response

    let xor_address_attr_length: usize = match remote {
        SocketAddr::V4(_) => 12,
        SocketAddr::V6(_) => 24
    };

    let message_length = xor_address_attr_length;
    writer.write_u16::<BigEndian>(message_length as u16)?;
    writer.write_u32::<BigEndian>(STUN_COOKIE)?;
    writer.write(&transaction_id)?;
    println!("{:?}", writer.buffer());

    compute_xor_mapped_address(remote, transaction_id)?;
    Ok(1)
}

fn compute_xor_mapped_address(remote: &SocketAddr, transaction_id: [u8; STUN_TRANSACTION_ID_LEN]) -> Result<Vec<u8>, Error> {
    let mut buffer = vec![];
    match remote {
        SocketAddr::V4(remote_addr) => {
            buffer.write_u16::<BigEndian>(StunAttributeType::XORMappedAddress as u16)?; // Type
            buffer.write_u16::<BigEndian>(8)?; // Length
            buffer.write_u8(0)?; // First byte needs to be unset
            buffer.write_u8(0x01)?; // IPv4


            let masked_port = remote_addr.port() ^ (STUN_COOKIE >> 16) as u16; // Mask with first 16-most-significant-bits
            let mut masked_address = remote_addr.ip().octets();
            xor_range(&mut masked_address, &mut STUN_COOKIE.to_be_bytes());

            buffer.write_u16::<BigEndian>(masked_port)?;
            buffer.write(&masked_address)?;
        }
        SocketAddr::V6(remote_addr) => {
            buffer.write_u16::<BigEndian>(StunAttributeType::XORMappedAddress as u16)?; // Type
            buffer.write_u16::<BigEndian>(20)?; // Length
            buffer.write_u8(0)?; // First byte needs to be unset
            buffer.write_u8(0x02)?; // IPv6


            let masked_port = remote_addr.port() ^ (STUN_COOKIE >> 16) as u16; // Mask with first 16-most-significant-bits
            let mut masked_address = remote_addr.ip().octets();
            let mut mask = vec![0; 16];
            mask.write(&STUN_COOKIE.to_be_bytes())?;
            mask.write(&transaction_id)?;

            xor_range(&mut masked_address, &mask);

            buffer.write_u16::<BigEndian>(masked_port)?;
            buffer.write(&masked_address)?;
        }
    };

    Ok(buffer)
}

fn xor_range(target: &mut [u8], xor: &[u8]) {
    for i in 0..target.len() {
        target[i] ^= xor[i];
    }
}

fn pad_to_4bytes(value: u16) -> u16 {
    let modulo = value % 4;
    match modulo {
        0 => value,
        _ => value + 4 - modulo
    }
}

#[derive(Debug)]
pub struct StunBindingRequest {
    pub attributes: Vec<StunAttribute>,
    pub transaction_id: [u8; STUN_TRANSACTION_ID_LEN],
}

#[derive(Debug)]
pub enum ICEStunMessageType {
    LiveCheck(ICEStunPacket),
    Nomination(ICEStunPacket),
}

#[derive(Debug)]
pub struct ICEStunPacket {
    pub username_attribute: SessionUsername,
    pub message_integrity: [u8; STUN_MESSAGE_INTEGRITY_LEN],
    pub transaction_id: [u8; STUN_TRANSACTION_ID_LEN],

}


#[derive(Debug)]
enum StunAttributeType {
    Username = 0x6,
    MessageIntegrity = 0x8,
    IceControlling = 0x802a,
    UseCandidate = 0x25,
    XORMappedAddress = 0x002,
    Unknown,
}


enum StunType {
    BindingRequest = 0x0001,
    SuccessResponse = 0x0101,
}

#[derive(Debug)]
pub enum StunAttribute {
    Unknown,
    MessageIntegrity([u8; STUN_MESSAGE_INTEGRITY_LEN]),
    Username(SessionUsername),
    IceControlling,
    UseCandidate,
}


const STUN_MESSAGE_INTEGRITY_LEN: usize = 20;
const STUN_TRANSACTION_ID_LEN: usize = 12;
const STUN_HEADER_LEN: usize = 20;
const STUN_ALIGNMENT: usize = 4;
const STUN_COOKIE: u32 = 0x2112a442;
const STUN_CRC_XOR: u32 = 0x5354554e;
