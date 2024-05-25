use crate::HOST_ADDRESS;
use crate::ice_registry::SessionCredentials;
use crate::rnd::get_random_string;

pub fn parse_sdp(data: String) -> Option<SDP> {
    println!("Received sdp {data}");
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
                let payload_number = media
                    .iter()
                    .find(|line| line.to_ascii_lowercase().ends_with("opus/48000/2"))
                    .and_then(|&line| line.split(" ").nth(0))
                    .and_then(|attr| attr.split(":").nth(1))
                    .and_then(|num| num.parse::<usize>().ok())?;
                audio_media = Some(AudioMedia {
                    format: AudioPayloadFormat::Opus,
                    extra_lines: media
                        .iter()
                        .filter(|&line| line.starts_with("a=ssrc") || line.starts_with("a=msid"))
                        .map(|line| line.to_string())
                        .collect(),
                    payload_number,
                })
            }
            "video" => {
                let payload_number = media
                    .iter()
                    .find(|line| line.to_ascii_lowercase().ends_with("h264/90000")) // todo handle other video codecs
                    .and_then(|&line| line.split(" ").nth(0))
                    .and_then(|attr| attr.split(":").nth(1))
                    .and_then(|num| num.parse::<usize>().ok())?;

                video_media = Some(VideoMedia {
                    format: VideoPayloadFormat::H264,
                    payload_number,
                    extra_lines: media
                        .iter()
                        .filter(|&line| line.starts_with("a=ssrc") || line.starts_with("a=msid"))
                        .map(|line| line.to_string())
                        .collect(),
                    profile_level_id: media
                        .iter()
                        .find(|line| {
                            line.starts_with(&format!(
                                "a=fmtp:{} profile-level-id=",
                                payload_number
                            ))
                        })
                        .map(|&line| line.to_owned())?,
                })
            }
            _ => return None,
        }
    }

    Some(SDP {
        ice_username: remote_username,
        ice_pwd: remote_password,
        group: bundle,
        audio_media: audio_media?,
        video_media: video_media?,
    })
}

pub fn create_sdp_receive_answer(
    sdp: &SDP,
    credentials: &SessionCredentials,
    fingerprint: &str,
) -> String {
    let SDP {
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
        o=sigma 2616320411 0 IN IP4 {HOST_ADDRESS}\r\n\
        s=-\r\n\
        t=0 0\r\n\
        a=group:{group}\r\n\
        a=setup:passive\r\n\
        a=ice-ufrag:{host_username}\r\n\
        a=ice-pwd:{host_password}\r\n\
        a=ice-options:ice2\r\n\
        a=ice-lite\r\n\
        a=fingerprint:sha-256 {fingerprint}\r\n"
    );

    let audio_media_description = format!(
        "m=audio 52000 UDP/TLS/RTP/SAVPF {payload_number}\r\n\
        c=IN IP4 {HOST_ADDRESS}\r\n\
        a=recvonly\r\n\
        a=rtcp-mux\r\n\
        a=candidate:1 1 UDP 2122317823 {HOST_ADDRESS} 52000 typ host\r\n\
        a=end-of-candidates\r\n\
        a=mid=0\r\n\
        a=rtmpmap:{payload_number} opus/48000/2\r\n",
        payload_number = audio_media.payload_number,
    );

    let video_media_description = format!(
        "m=video 52000 UDP/TLS/RTP/SAVPF {payload_number}\r\n\
        c=IN IP4 {HOST_ADDRESS}\r\n\
        a=recvonly\r\n\
        a=rtcp-mux\r\n\
        a=candidate:1 1 UDP 2122317823 {HOST_ADDRESS} 52000 typ host\r\n\
        a=end-of-candidates\r\n\
        a=mid:1\r\n\
        a=rtpmap:{payload_number} h264/90000\r\n",
        payload_number = video_media.payload_number,
        HOST_ADDRESS = HOST_ADDRESS
    );

    session_description + &audio_media_description + &video_media_description
}

pub fn create_streaming_sdp_answer(
    streamer_sdp: &SDP,
    fingerprint: &str,
) -> Option<(String, SessionCredentials)> {
    let host_username = get_random_string(4);
    let host_password = get_random_string(24);

    let session_description = format!(
        "v=0\r\n\
        o=sigma 2616320411 0 IN IP4 {HOST_ADDRESS}\r\n\
        s=-\r\n\
        t=0 0\r\n\
        a=group:BUNDLE 0 1\r\n\
        a=setup:passive\r\n\
        a=msid-semantic:WMS *\r\n\
        a=ice-ufrag:{host_username}\r\n\
        a=ice-pwd:{host_password}\r\n\
        a=ice-options:ice2\r\n\
        a=ice-lite\r\n\
        a=fingerprint:sha-256 {fingerprint}\r\n"
    );

    let audio_media_description = format!(
        "m=audio 52000 UDP/TLS/RTP/SAVPF {payload_number}\r\n\
        c=IN IP4 {HOST_ADDRESS}\r\n\
        a=sendonly\r\n\
        a=rtcp-mux\r\n\
        a=candidate:1 1 UDP 2122317823 {HOST_ADDRESS} 52000 typ host\r\n\
        a=end-of-candidates\r\n\
        a=mid:0\r\n\
        {extra_lines}\r\n\
        a=fmtp:111 minptime=10;maxaveragebitrate=96000;stereo=1;sprop-stereo=1;useinbandfec=1\r\n\
        a=rtpmap:{payload_number} opus/48000/2\r\n",
        payload_number = streamer_sdp.audio_media.payload_number,
        extra_lines = streamer_sdp.audio_media.extra_lines.join("\r\n")
    );

    let video_media_description = format!(
        "m=video 52000 UDP/TLS/RTP/SAVPF {payload_number}\r\n\
        c=IN IP4 {HOST_ADDRESS}\r\n\
        a=sendonly\r\n\
        a=rtcp-mux\r\n\
        a=bundleonly\r\n\
        a=mid:1\r\n\
        a=rtpmap:{payload_number} H264/90000\r\n\
        {extra_lines}\r\n\
        {profile_level_id}\r\n",
        payload_number = streamer_sdp.video_media.payload_number,
        profile_level_id = streamer_sdp.video_media.profile_level_id,
        extra_lines = streamer_sdp.video_media.extra_lines.join("\r\n")
    );

    let sdp_answer = session_description + &audio_media_description + &video_media_description;
    let credentials = SessionCredentials {
        host_username,
        host_password,
    };

    Some((sdp_answer, credentials))
}

#[derive(Debug, Clone)]
pub struct SDP {
    pub ice_username: String,
    pub ice_pwd: String,
    pub group: String,
    pub video_media: VideoMedia,
    pub audio_media: AudioMedia,
}

#[derive(Debug, Clone)]
struct VideoMedia {
    format: VideoPayloadFormat,
    profile_level_id: String,
    payload_number: usize,
    extra_lines: Vec<String>,
}

#[derive(Debug, Clone)]
enum VideoPayloadFormat {
    H264,
    VP8,
}

#[derive(Debug, Clone)]
struct AudioMedia {
    format: AudioPayloadFormat,
    payload_number: usize,
    extra_lines: Vec<String>,
}

#[derive(Debug, Clone)]
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
