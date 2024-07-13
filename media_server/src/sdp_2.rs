use core::str;
use std::io::BufRead;
use std::net::IpAddr;
use std::str::FromStr;

fn parse_raw_sdp(data: &str) -> Result<SDP, SDPParseError> {
    let sdp_lines = data
        .lines()
        .map(parse_sdp_line)
        .collect::<Result<Vec<SDPLine>, SDPParseError>>()?;

    let mut iter = sdp_lines.iter();

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

    let ice_username = sdp_lines
        .iter()
        .find_map(|line| {
            if let SDPLine::Attribute(Attribute::ICEUsername(username)) = line {
                return Some(username);
            }
            None
        })
        .ok_or(SDPParseError::MalformedSDPLine)?
        .to_string();
    let ice_password = sdp_lines
        .iter()
        .find_map(|line| {
            if let SDPLine::Attribute(Attribute::ICEPassword(username)) = line {
                return Some(username);
            }
            None
        })
        .ok_or(SDPParseError::MalformedSDPLine)?
        .to_string();

    let mut media_descriptors_iter = sdp_lines
        .iter()
        .skip_while(|line| !matches!(line, SDPLine::MediaDescription(_)));

    let media_descriptor_count = media_descriptors_iter
        .clone()
        .filter(|line| matches!(line, SDPLine::MediaDescription(_)))
        .count();

    if media_descriptor_count != 2 {
        return Err(SDPParseError::UnsupportedMedia);
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
        .ok_or(SDPParseError::MalformedMediaDescriptor)?;

    if !matches!(second_media_line.media_type, MediaType::Video) {
        return Err(SDPParseError::SequenceError);
    }

    let video_media_attributes = second_media_description_segment
        .filter_map(|line| match line {
            SDPLine::Attribute(attr) => Some(attr.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();

    Ok(SDP {
        ice_username,
        ice_password,
        audio_media_description: audio_media_attributes,
        video_media_description: video_media_attributes,
    })
}

fn parse_sdp_line(line: &str) -> Result<SDPLine, SDPParseError> {
    let (sdp_type, value) = line
        .split_once("=")
        .ok_or(SDPParseError::MalformedSDPLine)?;
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

fn parse_attribute(attribute: &str) -> Result<Attribute, SDPParseError> {
    let (key, value) = attribute
        .split_once(":")
        .map(|(key, value)| (key, Some(value.to_string())))
        .unwrap_or((attribute, None));

    match key {
        "ice-ufrag" => {
            let value = value.ok_or(SDPParseError::MalformedAttribute)?;
            Ok(Attribute::ICEUsername(value))
        }
        "ice-pwd" => {
            let value = value.ok_or(SDPParseError::MalformedAttribute)?;
            Ok(Attribute::ICEPassword(value))
        }
        "ice-options" => {
            let value = value.ok_or(SDPParseError::MalformedAttribute)?;
            Ok(Attribute::ICEOptions(value))
        }
        "fingerprint" => {
            let value = value.ok_or(SDPParseError::MalformedAttribute)?;
            Ok(Attribute::Fingerprint(parse_fingerprint(&value)?))
        }
        "candidate" => {
            let value = value.ok_or(SDPParseError::MalformedAttribute)?;
            Ok(Attribute::Candidate(parse_candidate(&value)?))
        }
        "ssrc" => {
            let value = value.ok_or(SDPParseError::MalformedAttribute)?;
            Ok(Attribute::MediaSSRC(parse_ssrc_attribute(&value)?))
        }
        "sendonly" => Ok(Attribute::SendOnly),
        "recvonly" => Ok(Attribute::ReceiveOnly),
        "mid" => {
            let value = value.ok_or(SDPParseError::MalformedAttribute)?;
            Ok(Attribute::MediaID(value))
        }
        "group" => {
            let value = value.ok_or(SDPParseError::MalformedAttribute)?;
            Ok(Attribute::MediaGroup(value))
        }
        "rtpmap" => {
            let value = value
                .ok_or(SDPParseError::MalformedAttribute)
                .and_then(|val| parse_rtpmap(&val))?;
            Ok(Attribute::RTPMap(value))
        }
        "fmtp" => {
            let value = value
                .ok_or(SDPParseError::MalformedAttribute)
                .and_then(|val| parse_fmtp(&val))?;
            Ok(Attribute::FMTP(value))
        }
        "rtcp-mux" => Ok(Attribute::RTCPMux),
        _ => Ok(Attribute::Unrecognized),
    }
}

fn parse_media_descriptor(descriptor: &str) -> Result<MediaDescription, SDPParseError> {
    let mut split = descriptor.split(" ");

    let media_type = split
        .next()
        .ok_or(SDPParseError::MalformedMediaDescriptor)
        .map(|media_type| match media_type {
            "video" => MediaType::Video,
            "audio" => MediaType::Audio,
            _ => MediaType::Unsupported,
        })?;

    let transport_port = split
        .next()
        .and_then(|port| port.parse::<usize>().ok())
        .ok_or(SDPParseError::MalformedMediaDescriptor)?;

    let transport_protocol = split
        .next()
        .ok_or(SDPParseError::MalformedMediaDescriptor)
        .map(|transport_protocol| match transport_protocol {
            "UDP/TLS/RTP/SAVPF" => MediaTransportProtocol::DTLS_SRTP,
            _ => MediaTransportProtocol::Unsupported,
        })?;

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

fn parse_ssrc_attribute(input: &str) -> Result<MediaSSRC, SDPParseError> {
    let ssrc = input
        .split(" ")
        .next()
        .ok_or(SDPParseError::MalformedSDPLine)?;

    Ok(MediaSSRC {
        ssrc: ssrc.to_string(),
    })
}

fn parse_fingerprint(input: &str) -> Result<Fingerprint, SDPParseError> {
    let (hash_function, hash) = input
        .split_once(" ")
        .ok_or(SDPParseError::MalformedAttribute)?;

    let hash_function = match hash_function {
        "sha-256" => HashFunction::SHA256,
        _ => HashFunction::Unsupported,
    };

    Ok(Fingerprint {
        hash_function,
        hash: hash.to_string(),
    })
}

fn parse_rtpmap(input: &str) -> Result<RTPMap, SDPParseError> {
    let (payload_number, codec) = input
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

fn parse_fmtp(input: &str) -> Result<FMTP, SDPParseError> {
    let (payload_number, capabilities) = input
        .split_once(" ")
        .ok_or(SDPParseError::MalformedAttribute)?;

    let payload_number = payload_number
        .parse::<usize>()
        .map_err(|_| SDPParseError::MalformedAttribute)?;

    Ok(FMTP {
        format_capability: capabilities.to_string(),
        payload_number,
    })
}

fn parse_candidate(input: &str) -> Result<Candidate, SDPParseError> {
    let mut split = input.split(" ");
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
        .parse::<usize>()
        .map_err(|_| SDPParseError::MalformedSDPLine)?;

    Ok(Candidate {
        component_id,
        foundation,
        connection_address: ip,
        port,
        priority,
    })
}

#[derive(Debug)]
struct SDP {
    ice_username: String,
    ice_password: String,
    audio_media_description: Vec<Attribute>,
    video_media_description: Vec<Attribute>,
}

struct StreamerSDP {
    ice_username: String,
    ice_password: String,
    video_codec: VideoCodec,
    audio_codec: AudioCodec,
    video_ssrc: MediaSSRC,
    audio_ssrc: MediaSSRC,
    audio_capability: FMTP,
    video_capability: FMTP,
}

struct ViewerSDP {
    ice_username: String,
    ice_password: String,
    resolved_video_payload_number: usize,
    resolved_audio_payload_number: usize,
}

#[derive(Debug)]
enum SDPParseError {
    SequenceError,
    UnsupportedMedia,
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

#[derive(Debug, Clone)]
enum Attribute {
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
struct MediaDescription {
    media_type: MediaType,
    transport_port: usize,
    transport_protocol: MediaTransportProtocol,
    media_format_description: Vec<usize>,
}

#[derive(Debug)]
enum MediaType {
    Video,
    Audio,
    Unsupported,
}

#[derive(Debug)]
enum MediaTransportProtocol {
    DTLS_SRTP,
    Unsupported,
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
struct RTPMap {
    codec: MediaCodec,
    payload_number: usize,
}

#[derive(Debug, Clone)]
enum MediaCodec {
    Audio(AudioCodec),
    Video(VideoCodec),
    Unsupported,
}
#[derive(Debug, Clone)]

enum VideoCodec {
    H264,
}
#[derive(Debug, Clone)]
enum AudioCodec {
    Opus,
}

#[derive(Debug, Clone)]
struct MediaSSRC {
    ssrc: String,
}

#[derive(Debug, Clone)]
struct FMTP {
    payload_number: usize,
    format_capability: String,
}

#[derive(Debug, Clone)]
struct Candidate {
    foundation: String,
    component_id: usize,
    priority: usize,
    connection_address: IpAddr,
    port: usize,
}

const EXAMPLE_SDP: &str = "v=0\r\no=rtc 3767197920 0 IN IP4 127.0.0.1\r\ns=-\r\nt=0 0\r\na=group:BUNDLE 0 1\r\na=group:LS 0 1\r\na=msid-semantic:WMS *\r\na=setup:actpass\r\na=ice-ufrag:E2Fr\r\na=ice-pwd:OpQzg1PAwUdeOB244chlgd\r\na=ice-options:trickle\r\na=fingerprint:sha-256 EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B\r\nm=audio 4557 UDP/TLS/RTP/SAVPF 111\r\nc=IN IP4 192.168.0.198\r\na=mid:0\r\na=sendonly\r\na=ssrc:1349455989 cname:0X2NGAsK9XcmnsuZ\r\na=ssrc:1349455989 msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-audio\r\na=msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-audio\r\na=rtcp-mux\r\na=rtpmap:111 opus/48000/2\r\na=fmtp:111 minptime=10;maxaveragebitrate=96000;stereo=1;sprop-stereo=1;useinbandfec=1\r\na=candidate:1 1 UDP 2015363327 192.168.0.198 4557 typ host\r\na=candidate:2 1 UDP 2015363583 fe80::6c3d:5b42:1532:2f9a 10007 typ host\r\na=end-of-candidates\r\nm=video 4557 UDP/TLS/RTP/SAVPF 96\r\nc=IN IP4 192.168.0.198\r\na=mid:1\r\na=sendonly\r\na=ssrc:1349455990 cname:0X2NGAsK9XcmnsuZ\r\na=ssrc:1349455990 msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-video\r\na=msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-video\r\na=rtcp-mux\r\na=rtpmap:96 H264/90000\r\na=rtcp-fb:96 nack\r\na=rtcp-fb:96 nack pli\r\na=rtcp-fb:96 goog-remb\r\na=fmtp:96 profile-level-id=42e01f;packetization-mode=1;level-asymmetry-allowed=1\r\n";

#[cfg(test)]
mod tests {
    use crate::sdp_2::{EXAMPLE_SDP, parse_raw_sdp};

    #[test]
    fn parses_all_attributes() {
        let sdp_parse = parse_raw_sdp(EXAMPLE_SDP);
        assert!(sdp_parse.is_ok())
    }
}
