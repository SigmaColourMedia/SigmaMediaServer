use crate::ice_registry::SessionCredentials;

pub fn parse_sdp(data: String) -> Option<SDP> {
    println!("{data}");
    let parsed2 = parse_sdp_2(data.clone())?;
    println!("parsed data {:?}", parsed2);
    let mut lines = data.lines();
    let remote_username = lines
        .clone()
        .find(|line| line.starts_with(ICE_USERNAME_ATTRIBUTE_PREFIX))
        .and_then(|line| line.split_once(":").map(|line| line.1.to_owned()))?;
    let remote_password = lines
        .clone()
        .find(|line| line.starts_with(ICE_PASSWORD_ATTRIBUTE_PREFIX))
        .and_then(|line| line.split_once(":").map(|line| line.1.to_owned()))?;
    let bundle = lines
        .clone()
        .find(|line| line.starts_with(GROUP_ATTRIBUTE_PREFIX))
        .and_then(|line| line.split_once(":").map(|line| line.1.to_owned()))?;

    let mut media_lines = lines
        .skip_while(|line| !line.starts_with(MEDIA_LINE_PREFIX))
        .filter(|line| {
            WHITELISTED_ATTRIBUTES
                .iter()
                .any(|item| line.starts_with(item))
        });

    let mut media_descriptors = vec![vec![]];
    let mut media_index = 0;
    media_descriptors[media_index].push(media_lines.next()?);
    while let Some(line) = media_lines.next() {
        if line.starts_with("m=") {
            media_index += 1;
            media_descriptors.push(vec![])
        }
        media_descriptors[media_index].push(line)
    }

    let media_descriptors = media_descriptors
        .into_iter()
        .map(|descriptor| {
            let mut iterator = descriptor.into_iter();
            let media_attribute: Vec<&str> = iterator.next()?.splitn(4, " ").collect();
            let media_type = media_attribute[0].split_once("=")?.1.to_owned();

            let attributes = iterator.map(|str| str.to_owned()).collect::<Vec<String>>();
            Some(MediaDescription {
                media_type,
                protocol: media_attribute[2].to_owned(),
                format: media_attribute[3].to_owned(),
                attributes,
            })
        })
        .collect::<Option<Vec<MediaDescription>>>()?;

    Some(SDP {
        ice_pwd: remote_password,
        ice_username: remote_username,
        group: bundle,
        media_descriptions: media_descriptors,
    })
}

// todo handle unwraps
pub fn create_sdp_receive_answer(
    sdp: &SDP,
    credentials: &SessionCredentials,
    fingerprint: &str,
) -> String {
    let SDP {
        group,
        media_descriptions,
        ..
    } = &sdp;
    let SessionCredentials {
        host_password,
        host_username,
        ..
    } = &credentials;

    let session_description = format!(
        "v=0\r\n\
o=sigma 2616320411 0 IN IP4 127.0.0.1\r\n\
s=-\r\n\
t=0 0\r\n\
a=group:{group}\r\n\
a=setup:passive\r\n\
a=ice-ufrag:{host_username}\r\n\
a=ice-pwd:{host_password}\r\n\
a=ice-options:ice2\r\n\
a=ice-lite\r\n\
a=msid-semantic: WMS *\r\n\
a=fingerprint:sha-256 {fingerprint}\r\n"
    );

    let media_description = media_descriptions
        .into_iter()
        .map(|media| match &media.media_type[..] {
            "audio" => {
                let audio_codec = media
                    .attributes
                    .iter()
                    .find(|line| line.to_ascii_lowercase().ends_with("opus/48000/2"))
                    .unwrap();
                let payload_number = audio_codec
                    .split(" ")
                    .next()
                    .unwrap()
                    .split(":")
                    .nth(1)
                    .unwrap();
                let media_header = format!(
                    "m=audio 52000 {proto} {fmt}\r\n\
a=rtcp:52000 IN IP4 127.0.0.1\r\n\
                c=IN IP4 127.0.0.1\r\n\
a=recvonly\r\n\
                a=candidate:1 1 UDP 2122317823 192.168.0.157 52000 typ host\r\n\
                a=end-of-candidates\r\n\
                {codec}\r\n\
                a=mid:0\r\n\
                a=rtcp-mux\r\n\
                a=maxptime:60",
                    proto = media.protocol,
                    fmt = payload_number,
                    codec = audio_codec
                );
                media_header
            }
            "video" => {
                let video_codec = media
                    .attributes
                    .iter()
                    .find(|line| line.to_ascii_lowercase().ends_with("h264/90000"))
                    .unwrap();
                let payload_number = video_codec
                    .split(" ")
                    .next()
                    .unwrap()
                    .split(":")
                    .nth(1)
                    .unwrap();
                let msid = media
                    .attributes
                    .iter()
                    .find(|line| line.starts_with("a=msid:"))
                    .unwrap();
                let rtcp_lines = media
                    .attributes
                    .iter()
                    .filter(|line| line.starts_with(&format!("a=rtcp-fb:{payload_number}")))
                    .collect::<Vec<&String>>()
                    .iter()
                    .map(|&line| line.to_owned())
                    .collect::<Vec<String>>()
                    .join("\r\n");
                let media_header = format!(
                    "m=video 52000 {proto} {fmt}\r\n\
                a=rtcp:52000 IN IP4 127.0.0.1\r\n\
                c=IN IP4 127.0.0.1\r\n\
a=recvonly\r\n\
                a=mid:1\r\n\
                {codec}\r\n\
                a=rtcp-mux\r\n\
                {msid}\r\n\
a=maxptime:60\r\n\
                {rtcp_lines}",
                    proto = media.protocol,
                    fmt = payload_number,
                    codec = video_codec
                );
                media_header
            }
            _ => panic!("Unrecognized media type"),
        })
        .collect::<Vec<String>>()
        .join("\r\n");

    session_description + &media_description + "\r\n"
}

#[derive(Debug)]
pub struct SDP {
    pub ice_username: String,
    pub ice_pwd: String,
    pub group: String,
    pub media_descriptions: Vec<MediaDescription>,
}

#[derive(Debug)]
pub struct MediaDescription {
    media_type: String,
    protocol: String,
    format: String,
    attributes: Vec<String>,
}

// **** New implementation *****

fn parse_sdp_2(data: String) -> Option<SDPD2> {
    let mut lines = data.lines();
    let remote_username = lines
        .clone()
        .find(|line| line.starts_with(ICE_USERNAME_ATTRIBUTE_PREFIX))
        .and_then(|line| line.split_once(":").map(|line| line.1.to_owned()))?;
    let remote_password = lines
        .clone()
        .find(|line| line.starts_with(ICE_PASSWORD_ATTRIBUTE_PREFIX))
        .and_then(|line| line.split_once(":").map(|line| line.1.to_owned()))?;
    let bundle = lines
        .clone()
        .find(|line| line.starts_with(GROUP_ATTRIBUTE_PREFIX))
        .and_then(|line| line.split_once(":").map(|line| line.1.to_owned()))?;

    let mut media_lines = lines
        .skip_while(|line| !line.starts_with(MEDIA_LINE_PREFIX))
        .filter(|line| {
            WHITELISTED_ATTRIBUTES
                .iter()
                .any(|item| line.starts_with(item))
        });

    let mut media_descriptors = vec![vec![]];
    let mut media_index = 0;
    media_descriptors[media_index].push(media_lines.next()?);
    while let Some(line) = media_lines.next() {
        if line.starts_with("m=") {
            media_index += 1;
            media_descriptors.push(vec![])
        }
        media_descriptors[media_index].push(line)
    }

    let mut audio_media: Option<AudioMedia> = None;
    let mut video_media: Option<VideoMedia> = None;

    for media in media_descriptors {
        let mut iterator = media.iter();
        let media_attribute: Vec<&str> = iterator.next()?.splitn(4, " ").collect();
        let media_type = media_attribute[0].split_once("=")?.1;

        match media_type {
            "audio" => {
                let attributes = media
                    .iter()
                    .map(|&str| str.to_owned())
                    .collect::<Vec<String>>();
                let payload_number = media
                    .iter()
                    .find(|line| line.to_ascii_lowercase().ends_with("opus/48000/2"))
                    .and_then(|&line| line.split(" ").nth(0))
                    .and_then(|attr| attr.split(":").nth(1))
                    .and_then(|num| num.parse::<usize>().ok())?;
                audio_media = Some(AudioMedia {
                    attributes,
                    format: AudioPayloadFormat::Opus,
                    payload_number,
                })
            }
            "video" => {
                let attributes = media
                    .iter()
                    .map(|&str| str.to_owned())
                    .collect::<Vec<String>>();
                let payload_number = media
                    .iter()
                    .find(|line| line.to_ascii_lowercase().ends_with("h264/90000")) // todo handle other video codecs
                    .and_then(|&line| line.split(" ").nth(0))
                    .and_then(|attr| attr.split(":").nth(1))
                    .and_then(|num| num.parse::<usize>().ok())?;

                video_media = Some(VideoMedia {
                    attributes,
                    format: VideoPayloadFormat::H264,
                    payload_number,
                })
            }
            _ => return None,
        }
    }

    Some(SDPD2 {
        ice_username: remote_username,
        ice_pwd: remote_password,
        group: bundle,
        audio_media: audio_media?,
        video_media: video_media?,
    })
}

pub fn create_sdp_receive_answer_2(
    sdp: &SDPD2,
    credentials: &SessionCredentials,
    fingerprint: &str,
) -> String {
    let SDPD2 {
        group,
        audio_media,
        video_media,
        ..
    } = &sdp;
    let SessionCredentials {
        host_password,
        host_username,
        ..
    } = &credentials;

    let session_description = format!(
        "v=0\r\n\
        o=sigma 2616320411 0 IN IP4 127.0.0.1\r\n\
        s=-\r\n\
        t=0 0\r\n\
        a=group:{group}\r\n\
        a=setup:passive\r\n\
        a=ice-ufrag:{host_username}\r\n\
        a=ice-pwd:{host_password}\r\n\
        a=ice-options:ice2\r\n\
        a=ice-lite\r\n\
        a=msid-semantic: WMS *\r\n\
        a=fingerprint:sha-256 {fingerprint}\r\n"
    );

    let find_audio_attr = |query: &str| {
        audio_media
            .attributes
            .iter()
            .find(|&line| line.starts_with(query))
            .unwrap()
    };

    let audio_media_description = format!(
        "m=audio 52000 UDP/TLS/RTP/SAVPF {payload_number}\r\n\
        c=IN IP4 192.168.0.157\r\n\
        a=recvonly\r\n\
        a=candidate:1 1 UDP 2122317823 192.168.0.157 52000 typ host\r\n\
        a=end-of-candidates\r\n\
        {mid_attr}\r\n\
        {rtpmap_attr}\r\n\
        {fmtp_attr}",
        payload_number = audio_media.payload_number,
        mid_attr = find_audio_attr("a=mid:"),
        rtpmap_attr = find_audio_attr(&format!("a=rtpmap:{}", audio_media.payload_number)),
        fmtp_attr = find_audio_attr(&format!("a=fmtp:{}", audio_media.payload_number))
    );

    let find_video_attr = |query: &str| {
        video_media
            .attributes
            .iter()
            .find(|&line| line.starts_with(query))
            .unwrap()
    };

    String::new()
}

#[derive(Debug)]
struct SDPD2 {
    pub ice_username: String,
    pub ice_pwd: String,
    pub group: String,
    pub video_media: VideoMedia,
    pub audio_media: AudioMedia,
}

#[derive(Debug)]
struct VideoMedia {
    format: VideoPayloadFormat,
    payload_number: usize,
    attributes: Vec<String>,
}

#[derive(Debug)]
enum VideoPayloadFormat {
    H264,
}

#[derive(Debug)]
struct AudioMedia {
    format: AudioPayloadFormat,
    payload_number: usize,
    attributes: Vec<String>,
}

#[derive(Debug)]
enum AudioPayloadFormat {
    Opus,
}

const ICE_USERNAME_ATTRIBUTE_PREFIX: &str = "a=ice-ufrag:";
const ICE_PASSWORD_ATTRIBUTE_PREFIX: &str = "a=ice-pwd:";
const GROUP_ATTRIBUTE_PREFIX: &str = "a=group:";
const MEDIA_LINE_PREFIX: &str = "m=";

const WHITELISTED_ATTRIBUTES: [&str; 9] = [
    "m=",
    "a=ssrc",
    "a=msid",
    "a=rtcp-mux",
    "a=rtpmap",
    "a=fmtp",
    "a=mid",
    "a=rtcp",
    "a=rtcp-fb",
];
