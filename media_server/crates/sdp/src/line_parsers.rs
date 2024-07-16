use std::net::IpAddr;
use std::str::FromStr;

#[derive(Debug)]
pub enum LineParseError {
    SequenceError,
    MissingICECredentials,
    UnsupportedMediaCount,
    UnsupportedMediaType,
    UnsupportedMediaProtocol,
    MalformedAttribute,
    MalformedMediaDescriptor,
    MalformedSDPLine,
}
#[derive(Debug)]
enum SDPLine {
    ProtocolVersion(String),
    Originator(String),
    SessionName(String),
    SessionTime(String),
    ConnectionData(String),
    Attribute(Attribute),
    MediaDescription(MediaDescription),
    Unrecognized,
}

#[derive(Debug)]
pub(crate) struct SDPOffer {
    pub(crate) ice_username: String,
    pub(crate) ice_password: String,
    pub(crate) audio_media_description: Vec<Attribute>,
    pub(crate) video_media_description: Vec<Attribute>,
}

#[derive(Debug, Clone)]
pub(crate) enum Attribute {
    Unrecognized,
    SendOnly,
    ReceiveOnly,
    MediaID(String),
    ICEUsername(String),
    ICEPassword(String),
    ICEOptions(String),
    Fingerprint(Fingerprint),
    MediaGroup(String),
    MediaSSRC(MediaSSRC),
    RTCPMux,
    RTPMap(RTPMap),
    FMTP(FMTP),
    Candidate(Candidate),
}
#[derive(Debug)]
pub(crate) struct MediaDescription {
    media_type: MediaType,
    transport_port: usize,
    transport_protocol: MediaTransportProtocol,
    media_format_description: Vec<usize>,
}

#[derive(Debug)]
pub(crate) enum MediaType {
    Video,
    Audio,
}

#[derive(Debug)]
enum MediaTransportProtocol {
    DtlsSrtp,
}

#[derive(Debug, Clone)]
struct Fingerprint {
    hash_function: HashFunction,
    hash: String,
}

#[derive(Debug, Clone)]
enum HashFunction {
    SHA256,
    Unsupported,
}

#[derive(Debug, Clone)]
pub(crate) struct RTPMap {
    pub(crate) codec: MediaCodec,
    pub(crate) payload_number: usize,
}

#[derive(Debug, Clone)]
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
pub(crate) struct FMTP {
    pub(crate) payload_number: usize,
    pub(crate) format_capability: Vec<String>,
}

#[derive(Debug, Clone)]
struct Candidate {
    foundation: String,
    component_id: usize,
    priority: usize,
    connection_address: IpAddr,
    port: usize,
}

pub fn parse_raw_sdp_offer(data: &str) -> Result<SDPOffer, LineParseError> {
    let sdp_lines = data
        .lines()
        .map(parse_sdp_line)
        .collect::<Result<Vec<SDPLine>, LineParseError>>()?;

    let mut iter = sdp_lines.iter();

    // Check if session description segment is properly formatted.
    let protocol_version = iter.next().ok_or(LineParseError::MalformedSDPLine)?;
    if !matches!(protocol_version, SDPLine::ProtocolVersion(_)) {
        return Err(LineParseError::SequenceError);
    }

    let originator = iter.next().ok_or(LineParseError::MalformedSDPLine)?;
    if !matches!(originator, SDPLine::Originator(_)) {
        return Err(LineParseError::SequenceError);
    }

    let session_name = iter.next().ok_or(LineParseError::MalformedSDPLine)?;
    if !matches!(session_name, SDPLine::SessionName(_)) {
        return Err(LineParseError::SequenceError);
    }

    let session_time = iter.next().ok_or(LineParseError::MalformedSDPLine)?;
    if !matches!(session_time, SDPLine::SessionTime(_)) {
        return Err(LineParseError::SequenceError);
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
        .ok_or(LineParseError::MissingICECredentials)?
        .to_string();
    let ice_password = sdp_lines
        .iter()
        .find_map(|line| {
            if let SDPLine::Attribute(Attribute::ICEPassword(username)) = line {
                return Some(username);
            }
            None
        })
        .ok_or(LineParseError::MissingICECredentials)?
        .to_string();

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
        return Err(LineParseError::UnsupportedMediaCount);
    }

    let first_media_line = media_descriptors_iter
        .next()
        .map(|line| match line {
            SDPLine::MediaDescription(media_description) => media_description,
            _ => unreachable!(
                "The first item after session description end should always be media description"
            ),
        })
        .ok_or(LineParseError::MalformedMediaDescriptor)?;

    // First media descriptor must be Audio. This is an arbitrary decision to ease implementation.
    if !matches!(first_media_line.media_type, MediaType::Audio) {
        return Err(LineParseError::SequenceError);
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
        return Err(LineParseError::SequenceError);
    }

    let video_media_attributes = second_media_description_segment
        .filter_map(|line| match line {
            SDPLine::Attribute(attr) => Some(attr.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();

    Ok(SDPOffer {
        ice_username,
        ice_password,
        audio_media_description: audio_media_attributes,
        video_media_description: video_media_attributes,
    })
}

fn parse_sdp_line(line: &str) -> Result<SDPLine, LineParseError> {
    let (sdp_type, value) = line
        .split_once("=")
        .ok_or(LineParseError::MalformedSDPLine)?;
    match sdp_type {
        "v" => Ok(SDPLine::ProtocolVersion(value.to_string())),
        "o" => Ok(SDPLine::Originator(value.to_string())),
        "s" => Ok(SDPLine::SessionName(value.to_string())),
        "t" => Ok(SDPLine::SessionTime(value.to_string())),
        "m" => {
            let media_descriptor = parse_media_descriptor(value)?;
            Ok(SDPLine::MediaDescription(media_descriptor))
        }
        "a" => {
            let attribute = parse_attribute(value)?;

            Ok(SDPLine::Attribute(attribute))
        }
        _ => Ok(SDPLine::Unrecognized),
    }
}

fn parse_attribute(attribute: &str) -> Result<Attribute, LineParseError> {
    let (key, value) = attribute
        .split_once(":")
        .map(|(key, value)| (key, Some(value.to_string())))
        .unwrap_or((attribute, None));

    match key {
        "ice-ufrag" => {
            let value = value.ok_or(LineParseError::MalformedAttribute)?;
            Ok(Attribute::ICEUsername(value))
        }
        "ice-pwd" => {
            let value = value.ok_or(LineParseError::MalformedAttribute)?;
            Ok(Attribute::ICEPassword(value))
        }
        "ice-options" => {
            let value = value.ok_or(LineParseError::MalformedAttribute)?;
            Ok(Attribute::ICEOptions(value))
        }
        "fingerprint" => {
            let value = value.ok_or(LineParseError::MalformedAttribute)?;
            Ok(Attribute::Fingerprint(parse_fingerprint(&value)?))
        }
        "candidate" => {
            let value = value.ok_or(LineParseError::MalformedAttribute)?;
            Ok(Attribute::Candidate(parse_candidate(&value)?))
        }
        "ssrc" => {
            let value = value.ok_or(LineParseError::MalformedAttribute)?;
            Ok(Attribute::MediaSSRC(parse_ssrc_attribute(&value)?))
        }
        "sendonly" => Ok(Attribute::SendOnly),
        "recvonly" => Ok(Attribute::ReceiveOnly),
        "mid" => {
            let value = value.ok_or(LineParseError::MalformedAttribute)?;
            Ok(Attribute::MediaID(value))
        }
        "group" => {
            let value = value.ok_or(LineParseError::MalformedAttribute)?;
            Ok(Attribute::MediaGroup(value))
        }
        "rtpmap" => {
            let value = value
                .ok_or(LineParseError::MalformedAttribute)
                .and_then(|val| parse_rtpmap(&val))?;
            Ok(Attribute::RTPMap(value))
        }
        "fmtp" => {
            let value = value
                .ok_or(LineParseError::MalformedAttribute)
                .and_then(|val| parse_fmtp(&val))?;
            Ok(Attribute::FMTP(value))
        }
        "rtcp-mux" => Ok(Attribute::RTCPMux),
        _ => Ok(Attribute::Unrecognized),
    }
}

fn parse_media_descriptor(descriptor: &str) -> Result<MediaDescription, LineParseError> {
    let mut split = descriptor.split(" ");

    let media_type = split
        .next()
        .ok_or(LineParseError::MalformedMediaDescriptor)
        .and_then(|media_type| match media_type {
            "video" => Ok(MediaType::Video),
            "audio" => Ok(MediaType::Audio),
            _ => Err(LineParseError::UnsupportedMediaType),
        })?;

    let transport_port = split
        .next()
        .and_then(|port| port.parse::<usize>().ok())
        .ok_or(LineParseError::MalformedMediaDescriptor)?;

    let transport_protocol = split
        .next()
        .ok_or(LineParseError::MalformedMediaDescriptor)
        .and_then(|transport_protocol| match transport_protocol {
            "UDP/TLS/RTP/SAVPF" => Ok(MediaTransportProtocol::DtlsSrtp),
            _ => Err(LineParseError::UnsupportedMediaProtocol),
        })?;

    let media_format_description = split
        .take_while(|line| !line.is_empty())
        .map(|line| line.parse::<usize>().ok())
        .collect::<Option<Vec<usize>>>()
        .ok_or(LineParseError::MalformedAttribute)?;

    Ok(MediaDescription {
        transport_port,
        media_type,
        media_format_description,
        transport_protocol,
    })
}

fn parse_ssrc_attribute(input: &str) -> Result<MediaSSRC, LineParseError> {
    let ssrc = input
        .split(" ")
        .next()
        .ok_or(LineParseError::MalformedSDPLine)?;

    Ok(MediaSSRC {
        ssrc: ssrc.to_string(),
    })
}

fn parse_fingerprint(input: &str) -> Result<Fingerprint, LineParseError> {
    let (hash_function, hash) = input
        .split_once(" ")
        .ok_or(LineParseError::MalformedAttribute)?;

    let hash_function = match hash_function {
        "sha-256" => HashFunction::SHA256,
        _ => HashFunction::Unsupported,
    };

    Ok(Fingerprint {
        hash_function,
        hash: hash.to_string(),
    })
}

fn parse_rtpmap(input: &str) -> Result<RTPMap, LineParseError> {
    let (payload_number, codec) = input
        .split_once(" ")
        .ok_or(LineParseError::MalformedAttribute)?;

    let payload_number = payload_number
        .parse::<usize>()
        .map_err(|_| LineParseError::MalformedAttribute)?;

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

fn parse_fmtp(input: &str) -> Result<FMTP, LineParseError> {
    let (payload_number, capabilities) = input
        .split_once(" ")
        .ok_or(LineParseError::MalformedAttribute)?;

    let payload_number = payload_number
        .parse::<usize>()
        .map_err(|_| LineParseError::MalformedAttribute)?;

    let format_capability = capabilities
        .split(";")
        .map(ToString::to_string)
        .collect::<Vec<String>>();

    Ok(FMTP {
        format_capability,
        payload_number,
    })
}

fn parse_candidate(input: &str) -> Result<Candidate, LineParseError> {
    let mut split = input.split(" ");
    let foundation = split
        .next()
        .ok_or(LineParseError::MalformedAttribute)?
        .to_string();
    let component_id = split
        .next()
        .ok_or(LineParseError::MalformedAttribute)
        .map(|id| id.parse::<usize>())?
        .map_err(|_| LineParseError::MalformedAttribute)?;

    let protocol = split.next().ok_or(LineParseError::MalformedAttribute)?;

    if !protocol.eq("UDP") {
        return Err(LineParseError::MalformedAttribute);
    }

    let priority = split
        .next()
        .ok_or(LineParseError::MalformedSDPLine)?
        .parse::<usize>()
        .map_err(|_| LineParseError::MalformedSDPLine)?;

    let ip = split
        .next()
        .ok_or(LineParseError::MalformedAttribute)
        .and_then(|ip| IpAddr::from_str(ip).map_err(|_| LineParseError::MalformedAttribute))?;

    let port = split
        .next()
        .ok_or(LineParseError::MalformedSDPLine)?
        .parse::<usize>()
        .map_err(|_| LineParseError::MalformedSDPLine)?;

    Ok(Candidate {
        component_id,
        foundation,
        connection_address: ip,
        port,
        priority,
    })
}

#[cfg(test)]
mod tests {

    mod parse_fmtp {
        use crate::line_parsers::parse_fmtp;

        #[test]
        fn rejects_malformed_line() {
            let attr = "96-profile-level-id other-attributes:1";
            let parse_result = parse_fmtp(attr);
            assert!(parse_result.is_err(), "Should reject FMTP attribute")
        }

        #[test]
        fn resolves_all_capabilities() {
            let attr = "96 profile-level-id=42e01f;packetization-mode=1;level-asymmetry-allowed=1";
            let fmtp = parse_fmtp(attr).expect("FMTP should be OK");
            assert_eq!(
                fmtp.payload_number, 96,
                "Should resolve correct payload number"
            );

            assert_eq!(
                fmtp.format_capability,
                vec![
                    "profile-level-id=42e01f",
                    "packetization-mode=1",
                    "level-asymmetry-allowed=1"
                ]
            )
        }
    }

    mod parse_media_descriptor {
        use crate::line_parsers::{
            LineParseError, MediaTransportProtocol, MediaType, parse_media_descriptor,
        };

        #[test]
        fn rejects_unsupported_media_type() {
            let media_descriptor = "text 52000 UDP 96";

            let parse_error =
                parse_media_descriptor(media_descriptor).expect_err("Should fail to parse");
            assert!(
                matches!(parse_error, LineParseError::UnsupportedMediaType),
                "Should reject with UnsupportedMediaType error"
            )
        }

        #[test]
        fn rejects_unsupported_media_transport_protocol() {
            let media_descriptor = "video 52000 UDP 96";

            let parse_error =
                parse_media_descriptor(media_descriptor).expect_err("Should fail to parse");
            assert!(
                matches!(parse_error, LineParseError::UnsupportedMediaProtocol),
                "Should reject with UnsupportedMediaType error"
            )
        }

        #[test]
        fn resolves_supported_media_with_single_payload_number() {
            let media_descriptor = "video 52000 UDP/TLS/RTP/SAVPF 96";

            let media =
                parse_media_descriptor(media_descriptor).expect("Should parse to MediaDescription");
            assert!(
                matches!(media.media_type, MediaType::Video),
                "Should resolve media_type to Video"
            );

            assert!(
                matches!(media.transport_protocol, MediaTransportProtocol::DtlsSrtp),
                "Should resolve transport_protocol to DTLS_RTP"
            );

            assert_eq!(
                media.transport_port, 52000,
                "Should resolve transport port to 52000"
            );
            assert_eq!(
                media.media_format_description,
                vec![96],
                "Should resolve to single payload number: 96"
            )
        }

        #[test]
        fn resolves_supported_media_with_multiple_payload_numbers() {
            let media_descriptor = "video 52000 UDP/TLS/RTP/SAVPF 96 102 112";

            let media =
                parse_media_descriptor(media_descriptor).expect("Should parse to MediaDescription");
            assert!(
                matches!(media.media_type, MediaType::Video),
                "Should resolve media_type to Video"
            );

            assert!(
                matches!(media.transport_protocol, MediaTransportProtocol::DtlsSrtp),
                "Should resolve transport_protocol to DTLS_RTP"
            );

            assert_eq!(
                media.transport_port, 52000,
                "Should resolve transport port to 52000"
            );
            assert_eq!(
                media.media_format_description,
                vec![96, 102, 112],
                "Should resolve to multiple payload numbers: 96, 102, 112"
            )
        }
    }
    mod parse_rtpmap {
        use crate::line_parsers::{MediaCodec, parse_rtpmap, VideoCodec};

        #[test]
        fn recognizes_unsupported_codec() {
            let rtp_attr = "96 myCodec";

            let rtp_map = parse_rtpmap(rtp_attr).expect("Should parse to RTPMap");

            assert_eq!(rtp_map.payload_number, 96, "Payload number should match");
            assert!(
                matches!(rtp_map.codec, MediaCodec::Unsupported),
                "Codec should be unsupported"
            )
        }

        #[test]
        fn rejects_malformed_attribute() {
            let rtp_attr = "96-myCodec";

            let rtp_map = parse_rtpmap(rtp_attr);

            assert!(rtp_map.is_err(), "Should reject attribute parse")
        }

        #[test]
        fn accepts_lowercase_video_codec() {
            let rtp_attr = "96 h264/90000";

            let rtp_map = parse_rtpmap(rtp_attr).expect("Should parse to RTPMap");

            assert_eq!(rtp_map.payload_number, 96, "Payload number should match");
            assert!(
                matches!(rtp_map.codec, MediaCodec::Video(VideoCodec::H264)),
                "Codec should be H264"
            )
        }

        #[test]
        fn accepts_uppercase_video_codec() {
            let rtp_attr = "96 H264/90000";

            let rtp_map = parse_rtpmap(rtp_attr).expect("Should parse to RTPMap");

            assert_eq!(rtp_map.payload_number, 96, "Payload number should match");
            assert!(
                matches!(rtp_map.codec, MediaCodec::Video(VideoCodec::H264)),
                "Codec should be H264"
            )
        }
    }

    mod parse_fingerprint {
        use crate::line_parsers::{HashFunction, parse_fingerprint};

        #[test]
        fn recognizes_unsupported_fingerprint() {
            let unsupported_fingerprint = "sha-test EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B";
            let sdp_parse = parse_fingerprint(unsupported_fingerprint);
            let fingerprint = sdp_parse.expect("Fingerprint parse result should be OK");

            assert_eq!(fingerprint.hash, "EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B", "Hash should match");
            assert!(
                matches!(fingerprint.hash_function, HashFunction::Unsupported),
                "HashFunction should be Unsupported"
            )
        }

        #[test]
        fn fails_on_malformed_attribute() {
            let unsupported_fingerprint = "sha-1,EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B";
            let sdp_parse = parse_fingerprint(unsupported_fingerprint);
            assert!(sdp_parse.is_err(), "Should return Err");
        }

        #[test]
        fn recognizes_sha_256() {
            let unsupported_fingerprint = "sha-256 EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B";
            let sdp_parse = parse_fingerprint(unsupported_fingerprint);

            let fingerprint = sdp_parse.expect("Fingerprint parse result should be OK");

            assert_eq!(fingerprint.hash, "EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B", "Hash should match");
            assert!(
                matches!(fingerprint.hash_function, HashFunction::SHA256),
                "HashFunction should be SHA256"
            )
        }
    }
}
const EXAMPLE_SDP: &str = "v=0\r\no=rtc 3767197920 0 IN IP4 127.0.0.1\r\ns=-\r\nt=0 0\r\na=group:BUNDLE 0 1\r\na=group:LS 0 1\r\na=msid-semantic:WMS *\r\na=setup:actpass\r\na=ice-ufrag:E2Fr\r\na=ice-pwd:OpQzg1PAwUdeOB244chlgd\r\na=ice-options:trickle\r\na=fingerprint:sha-256 EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B\r\nm=audio 4557 UDP/TLS/RTP/SAVPF 111\r\nc=IN IP4 192.168.0.198\r\na=mid:0\r\na=sendonly\r\na=ssrc:1349455989 cname:0X2NGAsK9XcmnsuZ\r\na=ssrc:1349455989 msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-audio\r\na=msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-audio\r\na=rtcp-mux\r\na=rtpmap:111 opus/48000/2\r\na=fmtp:111 minptime=10;maxaveragebitrate=96000;stereo=1;sprop-stereo=1;useinbandfec=1\r\na=candidate:1 1 UDP 2015363327 192.168.0.198 4557 typ host\r\na=candidate:2 1 UDP 2015363583 fe80::6c3d:5b42:1532:2f9a 10007 typ host\r\na=end-of-candidates\r\nm=video 4557 UDP/TLS/RTP/SAVPF 96\r\nc=IN IP4 192.168.0.198\r\na=mid:1\r\na=sendonly\r\na=ssrc:1349455990 cname:0X2NGAsK9XcmnsuZ\r\na=ssrc:1349455990 msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-video\r\na=msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-video\r\na=rtcp-mux\r\na=rtpmap:96 H264/90000\r\na=rtcp-fb:96 nack\r\na=rtcp-fb:96 nack pli\r\na=rtcp-fb:96 goog-remb\r\na=fmtp:96 profile-level-id=42e01f;packetization-mode=1;level-asymmetry-allowed=1\r\n";
