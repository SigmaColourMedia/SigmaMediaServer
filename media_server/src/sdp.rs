use crate::config::get_global_config;
use crate::ice_registry::SessionCredentials;
use crate::rnd::get_random_string;

pub fn parse_sdp(data: String) -> Option<SDP> {
    let lines = data.lines();
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
                    ssrc_attributes: media
                        .iter()
                        .filter(|&line| line.starts_with("a=ssrc") || line.starts_with("a=msid"))
                        .map(|line| line.to_string())
                        .collect(),
                    payload_number,
                    profile_level_id: media
                        .iter()
                        .find(|line| line.starts_with(&format!("a=fmtp:{}", payload_number)))
                        .map(|line| line.to_string())?,
                })
            }
            "video" => {
                let payload_number = media
                    .iter()
                    .find(|line| line.to_ascii_lowercase().ends_with("h264/90000"))
                    .and_then(|&line| line.split(" ").nth(0))
                    .and_then(|attr| attr.split(":").nth(1))
                    .and_then(|num| num.parse::<usize>().ok())?;

                video_media = Some(VideoMedia {
                    payload_number,
                    ssrc_attributes: media
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

pub fn create_sdp_receive_answer(sdp: &SDP, credentials: &SessionCredentials) -> String {
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

    let config = get_global_config();
    let udp_address = config.udp_server_config.address.ip().to_string();
    let udp_port = config.udp_server_config.address.port();

    let session_description = format!(
        "v=0\r\n\
        o=sigma 2616320411 0 IN IP4 {address}\r\n\
        s=-\r\n\
        t=0 0\r\n\
        a=group:{group}\r\n\
        a=setup:passive\r\n\
        a=ice-ufrag:{host_username}\r\n\
        a=ice-pwd:{host_password}\r\n\
        a=ice-options:ice2\r\n\
        a=ice-lite\r\n\
        a=fingerprint:sha-256 {fingerprint}\r\n",
        address = udp_address,
        fingerprint = config.ssl_config.fingerprint
    );

    let audio_media_description = format!(
        "m=audio {port} UDP/TLS/RTP/SAVPF {payload_number}\r\n\
        c=IN IP4 {address}\r\n\
        a=recvonly\r\n\
        a=rtcp-mux\r\n\
        a=candidate:1 1 UDP 2122317823 {address} {port} typ host\r\n\
        a=end-of-candidates\r\n\
        a=mid=0\r\n\
        a=rtmpmap:{payload_number} opus/48000/2\r\n\
        {profile_id}",
        payload_number = audio_media.payload_number,
        address = udp_address,
        port = udp_port,
        profile_id = audio_media.profile_level_id
    );

    let video_media_description = format!(
        "m=video {port} UDP/TLS/RTP/SAVPF {payload_number}\r\n\
        c=IN IP4 {address}\r\n\
        a=recvonly\r\n\
        a=rtcp-mux\r\n\
        a=candidate:1 1 UDP 2122317823 {address} {port} typ host\r\n\
        a=end-of-candidates\r\n\
        a=mid:1\r\n\
        a=rtpmap:{payload_number} h264/90000\r\n",
        payload_number = video_media.payload_number,
        port = udp_port,
        address = udp_address
    );

    session_description + &audio_media_description + &video_media_description
}

pub fn create_streaming_sdp_answer(streamer_sdp: &SDP) -> Option<(String, SessionCredentials)> {
    let host_username = get_random_string(4);
    let host_password = get_random_string(24);

    let config = get_global_config();
    let udp_address = config.udp_server_config.address.ip().to_string();
    let udp_port = config.udp_server_config.address.port();

    let session_description = format!(
        "v=0\r\n\
        o=sigma 2616320411 0 IN IP4 {address}\r\n\
        s=-\r\n\
        t=0 0\r\n\
        a=group:BUNDLE 0 1\r\n\
        a=group:LS 0 1\r\n\
        a=setup:passive\r\n\
        a=msid-semantic:WMS *\r\n\
        a=ice-ufrag:{host_username}\r\n\
        a=ice-pwd:{host_password}\r\n\
        a=ice-options:ice2\r\n\
        a=ice-lite\r\n\
        a=fingerprint:sha-256 {fingerprint}\r\n",
        address = udp_address,
        fingerprint = config.ssl_config.fingerprint
    );

    let audio_media_description = format!(
        "m=audio {port} UDP/TLS/RTP/SAVPF {payload_number}\r\n\
        c=IN IP4 {address}\r\n\
        a=sendonly\r\n\
        a=rtcp-mux\r\n\
        a=candidate:1 1 UDP 2122317823 {address} {port} typ host\r\n\
        a=end-of-candidates\r\n\
        a=mid:0\r\n\
        {ssrc}\r\n\
        {profile_id}\r\n\
        a=rtpmap:{payload_number} opus/48000/2\r\n",
        payload_number = streamer_sdp.audio_media.payload_number,
        ssrc = streamer_sdp.audio_media.ssrc_attributes.join("\r\n"),
        address = udp_address,
        port = udp_port,
        profile_id = streamer_sdp.audio_media.profile_level_id
    );

    let video_media_description = format!(
        "m=video {port} UDP/TLS/RTP/SAVPF {payload_number}\r\n\
        c=IN IP4 {address}\r\n\
        a=sendonly\r\n\
        a=rtcp-mux\r\n\
        a=mid:1\r\n\
        a=rtpmap:{payload_number} H264/90000\r\n\
        {ssrc}\r\n\
        {profile_level_id}\r\n",
        payload_number = streamer_sdp.video_media.payload_number,
        address = udp_address,
        port = udp_port,
        profile_level_id = streamer_sdp.video_media.profile_level_id,
        ssrc = streamer_sdp.video_media.ssrc_attributes.join("\r\n")
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
pub struct VideoMedia {
    profile_level_id: String,
    payload_number: usize,
    ssrc_attributes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AudioMedia {
    profile_level_id: String,
    payload_number: usize,
    ssrc_attributes: Vec<String>,
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
