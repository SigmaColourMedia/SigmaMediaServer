#[derive(Debug)]
pub struct StunBindingRequest {
    attributes: Vec<StunAttribute>,
    transaction_id: [u8; STUN_TRANSACTION_ID_LEN],
}

//
// pub fn parse_stun_packet(packet: &[u8]) -> Option<StunBindingRequest> {
//     if packet.len() < STUN_HEADER_LEN {
//         return None;
//     }
//     let mut reader = BufReader::new(packet);
//     let message_type = reader.read_u16().ok()?;
//     if message_type != StunType::BindingRequest as u16 {
//         return None;
//     }
//
//     let length = reader.read_u16().ok()?;
//     if length % 4 != 0 || STUN_HEADER_LEN + length as usize > packet.len() {
//         return None;
//     }
//
//     let magic_cookie = reader.read_u32().ok()?;
//     if magic_cookie != STUN_COOKIE {
//         return None;
//     }
//
//     let mut transaction_id = [0; 12];
//     reader.read(&mut transaction_id).ok()?;
//
//
//     let mut attributes: Vec<StunAttribute> = Vec::new();
//
//     while let Ok(attribute_type_key) = reader.read_u16() {
//         let attribute_type: StunAttributeType = match attribute_type_key {
//             0x6 => StunAttributeType::Username,
//             0x8 => StunAttributeType::MessageIntegrity,
//             0x802a => StunAttributeType::IceControlling,
//             0x25 => StunAttributeType::UseCandidate,
//             _ => StunAttributeType::Unknown
//         };
//
//         let mut length = reader.read_u16().unwrap();
//         length = pad_to_4bytes(length);
//         let mut value_buffer: Vec<u8> = vec![0; length as usize];
//         reader.read_exact(&mut value_buffer).unwrap();
//
//         match attribute_type {
//             StunAttributeType::Username => {
//                 let username_string = String::from_utf8(value_buffer).unwrap();
//                 let (host_username, remote_username) = username_string.split_once(":").unwrap();
//                 attributes.push(StunAttribute::Username(UsernameAttribute {
//                     host_username: host_username.to_owned(),
//                     remote_username: remote_username.to_owned(),
//                 }))
//             }
//             StunAttributeType::MessageIntegrity => {
//                 attributes.push(StunAttribute::MessageIntegrity(value_buffer))
//             }
//             StunAttributeType::IceControlling => {
//                 attributes.push(StunAttribute::IceControlling)
//             }
//             StunAttributeType::UseCandidate => {
//                 attributes.push(StunAttribute::UseCandidate)
//             }
//             StunAttributeType::Unknown => {
//                 attributes.push(StunAttribute::Unknown)
//             }
//         }
//     }
//
//     return Some(StunBindingRequest {
//         transaction_id,
//         attributes,
//     });
// }

fn pad_to_4bytes(value: u16) -> u16 {
    let modulo = value % 4;
    match modulo {
        0 => value,
        _ => value + 4 - modulo
    }
}


enum StunType {
    BindingRequest = 0x0001,
    SuccessResponse = 0x0101,
}

#[derive(Debug)]
enum StunAttributeType {
    Username = 0x6,
    MessageIntegrity = 0x8,
    IceControlling = 0x802a,
    UseCandidate = 0x25,
    Unknown,
}

#[derive(Debug)]
enum StunAttribute {
    Unknown,
    MessageIntegrity(Vec<u8>),
    Username(UsernameAttribute),
    IceControlling,
    UseCandidate,
}

#[derive(Debug)]
struct UsernameAttribute {
    remote_username: String,
    host_username: String,
}

const STUN_TRANSACTION_ID_LEN: usize = 12;
const STUN_HEADER_LEN: usize = 20;
const STUN_ALIGNMENT: usize = 4;
const STUN_COOKIE: u32 = 0x2112a442;
const STUN_CRC_XOR: u32 = 0x5354554e;
