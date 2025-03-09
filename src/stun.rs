use std::io::{BufReader, BufWriter, Error, Read, Write};
use std::net::SocketAddr;

use byteorder::{BigEndian, ByteOrder, ReadBytesExt, WriteBytesExt};
use openssl::hash::MessageDigest;
use openssl::pkey::PKey;
use openssl::sign::Signer;

use sdp::ICECredentials;
use crate::ice_registry::SessionUsername;


// todo Refactor this and move into internal crate

fn parse_stun_packet(packet: &[u8]) -> Option<StunBindingRequest> {
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
            _ => StunAttributeType::Unknown,
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
            StunAttributeType::IceControlling => attributes.push(StunAttribute::IceControlling),
            StunAttributeType::UseCandidate => attributes.push(StunAttribute::UseCandidate),
            _ => attributes.push(StunAttribute::Unknown),
        }
    }

    return Some(StunBindingRequest {
        transaction_id,
        attributes,
    });
}

fn parse_binding_request(stun_message: StunBindingRequest) -> Option<ICEStunMessageType> {
    let message_integrity = stun_message.attributes.iter().find_map(|attr| match attr {
        StunAttribute::MessageIntegrity(integrity) => Some(*integrity),
        _ => None,
    })?;

    let nominate_flag = stun_message.attributes.iter().find_map(|attr| match attr {
        StunAttribute::UseCandidate => Some(()),
        _ => None,
    });
    let session_username = stun_message
        .attributes
        .into_iter()
        .find_map(|attr| match attr {
            StunAttribute::Username(username_session) => Some(username_session),
            _ => None,
        })?;

    match nominate_flag {
        None => Some(ICEStunMessageType::LiveCheck(ICEStunPacket {
            message_integrity,
            username_attribute: session_username,
            transaction_id: stun_message.transaction_id,
        })),
        Some(_) => Some(ICEStunMessageType::Nomination(ICEStunPacket {
            message_integrity,
            username_attribute: session_username,
            transaction_id: stun_message.transaction_id,
        })),
    }
}

pub fn get_stun_packet(data: &[u8]) -> Option<ICEStunMessageType> {
    parse_stun_packet(data).and_then(parse_binding_request)
}

pub fn create_stun_success(
    credentials: &ICECredentials,
    transaction_id: [u8; STUN_TRANSACTION_ID_LEN],
    remote: &SocketAddr,
    buffer: &mut [u8],
) -> Result<usize, Error> {
    let (header, attributes) = buffer.split_at_mut(20);

    let mut username_attribute = [0u8; 120];
    let username_attr_length = write_username_attribute(&mut username_attribute, credentials);
    let username_attribute = &username_attribute[..username_attr_length];

    let xor_attr = compute_xor_mapped_address(remote, transaction_id)?;

    let mut mapped_address_attribute = [0u8; 24];
    let mapped_address_attribute_len =
        write_mapped_address_attribute(&mut mapped_address_attribute, remote);
    let mapped_address_attribute = &mapped_address_attribute[..mapped_address_attribute_len];

    let message_length = xor_attr.len()
        + STUN_MESSAGE_INTEGRITY_ATTRIBUTE_LEN
        + username_attr_length
        + mapped_address_attribute_len;

    BigEndian::write_u16(&mut header[..2], StunType::SuccessResponse as u16); // Write message type
    BigEndian::write_u16(&mut header[2..4], message_length as u16); // Write message length
    BigEndian::write_u32(&mut header[4..8], STUN_COOKIE); // Write MAGIC Cookie
    header[8..20].copy_from_slice(&transaction_id); // Write transaction id

    let mut attributes_writer = BufWriter::new(attributes);

    attributes_writer.write(username_attribute)?;
    attributes_writer.write(&xor_attr)?;
    attributes_writer.write(&mapped_address_attribute)?;

    let mut message_integrity_attribute = [0u8; 24];
    write_message_integrity_attribute(
        &mut message_integrity_attribute,
        header,
        attributes_writer.buffer(),
        &credentials.host_password,
    );
    attributes_writer.write(&message_integrity_attribute)?;

    attributes_writer.flush()?;
    std::mem::drop(attributes_writer);

    BigEndian::write_u16(&mut header[2..4], message_length as u16 + 8); // Write message length

    let fingerprint = crc32fast::hash(&buffer[..20 + message_length]) ^ 0x5354554e;
    let mut fingerprint_attribute = [0u8; 8];
    BigEndian::write_u16(
        &mut fingerprint_attribute[..2],
        StunAttributeType::Fingerprint as u16,
    );
    BigEndian::write_u16(&mut fingerprint_attribute[2..4], 0x4);
    BigEndian::write_u32(&mut fingerprint_attribute[4..], fingerprint);
    buffer[STUN_HEADER_LEN + message_length
        ..STUN_HEADER_LEN + message_length + fingerprint_attribute.len()]
        .copy_from_slice(&mut fingerprint_attribute);

    Ok(STUN_HEADER_LEN + message_length + 8)
}

// todo handle unwraps
fn write_message_integrity_attribute(
    mut buffer: &mut [u8],
    header: &[u8],
    attributes: &[u8],
    key: &str,
) -> usize {
    let key = PKey::hmac(key.as_bytes()).unwrap();

    let mut signer = Signer::new(MessageDigest::sha1(), &key).unwrap();
    signer.update(header).unwrap();
    signer.update(attributes).unwrap();
    buffer
        .write_u16::<BigEndian>(StunAttributeType::MessageIntegrity as u16)
        .unwrap();
    buffer.write_u16::<BigEndian>(20).unwrap();
    signer.sign(&mut buffer).unwrap()
}

fn write_username_attribute(buffer: &mut [u8], credentials: &ICECredentials) -> usize {
    let mut writer = BufWriter::new(buffer);
    writer
        .write_u16::<BigEndian>(StunAttributeType::Username as u16)
        .unwrap();
    let mut username = format!(
        "{}:{}",
        credentials.host_username, credentials.remote_username
    );
    writer
        .write_u16::<BigEndian>(username.len() as u16)
        .unwrap();

    let padded_length = pad_to_4bytes(username.len() as u16) as usize;
    if padded_length > username.len() {
        username.push_str(&"\0".repeat(padded_length - username.len()));
        writer.write(username.as_bytes()).unwrap();
    }
    let buff_len = writer.buffer().len();

    writer.flush().unwrap();
    buff_len
}

fn write_mapped_address_attribute(buffer: &mut [u8], remote: &SocketAddr) -> usize {
    let mut writer = BufWriter::new(buffer);
    writer
        .write_u16::<BigEndian>(StunAttributeType::MappedAddress as u16)
        .unwrap();

    match remote {
        SocketAddr::V4(remote_addr) => {
            writer.write_u16::<BigEndian>(0x8).unwrap(); // Message length
            writer.write_u8(0x0).unwrap();
            writer.write_u8(0x01).unwrap();
            writer.write_u16::<BigEndian>(remote_addr.port()).unwrap();
            writer.write(&remote_addr.ip().octets()).unwrap();
        }
        SocketAddr::V6(remote_addr) => {
            writer.write_u16::<BigEndian>(0x14).unwrap(); // Message length
            writer.write_u8(0x0).unwrap();
            writer.write_u8(0x2).unwrap();
            writer.write_u16::<BigEndian>(remote_addr.port()).unwrap();
            writer.write(&remote_addr.ip().octets()).unwrap();
        }
    };

    let bytes_written = writer.buffer().len();
    writer.flush().unwrap();
    bytes_written
}

fn compute_xor_mapped_address(
    remote: &SocketAddr,
    transaction_id: [u8; STUN_TRANSACTION_ID_LEN],
) -> Result<Vec<u8>, Error> {
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
            let mut mask = [0; 16];
            mask[0..4].copy_from_slice(&STUN_COOKIE.to_be_bytes());
            mask[4..].copy_from_slice(&transaction_id);

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
        _ => value + 4 - modulo,
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
    MappedAddress = 0x1,
    Username = 0x6,
    MessageIntegrity = 0x8,
    IceControlling = 0x802a,
    UseCandidate = 0x25,
    XORMappedAddress = 0x020,
    Fingerprint = 0x8028,
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
const STUN_MESSAGE_INTEGRITY_ATTRIBUTE_LEN: usize = 24;

const STUN_TRANSACTION_ID_LEN: usize = 12;
const STUN_HEADER_LEN: usize = 20;
const STUN_COOKIE: u32 = 0x2112a442;
