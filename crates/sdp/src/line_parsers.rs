use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

use crate::SDPParseError::MalformedAttribute;

#[derive(Debug)]
pub enum SDPParseError {
    SequenceError,
    InvalidDTLSRole,
    MissingICECredentials,
    MissingStreamSSRC,
    UnsupportedMediaCodecs,
    InvalidStreamDirection,
    InvalidMediaID,
    BundleRequired,
    MissingVideoCapabilities,
    DemuxRequired,
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
    Originator(Originator),
    SessionName(String),
    SessionTime(SessionTime),
    ConnectionData(ConnectionData),
    Attribute(Attribute),
    MediaDescription(MediaDescription),
    Unrecognized,
}

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct ConnectionData {
    pub(crate) ip: IpAddr,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Attribute {
    Unrecognized,
    EndOfCandidates,
    ICELite,
    Feedback(Feedback),
    ICEOptions(ICEOptions),
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
    Setup(Setup),
    Candidate(Candidate),
}

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct MediaDescription {
    pub(crate) media_type: MediaType,
    pub(crate) transport_port: usize,
    pub(crate) transport_protocol: MediaTransportProtocol,
    pub(crate) media_format_description: Vec<usize>,
}

#[derive(Debug, PartialEq, Clone)]
pub(crate) enum MediaType {
    Video,
    Audio,
}

#[derive(Debug, PartialEq, Clone)]
pub(crate) enum MediaTransportProtocol {
    DtlsSrtp,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ICEOption {
    ICE2,
    Trickle,
    Unsupported,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Originator {
    pub(crate) username: String,
    pub(crate) session_id: String,
    pub(crate) session_version: String,
    pub(crate) ip_addr: IpAddr,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SessionTime {
    pub(crate) start_time: usize,
    pub(crate) end_time: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ICEOptions {
    pub(crate) options: Vec<ICEOption>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MediaID {
    pub(crate) id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Fingerprint {
    pub(crate) hash_function: HashFunction,
    pub(crate) hash: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Feedback {
    pub(crate) payload_type: usize,
    pub(crate) feedback_type: FeedbackType,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FeedbackType {
    NACK,
    PLI,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum HashFunction {
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
pub enum VideoCodec {
    H264,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AudioCodec {
    Opus,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MediaSSRC {
    pub(crate) ssrc: u32,
    pub(crate) source_attribute: SourceAttribute,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SourceAttribute {
    CNAME(String),
    Unsupported,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Setup {
    ActivePassive,
    Active,
    Passive,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum MediaGroup {
    Bundle(Vec<String>),
    LipSync(Vec<String>),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RTPMap {
    pub(crate) codec: MediaCodec,
    pub(crate) payload_number: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FMTP {
    pub(crate) payload_number: usize,
    pub(crate) format_capability: HashSet<String>,
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
            SDPLine::Originator(originator) => String::from(originator),
            SDPLine::SessionName(session_name) => format!("s={}", session_name),
            SDPLine::SessionTime(session_time) => String::from(session_time),
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
        format!("c=IN {} {}", ip_family, value.ip.to_string())
    }
}

impl From<Attribute> for String {
    fn from(value: Attribute) -> Self {
        let attribute_name = match value {
            Attribute::Unrecognized => {
                panic!("Unrecognized attributes should not be converted to String")
            }
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
            Attribute::Setup(attr) => String::from(attr),
            Attribute::ICELite => "ice-lite".to_string(),
            Attribute::EndOfCandidates => "end-of-candidates".to_string(),
            Attribute::ICEOptions(ice_options) => String::from(ice_options),
            Attribute::Feedback(feedback) => String::from(feedback),
        };
        format!("a={attribute_name}")
    }
}

impl From<Feedback> for String {
    fn from(value: Feedback) -> Self {
        match value.feedback_type {
            FeedbackType::NACK => format!("rtcp-fb:{} nack", value.payload_type),
            FeedbackType::PLI => format!("rtcp-fb:{} nack pli", value.payload_type),
            FeedbackType::Unsupported => {
                panic!("Attempted to map unsupported FeedbackType to String")
            }
        }
    }
}

impl From<SessionTime> for String {
    fn from(value: SessionTime) -> Self {
        format!("t={} {}", value.start_time, value.end_time)
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

impl From<Originator> for String {
    fn from(value: Originator) -> Self {
        let ip_version = match value.ip_addr {
            IpAddr::V4(_) => "IP4",
            IpAddr::V6(_) => "IP6",
        };
        format!(
            "o={} {} {} IN {} {}",
            value.username,
            value.session_id,
            value.session_version,
            ip_version,
            value.ip_addr.to_string()
        )
    }
}

impl From<ICEOptions> for String {
    fn from(value: ICEOptions) -> Self {
        let ice_options = value
            .options
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
            .join(" ");
        format!("ice-options:{}", ice_options)
    }
}

impl From<ICEOption> for String {
    fn from(value: ICEOption) -> Self {
        match value {
            ICEOption::ICE2 => "ice2".to_string(),
            ICEOption::Trickle => "trickle".to_string(),
            ICEOption::Unsupported => {
                panic!("Unsupported attributes should not be converted to String")
            }
        }
    }
}

impl From<Setup> for String {
    fn from(value: Setup) -> Self {
        match value {
            Setup::ActivePassive => "setup:actpass".to_string(),
            Setup::Active => "setup:active".to_string(),
            Setup::Passive => "setup:passive".to_string(),
        }
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
        match value {
            MediaGroup::Bundle(groups) => {
                format!("group:BUNDLE {}", groups.join(" "))
            }
            MediaGroup::LipSync(groups) => {
                format!("group:LS {}", groups.join(" "))
            }
        }
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
        format!(
            "ssrc:{} {}",
            value.ssrc,
            String::from(value.source_attribute)
        )
    }
}

impl From<SourceAttribute> for String {
    fn from(value: SourceAttribute) -> Self {
        match value {
            SourceAttribute::CNAME(cname) => format!("cname:{}", cname),
            SourceAttribute::Unsupported => {
                panic!("Cannot cast unsupported SourceAttribute to String")
            }
        }
    }
}

impl From<FMTP> for String {
    fn from(value: FMTP) -> Self {
        let format_capabilities = value
            .format_capability
            .into_iter()
            .collect::<Vec<String>>()
            .join(";");
        format!("fmtp:{} {}", value.payload_number, format_capabilities)
    }
}

impl From<Candidate> for String {
    fn from(value: Candidate) -> Self {
        format!(
            "candidate:{} {} UDP {} {} {} typ host", //todo Handle other candidate types
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

        println!("sdp_type {}, value {}", sdp_type.len(), value);

        match sdp_type {
            "v" => Ok(SDPLine::ProtocolVersion(value.to_string())),
            "c" => Ok(SDPLine::ConnectionData(ConnectionData::try_from(input)?)),
            "o" => Ok(SDPLine::Originator(Originator::try_from(input)?)),
            "s" => Ok(SDPLine::SessionName(value.to_string())),
            "t" => Ok(SDPLine::SessionTime(SessionTime::try_from(input)?)),
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
        let (_, value) = value.split_once("a=").ok_or(MalformedAttribute)?;
        let key = value.split(":").next().ok_or(MalformedAttribute)?;

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
            "ice-options" => Ok(Attribute::ICEOptions(ICEOptions::try_from(value)?)),
            "end-of-candidates" => Ok(Attribute::EndOfCandidates),
            "setup" => Ok(Attribute::Setup(Setup::try_from(value)?)),
            "rtcp-fb" => Ok(Attribute::Feedback(Feedback::try_from(value)?)),
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

impl TryFrom<&str> for Originator {
    type Error = SDPParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let (_key, value) = value
            .split_once("o=")
            .ok_or(Self::Error::MalformedSDPLine)?;
        let mut split = value.split(" ");

        let username = split.next().ok_or(Self::Error::MalformedSDPLine)?;
        let session_id = split.next().ok_or(Self::Error::MalformedSDPLine)?;
        let session_version = split.next().ok_or(Self::Error::MalformedSDPLine)?;
        let network_type = split.next().ok_or(Self::Error::MalformedSDPLine)?;

        if network_type.ne("IN") {
            return Err(Self::Error::MalformedSDPLine);
        }

        let ip_type = split.next().ok_or(Self::Error::MalformedSDPLine)?;

        match ip_type {
            "IP4" => {
                let unicast_address = split.next().ok_or(Self::Error::MalformedSDPLine)?;
                let ip = Ipv4Addr::from_str(unicast_address)
                    .map_err(|_| Self::Error::MalformedSDPLine)?;
                Ok(Self {
                    username: username.to_string(),
                    session_id: session_id.to_string(),
                    session_version: session_version.to_string(),
                    ip_addr: IpAddr::V4(ip),
                })
            }
            "IP6" => {
                let unicast_address = split.next().ok_or(Self::Error::MalformedSDPLine)?;
                let ip = Ipv6Addr::from_str(unicast_address)
                    .map_err(|_| Self::Error::MalformedSDPLine)?;
                Ok(Self {
                    username: username.to_string(),
                    session_id: session_id.to_string(),
                    session_version: session_version.to_string(),
                    ip_addr: IpAddr::V6(ip),
                })
            }
            _ => Err(Self::Error::MalformedSDPLine),
        }
    }
}

impl TryFrom<&str> for SessionTime {
    type Error = SDPParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let (_, value) = value
            .split_once("t=")
            .ok_or(Self::Error::MalformedSDPLine)?;
        let (start_time, end_time) = value.split_once(" ").ok_or(Self::Error::MalformedSDPLine)?;

        Ok(Self {
            start_time: start_time
                .parse::<usize>()
                .map_err(|_| Self::Error::MalformedSDPLine)?,
            end_time: end_time
                .parse::<usize>()
                .map_err(|_| Self::Error::MalformedSDPLine)?,
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

impl TryFrom<&str> for ICEOptions {
    type Error = SDPParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let (_, value) = value
            .split_once("ice-options:")
            .ok_or(Self::Error::MalformedAttribute)?;

        let ice_options = value
            .split(" ")
            .map(ICEOption::try_from)
            .collect::<Result<Vec<ICEOption>, Self::Error>>()?;

        Ok(Self {
            options: ice_options,
        })
    }
}

impl TryFrom<&str> for ICEOption {
    type Error = SDPParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "ice2" => Ok(ICEOption::ICE2),
            "trickle" => Ok(ICEOption::Trickle),
            _ => Ok(ICEOption::Unsupported),
        }
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

        let (group_type, group_values) = value
            .split_once(" ")
            .ok_or(Self::Error::MalformedAttribute)?;

        let group_values = group_values
            .split(" ")
            .map(ToString::to_string)
            .collect::<Vec<_>>();

        match group_type {
            "BUNDLE" => Ok(MediaGroup::Bundle(group_values)),
            "LS" => Ok(MediaGroup::LipSync(group_values)),
            _ => Err(Self::Error::MalformedAttribute),
        }
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

impl TryFrom<&str> for Feedback {
    type Error = SDPParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let (_, value) = value
            .split_once("rtcp-fb:")
            .ok_or(Self::Error::MalformedAttribute)?;
        let (payload_type, feedback_type) = value
            .split_once(" ")
            .ok_or(SDPParseError::MalformedAttribute)?;
        Ok(Feedback {
            payload_type: payload_type
                .parse::<usize>()
                .or(Err(SDPParseError::MalformedAttribute))?,
            feedback_type: match feedback_type {
                "nack" => FeedbackType::NACK,
                "nack pli" => FeedbackType::PLI,
                _ => FeedbackType::Unsupported,
            },
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

        let mut split = value.split(" ");

        let ssrc = split
            .next()
            .ok_or(SDPParseError::MalformedAttribute)?
            .parse::<u32>()
            .map_err(|_| Self::Error::MalformedAttribute)?;
        let attribute = split.next().ok_or(SDPParseError::MalformedAttribute)?;

        Ok(MediaSSRC {
            ssrc,
            source_attribute: SourceAttribute::try_from(attribute)?,
        })
    }
}

impl TryFrom<&str> for SourceAttribute {
    type Error = SDPParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let mut split = value.split(":");

        let key = split.next().ok_or(Self::Error::MalformedAttribute)?;

        match key {
            "cname" => {
                let cname_value = split.next().ok_or(MalformedAttribute)?.to_string();
                Ok(Self::CNAME(cname_value))
            }
            _ => Ok(Self::Unsupported),
        }
    }
}

impl TryFrom<&str> for Setup {
    type Error = SDPParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let (_, value) = value
            .split_once("setup:")
            .ok_or(Self::Error::MalformedAttribute)?;

        match value {
            "actpass" => Ok(Self::ActivePassive),
            "active" => Ok(Self::Active),
            "passive" => Ok(Self::Passive),
            _ => Err(Self::Error::MalformedAttribute),
        }
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
            .collect::<HashSet<String>>();

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