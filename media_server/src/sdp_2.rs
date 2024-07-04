use core::str;
use std::io::BufRead;

fn parse_raw_sdp_to_sdp_lines(data: &str) -> Option<Vec<SDPLine>> {
    let mut sdp_lines: Vec<SDPLine> = vec![];

    for line in data.lines() {
        let (sdp_type, value) = line.split_once("=")?;
        match sdp_type {
            "v" => sdp_lines.push(SDPLine::ProtocolVersion(value.to_string())),
            "o" => sdp_lines.push(SDPLine::Originator(value.to_string())),
            "s" => sdp_lines.push(SDPLine::SessionName(value.to_string())),
            "t" => sdp_lines.push(SDPLine::SessionTime(value.to_string())),
            "m" => {
                let media_descriptor = parse_media_descriptor(value)?;
                sdp_lines.push(SDPLine::MediaDescription(media_descriptor))
            }
            _ => sdp_lines.push(SDPLine::Unrecognized),
        }
    }

    Some(sdp_lines)
}

fn parse_media_descriptor(descriptor: &str) -> Option<MediaDescription> {
    let mut split = descriptor.split(" ");

    let media_type = match split.next()? {
        "audio" => Some(MediaType::Audio),
        "video" => Some(MediaType::Video),
        _ => None,
    }?;

    let transport_port = split.next().and_then(|port| port.parse::<usize>().ok())?;

    let transport_protocol = match split.next()? {
        "UDP/TLS/RTP/SAVPF" => Some(TransportProtocol::DTLS_SRTP),
        _ => None,
    }?;

    let media_format_description = split
        .take_while(|line| !line.is_empty())
        .map(|line| line.parse::<usize>().ok())
        .collect::<Option<Vec<usize>>>()?;

    Some(MediaDescription {
        transport_port,
        media_type,
        media_format_description,
        transport_protocol,
    })
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
enum Attribute {
    Unrecognized(KeyValue),
    SendOnly(KeyValue),
    ReceiveOnly(KeyValue),
    MediaID(KeyValue),
    ICEUsername(KeyValue),
    ICEPassword(KeyValue),
    ICEOptions(KeyValue),
    Fingerprint(KeyValue),
    MediaGroup(KeyValue),
    MediaSSRC(MediaSSRC),
    RTCPMux(KeyValue),
    RTPMap(RTPMap),
    FMTP(FMTP),
    Candidate(Candidate),
    RTCPFeedback(RTCPFeedback),
}
#[derive(Debug)]
struct MediaDescription {
    media_type: MediaType,
    transport_port: usize,
    transport_protocol: TransportProtocol,
    media_format_description: Vec<usize>,
}

#[derive(Debug)]
enum TransportProtocol {
    DTLS_SRTP,
}

#[derive(Debug)]
enum MediaType {
    Audio,
    Video,
}
#[derive(Debug)]
struct KeyValue {
    key: String,
    value: String,
}

impl KeyValue {
    pub fn new(key: &str, value: &str) -> Self {
        KeyValue {
            key: key.to_string(),
            value: value.to_string(),
        }
    }
}

#[derive(Debug)]
struct RTPMap {}

#[derive(Debug)]
struct MediaSSRC {}

#[derive(Debug)]
struct FMTP {}

#[derive(Debug)]
struct Candidate {}

#[derive(Debug)]
enum RTCPFeedback {}
const EXAMPLE_SDP: &str = "v=0\r\no=rtc 3767197920 0 IN IP4 127.0.0.1\r\ns=-\r\nt=0 0\r\na=group:BUNDLE 0 1\r\na=group:LS 0 1\r\na=msid-semantic:WMS *\r\na=setup:actpass\r\na=ice-ufrag:E2Fr\r\na=ice-pwd:OpQzg1PAwUdeOB244chlgd\r\na=ice-options:trickle\r\na=fingerprint:sha-256 EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B\r\nm=audio 4557 UDP/TLS/RTP/SAVPF 111\r\nc=IN IP4 192.168.0.198\r\na=mid:0\r\na=sendonly\r\na=ssrc:1349455989 cname:0X2NGAsK9XcmnsuZ\r\na=ssrc:1349455989 msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-audio\r\na=msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-audio\r\na=rtcp-mux\r\na=rtpmap:111 opus/48000/2\r\na=fmtp:111 minptime=10;maxaveragebitrate=96000;stereo=1;sprop-stereo=1;useinbandfec=1\r\na=candidate:1 1 UDP 2015363327 192.168.0.198 4557 typ host\r\na=candidate:2 1 UDP 2015363583 fe80::6c3d:5b42:1532:2f9a 10007 typ host\r\na=end-of-candidates\r\nm=video 4557 UDP/TLS/RTP/SAVPF 96\r\nc=IN IP4 192.168.0.198\r\na=mid:1\r\na=sendonly\r\na=ssrc:1349455990 cname:0X2NGAsK9XcmnsuZ\r\na=ssrc:1349455990 msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-video\r\na=msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-video\r\na=rtcp-mux\r\na=rtpmap:96 H264/90000\r\na=rtcp-fb:96 nack\r\na=rtcp-fb:96 nack pli\r\na=rtcp-fb:96 goog-remb\r\na=fmtp:96 profile-level-id=42e01f;packetization-mode=1;level-asymmetry-allowed=1\r\n";

#[cfg(test)]
mod tests {
    use crate::sdp_2::{EXAMPLE_SDP, parse_raw_sdp_to_sdp_lines};

    #[test]
    fn parses_all_attributes() {
        let sdp_parse = parse_raw_sdp_to_sdp_lines(EXAMPLE_SDP);
        println!("res {:?}", sdp_parse.unwrap())
    }
}
