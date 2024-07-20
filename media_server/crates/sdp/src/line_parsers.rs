use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

#[derive(Debug)]
pub(crate) struct SDPOffer {
    pub(crate) ice_username: ICEUsername,
    pub(crate) ice_password: ICEPassword,
    pub(crate) audio_media_description: Vec<Attribute>,
    pub(crate) video_media_description: Vec<Attribute>,
}
#[derive(Debug)]
pub enum SDPParseError {
    SequenceError,
    MissingICECredentials,
    UnsupportedMediaCount,
    UnsupportedMediaType,
    UnsupportedMediaProtocol,
    MalformedAttribute,
    MalformedMediaDescriptor,
    MalformedSDPLine,
}
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SDPLine {
    ProtocolVersion(String),
    Originator(String),
    SessionName(String),
    SessionTime(String),
    ConnectionData(ConnectionData),
    Attribute(Attribute),
    MediaDescription(MediaDescription),
    Unrecognized,
}

#[derive(Debug, PartialEq, Clone)]
struct ConnectionData {
    ip: IpAddr,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Attribute {
    Unrecognized,
    SendOnly,
    ReceiveOnly,
    MediaID(MediaID),
    ICEUsername(ICEUsername),
    ICEPassword(ICEPassword),
    Fingerprint(Fingerprint),
    MediaGroup(MediaGroup),
    MediaSSRC(MediaSSRC),
    RTCPMux,
    RTPMap(RTPMap),
    FMTP(FMTP),
    Candidate(Candidate),
}

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct MediaDescription {
    pub(crate) media_type: MediaType,
    transport_port: usize,
    transport_protocol: MediaTransportProtocol,
    media_format_description: Vec<usize>,
}

#[derive(Debug, PartialEq, Clone)]
pub(crate) enum MediaType {
    Video,
    Audio,
}

#[derive(Debug, PartialEq, Clone)]
enum MediaTransportProtocol {
    DtlsSrtp,
}

#[derive(Clone, Debug, PartialEq)]
struct MediaID {
    id: String,
}

#[derive(Clone, Debug, PartialEq)]
struct MediaGroup {
    group: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Fingerprint {
    hash_function: HashFunction,
    hash: String,
}

#[derive(Debug, Clone, PartialEq)]
enum HashFunction {
    SHA256,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum MediaCodec {
    Audio(AudioCodec),
    Video(VideoCodec),
    Unsupported,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum VideoCodec {
    H264,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum AudioCodec {
    Opus,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MediaSSRC {
    pub(crate) ssrc: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RTPMap {
    pub(crate) codec: MediaCodec,
    pub(crate) payload_number: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FMTP {
    pub(crate) payload_number: usize,
    pub(crate) format_capability: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Candidate {
    pub(crate) foundation: String,
    pub(crate) component_id: usize,
    pub(crate) priority: usize,
    pub(crate) connection_address: IpAddr,
    pub(crate) port: u16,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ICEUsername {
    pub(crate) username: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ICEPassword {
    pub(crate) password: String,
}

impl From<SDPLine> for String {
    fn from(value: SDPLine) -> Self {
        match value {
            SDPLine::ProtocolVersion(proto) => format!("v={}", proto),
            SDPLine::Originator(originator) => format!("o={}", originator),
            SDPLine::SessionName(session_name) => format!("s={}", session_name),
            SDPLine::SessionTime(session_time) => format!("t={}", session_time),
            SDPLine::ConnectionData(connection_data) => String::from(connection_data),
            SDPLine::Attribute(attr) => String::from(attr),
            SDPLine::MediaDescription(media_description) => String::from(media_description),
            SDPLine::Unrecognized => "".to_string(), //todo handle Unrecognized cases
        }
    }
}

impl From<ConnectionData> for String {
    fn from(value: ConnectionData) -> Self {
        let ip_family = match &value.ip {
            IpAddr::V4(_) => "IP4",
            IpAddr::V6(_) => "IP6",
        };
        format!("c=IN {} {}", ip_family, ip_family.to_string())
    }
}

impl From<Attribute> for String {
    fn from(value: Attribute) -> Self {
        let attribute_name = match value {
            Attribute::Unrecognized => "undefined".to_string(),
            Attribute::SendOnly => "sendonly".to_string(),
            Attribute::ReceiveOnly => "recvonly".to_string(),
            Attribute::RTCPMux => "rtcp-mux".to_string(),
            Attribute::MediaID(attr) => String::from(attr),
            Attribute::ICEUsername(attr) => String::from(attr),
            Attribute::ICEPassword(attr) => String::from(attr),
            Attribute::Fingerprint(attr) => String::from(attr),
            Attribute::MediaGroup(attr) => String::from(attr),
            Attribute::MediaSSRC(attr) => String::from(attr),
            Attribute::RTPMap(attr) => String::from(attr),
            Attribute::FMTP(attr) => String::from(attr),
            Attribute::Candidate(attr) => String::from(attr),
        };
        format!("a={attribute_name}")
    }
}

impl From<ICEUsername> for String {
    fn from(value: ICEUsername) -> Self {
        format!("ice-ufrag:{}", value.username)
    }
}

impl From<ICEPassword> for String {
    fn from(value: ICEPassword) -> Self {
        format!("ice-pwd:{}", value.password)
    }
}

impl From<MediaType> for String {
    fn from(value: MediaType) -> Self {
        match value {
            MediaType::Video => "video".to_string(),

            MediaType::Audio => "audio".to_string(),
        }
    }
}

impl From<MediaTransportProtocol> for String {
    fn from(value: MediaTransportProtocol) -> Self {
        match value {
            MediaTransportProtocol::DtlsSrtp => "UDP/TLS/RTP/SAVPF".to_string(),
        }
    }
}

impl From<MediaDescription> for String {
    fn from(value: MediaDescription) -> Self {
        let media_payloads = value
            .media_format_description
            .into_iter()
            .map(|item| item.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        format!(
            "m={} {} {} {}",
            String::from(value.media_type),
            value.transport_port,
            String::from(value.transport_protocol),
            media_payloads
        )
    }
}

impl From<MediaID> for String {
    fn from(value: MediaID) -> Self {
        format!("mid:{}", value.id)
    }
}

impl From<MediaGroup> for String {
    fn from(value: MediaGroup) -> Self {
        format!("group:{}", value.group)
    }
}

impl From<Fingerprint> for String {
    fn from(value: Fingerprint) -> Self {
        format!(
            "fingerprint:{} {}",
            String::from(value.hash_function),
            value.hash
        )
    }
}

impl From<HashFunction> for String {
    fn from(value: HashFunction) -> Self {
        match value {
            HashFunction::SHA256 => "sha-256".to_string(),
            HashFunction::Unsupported => {
                panic!("Unsupported HashFunction cannot be converted to String")
            }
        }
    }
}

impl From<RTPMap> for String {
    fn from(value: RTPMap) -> Self {
        format!(
            "rtpmap:{} {}",
            value.payload_number,
            String::from(value.codec)
        )
    }
}

impl From<MediaCodec> for String {
    fn from(value: MediaCodec) -> Self {
        match value {
            MediaCodec::Audio(audio_codec) => String::from(audio_codec),
            MediaCodec::Video(video_codec) => String::from(video_codec),
            MediaCodec::Unsupported => {
                panic!("Unsupported MediaCodec cannot be converted to String")
            }
        }
    }
}

impl From<VideoCodec> for String {
    fn from(value: VideoCodec) -> Self {
        match value {
            VideoCodec::H264 => "h264/90000".to_string(),
        }
    }
}

impl From<AudioCodec> for String {
    fn from(value: AudioCodec) -> Self {
        match value {
            AudioCodec::Opus => "opus/48000/2".to_string(),
        }
    }
}

impl From<MediaSSRC> for String {
    fn from(value: MediaSSRC) -> Self {
        format!("ssrc:{}", value.ssrc)
    }
}

impl From<FMTP> for String {
    fn from(value: FMTP) -> Self {
        let format_capabilities = value.format_capability.join(";");
        format!("fmtp:{} {}", value.payload_number, format_capabilities)
    }
}

impl From<Candidate> for String {
    fn from(value: Candidate) -> Self {
        format!(
            "cadidate:{} {} UDP {} {} {}",
            value.foundation,
            value.component_id,
            value.priority,
            value.connection_address.to_string(),
            value.port
        )
    }
}

impl TryFrom<&str> for SDPLine {
    type Error = SDPParseError;

    fn try_from(input: &str) -> Result<Self, Self::Error> {
        let (sdp_type, value) = input
            .split_once("=")
            .ok_or(SDPParseError::MalformedSDPLine)?;

        match sdp_type {
            "v" => Ok(SDPLine::ProtocolVersion(value.to_string())),
            "c" => Ok(SDPLine::ConnectionData(ConnectionData::try_from(input)?)),
            "o" => Ok(SDPLine::Originator(value.to_string())),
            "s" => Ok(SDPLine::SessionName(value.to_string())),
            "t" => Ok(SDPLine::SessionTime(value.to_string())),
            "m" => Ok(SDPLine::MediaDescription(MediaDescription::try_from(
                input,
            )?)),
            "a" => Ok(SDPLine::Attribute(Attribute::try_from(input)?)),
            _ => Ok(SDPLine::Unrecognized),
        }
    }
}

impl TryFrom<&str> for Attribute {
    type Error = SDPParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let (_, value) = value
            .split_once("a=")
            .ok_or(Self::Error::MalformedAttribute)?;
        let key = value
            .split(":")
            .next()
            .ok_or(Self::Error::MalformedAttribute)?;

        match key {
            "ice-ufrag" => Ok(Attribute::ICEUsername(ICEUsername::try_from(value)?)),
            "ice-pwd" => Ok(Attribute::ICEPassword(ICEPassword::try_from(value)?)),
            "fingerprint" => Ok(Attribute::Fingerprint(Fingerprint::try_from(value)?)),
            "candidate" => Ok(Attribute::Candidate(Candidate::try_from(value)?)),
            "ssrc" => Ok(Attribute::MediaSSRC(MediaSSRC::try_from(value)?)),
            "sendonly" => Ok(Attribute::SendOnly),
            "recvonly" => Ok(Attribute::ReceiveOnly),
            "mid" => Ok(Attribute::MediaID(MediaID::try_from(value)?)),
            "group" => Ok(Attribute::MediaGroup(MediaGroup::try_from(value)?)),
            "rtpmap" => Ok(Attribute::RTPMap(RTPMap::try_from(value)?)),
            "fmtp" => Ok(Attribute::FMTP(FMTP::try_from(value)?)),
            "rtcp-mux" => Ok(Attribute::RTCPMux),
            _ => Ok(Attribute::Unrecognized),
        }
    }
}

impl TryFrom<&str> for MediaDescription {
    type Error = SDPParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let (_, value) = value
            .split_once("m=")
            .ok_or(Self::Error::MalformedSDPLine)?;
        let mut split = value.split(" ");

        let media_type = split
            .next()
            .ok_or(SDPParseError::MalformedMediaDescriptor)
            .and_then(|media_type| MediaType::try_from(media_type))?;

        let transport_port = split
            .next()
            .and_then(|port| port.parse::<usize>().ok())
            .ok_or(SDPParseError::MalformedMediaDescriptor)?;

        let transport_protocol = split
            .next()
            .ok_or(SDPParseError::MalformedMediaDescriptor)
            .and_then(|transport_protocol| MediaTransportProtocol::try_from(transport_protocol))?;

        let media_format_description = split
            .take_while(|line| !line.is_empty())
            .map(|line| line.parse::<usize>().ok())
            .collect::<Option<Vec<usize>>>()
            .ok_or(SDPParseError::MalformedAttribute)?;

        Ok(MediaDescription {
            transport_port,
            media_type,
            media_format_description,
            transport_protocol,
        })
    }
}

impl TryFrom<&str> for ConnectionData {
    type Error = SDPParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let (_, value) = value
            .split_once("c=")
            .ok_or(Self::Error::MalformedSDPLine)?;
        let mut split = value.split(" ");

        let first_line_matches_pattern = split
            .next()
            .ok_or(Self::Error::MalformedSDPLine)?
            .eq_ignore_ascii_case("in");

        if !first_line_matches_pattern {
            return Err(Self::Error::MalformedSDPLine);
        }

        let ip_addr = split
            .next()
            .and_then(|line| match line {
                "IP4" => {
                    let unparsed_ip = split.next()?;
                    let ip = Ipv4Addr::from_str(unparsed_ip).ok()?;
                    Some(IpAddr::V4(ip))
                }
                "IP6" => {
                    let unparsed_ip = split.next()?;
                    let ip = Ipv6Addr::from_str(unparsed_ip).ok()?;
                    Some(IpAddr::V6(ip))
                }
                _ => None,
            })
            .ok_or(Self::Error::MalformedSDPLine)?;

        Ok(Self { ip: ip_addr })
    }
}

impl TryFrom<&str> for MediaType {
    type Error = SDPParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "video" => Ok(Self::Video),
            "audio" => Ok(Self::Audio),
            _ => Err(Self::Error::UnsupportedMediaType),
        }
    }
}

impl TryFrom<&str> for MediaTransportProtocol {
    type Error = SDPParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "UDP/TLS/RTP/SAVPF" => Ok(Self::DtlsSrtp),
            _ => Err(Self::Error::UnsupportedMediaProtocol),
        }
    }
}

impl TryFrom<&str> for MediaID {
    type Error = SDPParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let (_, value) = value
            .split_once("mid:")
            .ok_or(Self::Error::MalformedAttribute)?;
        Ok(Self {
            id: value.to_string(),
        })
    }
}

impl TryFrom<&str> for MediaGroup {
    type Error = SDPParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let (_, value) = value
            .split_once("group:")
            .ok_or(Self::Error::MalformedAttribute)?;
        Ok(Self {
            group: value.to_string(),
        })
    }
}

impl TryFrom<&str> for Fingerprint {
    type Error = SDPParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let (_, value) = value
            .split_once("fingerprint:")
            .ok_or(Self::Error::MalformedAttribute)?;
        let (hash_function, hash) = value
            .split_once(" ")
            .ok_or(SDPParseError::MalformedAttribute)?;

        let hash_function = HashFunction::from(hash_function);

        Ok(Fingerprint {
            hash_function,
            hash: hash.to_string(),
        })
    }
}

impl From<&str> for HashFunction {
    fn from(value: &str) -> Self {
        match value {
            "sha-256" => HashFunction::SHA256,
            _ => HashFunction::Unsupported,
        }
    }
}

impl TryFrom<&str> for RTPMap {
    type Error = SDPParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let (_, value) = value
            .split_once("rtpmap:")
            .ok_or(Self::Error::MalformedAttribute)?;
        let (payload_number, codec) = value
            .split_once(" ")
            .ok_or(SDPParseError::MalformedAttribute)?;

        let payload_number = payload_number
            .parse::<usize>()
            .map_err(|_| SDPParseError::MalformedAttribute)?;

        let media_codec = match codec.to_ascii_lowercase().as_str() {
            "h264/90000" => MediaCodec::Video(VideoCodec::H264),
            "opus/48000/2" => MediaCodec::Audio(AudioCodec::Opus),
            _ => MediaCodec::Unsupported,
        };

        Ok(RTPMap {
            codec: media_codec,
            payload_number,
        })
    }
}

impl TryFrom<&str> for MediaSSRC {
    type Error = SDPParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let (_, value) = value
            .split_once("ssrc:")
            .ok_or(Self::Error::MalformedAttribute)?;
        let ssrc = value
            .split(" ")
            .next()
            .ok_or(SDPParseError::MalformedSDPLine)?;

        Ok(MediaSSRC {
            ssrc: ssrc.to_string(),
        })
    }
}

impl TryFrom<&str> for FMTP {
    type Error = SDPParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let (_, value) = value
            .split_once("fmtp:")
            .ok_or(Self::Error::MalformedAttribute)?;
        let (payload_number, capabilities) = value
            .split_once(" ")
            .ok_or(SDPParseError::MalformedAttribute)?;

        let payload_number = payload_number
            .parse::<usize>()
            .map_err(|_| SDPParseError::MalformedAttribute)?;

        let format_capability = capabilities
            .split(";")
            .map(ToString::to_string)
            .collect::<Vec<String>>();

        Ok(FMTP {
            format_capability,
            payload_number,
        })
    }
}

impl TryFrom<&str> for Candidate {
    type Error = SDPParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let (_, value) = value
            .split_once("candidate:")
            .ok_or(Self::Error::MalformedAttribute)?;
        let mut split = value.split(" ");
        let foundation = split
            .next()
            .ok_or(SDPParseError::MalformedAttribute)?
            .to_string();
        let component_id = split
            .next()
            .ok_or(SDPParseError::MalformedAttribute)
            .map(|id| id.parse::<usize>())?
            .map_err(|_| SDPParseError::MalformedAttribute)?;

        let protocol = split.next().ok_or(SDPParseError::MalformedAttribute)?;

        if !protocol.eq("UDP") {
            return Err(SDPParseError::MalformedAttribute);
        }

        let priority = split
            .next()
            .ok_or(SDPParseError::MalformedSDPLine)?
            .parse::<usize>()
            .map_err(|_| SDPParseError::MalformedSDPLine)?;

        let ip = split
            .next()
            .ok_or(SDPParseError::MalformedAttribute)
            .and_then(|ip| IpAddr::from_str(ip).map_err(|_| SDPParseError::MalformedAttribute))?;

        let port = split
            .next()
            .ok_or(SDPParseError::MalformedSDPLine)?
            .parse::<u16>()
            .map_err(|_| SDPParseError::MalformedSDPLine)?;

        Ok(Candidate {
            component_id,
            foundation,
            connection_address: ip,
            port,
            priority,
        })
    }
}

impl TryFrom<&str> for ICEUsername {
    type Error = SDPParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let (_, value) = value
            .split_once("ice-ufrag:")
            .ok_or(Self::Error::MalformedAttribute)?;
        Ok(ICEUsername {
            username: value.to_string(),
        })
    }
}

impl TryFrom<&str> for ICEPassword {
    type Error = SDPParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let (_, value) = value
            .split_once("ice-pwd:")
            .ok_or(Self::Error::MalformedAttribute)?;
        Ok(ICEPassword {
            password: value.to_string(),
        })
    }
}

pub fn parse_raw_sdp_offer(data: &str) -> Result<SDPOffer, SDPParseError> {
    let sdp_lines = data
        .lines()
        .map(SDPLine::try_from)
        .collect::<Result<Vec<SDPLine>, SDPParseError>>()?;

    let mut iter = sdp_lines.iter();

    // Check if session description segment is properly formatted.
    let protocol_version = iter.next().ok_or(SDPParseError::MalformedSDPLine)?;
    if !matches!(protocol_version, SDPLine::ProtocolVersion(_)) {
        return Err(SDPParseError::SequenceError);
    }

    let originator = iter.next().ok_or(SDPParseError::MalformedSDPLine)?;
    if !matches!(originator, SDPLine::Originator(_)) {
        return Err(SDPParseError::SequenceError);
    }

    let session_name = iter.next().ok_or(SDPParseError::MalformedSDPLine)?;
    if !matches!(session_name, SDPLine::SessionName(_)) {
        return Err(SDPParseError::SequenceError);
    }

    let session_time = iter.next().ok_or(SDPParseError::MalformedSDPLine)?;
    if !matches!(session_time, SDPLine::SessionTime(_)) {
        return Err(SDPParseError::SequenceError);
    }

    // Check for ICE credentials. If multiple credentials are provided, only the first occurrence will be used.
    // todo Reject SDP with multiple different ICE credential attributes
    let ice_username = sdp_lines
        .iter()
        .find_map(|line| {
            if let SDPLine::Attribute(Attribute::ICEUsername(username)) = line {
                return Some(username);
            }
            None
        })
        .ok_or(SDPParseError::MissingICECredentials)?;
    let ice_password = sdp_lines
        .iter()
        .find_map(|line| {
            if let SDPLine::Attribute(Attribute::ICEPassword(username)) = line {
                return Some(username);
            }
            None
        })
        .ok_or(SDPParseError::MissingICECredentials)?;

    // Validate media descriptor segments
    let mut media_descriptors_iter = sdp_lines
        .iter()
        .skip_while(|line| !matches!(line, SDPLine::MediaDescription(_)));

    let media_descriptor_count = media_descriptors_iter
        .clone()
        .filter(|line| matches!(line, SDPLine::MediaDescription(_)))
        .count();

    // Assert that we're dealing with 2 media descriptors to avoid redundant checks later (Audio and Video)
    if media_descriptor_count != 2 {
        return Err(SDPParseError::UnsupportedMediaCount);
    }

    let first_media_line = media_descriptors_iter
        .next()
        .map(|line| match line {
            SDPLine::MediaDescription(media_description) => media_description,
            _ => unreachable!(
                "The first item after session description end should always be media description"
            ),
        })
        .ok_or(SDPParseError::MalformedMediaDescriptor)?;

    // First media descriptor must be Audio. This is an arbitrary decision to ease implementation.
    if !matches!(first_media_line.media_type, MediaType::Audio) {
        return Err(SDPParseError::SequenceError);
    }

    let audio_media_attributes = media_descriptors_iter
        .clone()
        .take_while(|line| !matches!(line, SDPLine::MediaDescription(_)))
        .filter_map(|line| match line {
            SDPLine::Attribute(attr) => Some(attr.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();

    let mut second_media_description_segment =
        media_descriptors_iter.skip_while(|line| !matches!(line, SDPLine::MediaDescription(_)));

    let second_media_line = second_media_description_segment
        .next()
        .map(|line| match line {
            SDPLine::MediaDescription(media_description) => media_description,
            _ => unreachable!(
                "The first item after session description end should always be media description"
            ),
        })
        .expect("Second media descriptor should be present");

    if !matches!(second_media_line.media_type, MediaType::Video) {
        return Err(SDPParseError::SequenceError);
    }

    let video_media_attributes = second_media_description_segment
        .filter_map(|line| match line {
            SDPLine::Attribute(attr) => Some(attr.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();

    Ok(SDPOffer {
        ice_username: ice_username.clone(),
        ice_password: ice_password.clone(),
        audio_media_description: audio_media_attributes,
        video_media_description: video_media_attributes,
    })
}

// #[cfg(test)]
// mod tests {
//
//     mod parse_fmtp {
//         use crate::line_parsers::parse_fmtp;
//
//         #[test]
//         fn rejects_malformed_line() {
//             let attr = "96-profile-level-id other-attributes:1";
//             let parse_result = parse_fmtp(attr);
//             assert!(parse_result.is_err(), "Should reject FMTP attribute")
//         }
//
//         #[test]
//         fn resolves_all_capabilities() {
//             let attr = "96 profile-level-id=42e01f;packetization-mode=1;level-asymmetry-allowed=1";
//             let fmtp = parse_fmtp(attr).expect("FMTP should be OK");
//             assert_eq!(
//                 fmtp.payload_number, 96,
//                 "Should resolve correct payload number"
//             );
//
//             assert_eq!(
//                 fmtp.format_capability,
//                 vec![
//                     "profile-level-id=42e01f",
//                     "packetization-mode=1",
//                     "level-asymmetry-allowed=1"
//                 ]
//             )
//         }
//     }
//
//     mod parse_media_descriptor {
//         use crate::line_parsers::{
//             LineParseError, MediaTransportProtocol, MediaType, parse_media_descriptor,
//         };
//
//         #[test]
//         fn rejects_unsupported_media_type() {
//             let media_descriptor = "text 52000 UDP 96";
//
//             let parse_error =
//                 parse_media_descriptor(media_descriptor).expect_err("Should fail to parse");
//             assert!(
//                 matches!(parse_error, LineParseError::UnsupportedMediaType),
//                 "Should reject with UnsupportedMediaType error"
//             )
//         }
//
//         #[test]
//         fn rejects_unsupported_media_transport_protocol() {
//             let media_descriptor = "video 52000 UDP 96";
//
//             let parse_error =
//                 parse_media_descriptor(media_descriptor).expect_err("Should fail to parse");
//             assert!(
//                 matches!(parse_error, LineParseError::UnsupportedMediaProtocol),
//                 "Should reject with UnsupportedMediaType error"
//             )
//         }
//
//         #[test]
//         fn resolves_supported_media_with_single_payload_number() {
//             let media_descriptor = "video 52000 UDP/TLS/RTP/SAVPF 96";
//
//             let media =
//                 parse_media_descriptor(media_descriptor).expect("Should parse to MediaDescription");
//             assert!(
//                 matches!(media.media_type, MediaType::Video),
//                 "Should resolve media_type to Video"
//             );
//
//             assert!(
//                 matches!(media.transport_protocol, MediaTransportProtocol::DtlsSrtp),
//                 "Should resolve transport_protocol to DTLS_RTP"
//             );
//
//             assert_eq!(
//                 media.transport_port, 52000,
//                 "Should resolve transport port to 52000"
//             );
//             assert_eq!(
//                 media.media_format_description,
//                 vec![96],
//                 "Should resolve to single payload number: 96"
//             )
//         }
//
//         #[test]
//         fn resolves_supported_media_with_multiple_payload_numbers() {
//             let media_descriptor = "video 52000 UDP/TLS/RTP/SAVPF 96 102 112";
//
//             let media =
//                 parse_media_descriptor(media_descriptor).expect("Should parse to MediaDescription");
//             assert!(
//                 matches!(media.media_type, MediaType::Video),
//                 "Should resolve media_type to Video"
//             );
//
//             assert!(
//                 matches!(media.transport_protocol, MediaTransportProtocol::DtlsSrtp),
//                 "Should resolve transport_protocol to DTLS_RTP"
//             );
//
//             assert_eq!(
//                 media.transport_port, 52000,
//                 "Should resolve transport port to 52000"
//             );
//             assert_eq!(
//                 media.media_format_description,
//                 vec![96, 102, 112],
//                 "Should resolve to multiple payload numbers: 96, 102, 112"
//             )
//         }
//     }
//     mod parse_rtpmap {
//         use crate::line_parsers::{MediaCodec, parse_rtpmap, VideoCodec};
//
//         #[test]
//         fn recognizes_unsupported_codec() {
//             let rtp_attr = "96 myCodec";
//
//             let rtp_map = parse_rtpmap(rtp_attr).expect("Should parse to RTPMap");
//
//             assert_eq!(rtp_map.payload_number, 96, "Payload number should match");
//             assert!(
//                 matches!(rtp_map.codec, MediaCodec::Unsupported),
//                 "Codec should be unsupported"
//             )
//         }
//
//         #[test]
//         fn rejects_malformed_attribute() {
//             let rtp_attr = "96-myCodec";
//
//             let rtp_map = parse_rtpmap(rtp_attr);
//
//             assert!(rtp_map.is_err(), "Should reject attribute parse")
//         }
//
//         #[test]
//         fn accepts_lowercase_video_codec() {
//             let rtp_attr = "96 h264/90000";
//
//             let rtp_map = parse_rtpmap(rtp_attr).expect("Should parse to RTPMap");
//
//             assert_eq!(rtp_map.payload_number, 96, "Payload number should match");
//             assert!(
//                 matches!(rtp_map.codec, MediaCodec::Video(VideoCodec::H264)),
//                 "Codec should be H264"
//             )
//         }
//
//         #[test]
//         fn accepts_uppercase_video_codec() {
//             let rtp_attr = "96 H264/90000";
//
//             let rtp_map = parse_rtpmap(rtp_attr).expect("Should parse to RTPMap");
//
//             assert_eq!(rtp_map.payload_number, 96, "Payload number should match");
//             assert!(
//                 matches!(rtp_map.codec, MediaCodec::Video(VideoCodec::H264)),
//                 "Codec should be H264"
//             )
//         }
//     }
//
//     mod parse_fingerprint {
//         use crate::line_parsers::{HashFunction, parse_fingerprint};
//
//         #[test]
//         fn recognizes_unsupported_fingerprint() {
//             let unsupported_fingerprint = "sha-test EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B";
//             let sdp_parse = parse_fingerprint(unsupported_fingerprint);
//             let fingerprint = sdp_parse.expect("Fingerprint parse result should be OK");
//
//             assert_eq!(fingerprint.hash, "EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B", "Hash should match");
//             assert!(
//                 matches!(fingerprint.hash_function, HashFunction::Unsupported),
//                 "HashFunction should be Unsupported"
//             )
//         }
//
//         #[test]
//         fn fails_on_malformed_attribute() {
//             let unsupported_fingerprint = "sha-1,EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B";
//             let sdp_parse = parse_fingerprint(unsupported_fingerprint);
//             assert!(sdp_parse.is_err(), "Should return Err");
//         }
//
//         #[test]
//         fn recognizes_sha_256() {
//             let unsupported_fingerprint = "sha-256 EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B";
//             let sdp_parse = parse_fingerprint(unsupported_fingerprint);
//
//             let fingerprint = sdp_parse.expect("Fingerprint parse result should be OK");
//
//             assert_eq!(fingerprint.hash, "EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B", "Hash should match");
//             assert!(
//                 matches!(fingerprint.hash_function, HashFunction::SHA256),
//                 "HashFunction should be SHA256"
//             )
//         }
//     }
// }
const EXAMPLE_SDP: &str = "v=0\r\no=rtc 3767197920 0 IN IP4 127.0.0.1\r\ns=-\r\nt=0 0\r\na=group:BUNDLE 0 1\r\na=group:LS 0 1\r\na=msid-semantic:WMS *\r\na=setup:actpass\r\na=ice-ufrag:E2Fr\r\na=ice-pwd:OpQzg1PAwUdeOB244chlgd\r\na=ice-options:trickle\r\na=fingerprint:sha-256 EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B\r\nm=audio 4557 UDP/TLS/RTP/SAVPF 111\r\nc=IN IP4 192.168.0.198\r\na=mid:0\r\na=sendonly\r\na=ssrc:1349455989 cname:0X2NGAsK9XcmnsuZ\r\na=ssrc:1349455989 msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-audio\r\na=msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-audio\r\na=rtcp-mux\r\na=rtpmap:111 opus/48000/2\r\na=fmtp:111 minptime=10;maxaveragebitrate=96000;stereo=1;sprop-stereo=1;useinbandfec=1\r\na=candidate:1 1 UDP 2015363327 192.168.0.198 4557 typ host\r\na=candidate:2 1 UDP 2015363583 fe80::6c3d:5b42:1532:2f9a 10007 typ host\r\na=end-of-candidates\r\nm=video 4557 UDP/TLS/RTP/SAVPF 96\r\nc=IN IP4 192.168.0.198\r\na=mid:1\r\na=sendonly\r\na=ssrc:1349455990 cname:0X2NGAsK9XcmnsuZ\r\na=ssrc:1349455990 msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-video\r\na=msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-video\r\na=rtcp-mux\r\na=rtpmap:96 H264/90000\r\na=rtcp-fb:96 nack\r\na=rtcp-fb:96 nack pli\r\na=rtcp-fb:96 goog-remb\r\na=fmtp:96 profile-level-id=42e01f;packetization-mode=1;level-asymmetry-allowed=1\r\n";
