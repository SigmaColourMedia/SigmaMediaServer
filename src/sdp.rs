use crate::ice_registry::SessionCredentials;

pub fn parse_sdp(data: String) -> Option<SDP> {
    let mut lines = data.lines();
    let remote_username = lines.clone().find(|line| line.starts_with(ICE_USERNAME_ATTRIBUTE_PREFIX)).and_then(|line| line.split_once(":").map(|line| line.1))?.to_owned();
    let remote_password = lines.clone().find(|line| line.starts_with(ICE_PASSWORD_ATTRIBUTE_PREFIX)).and_then(|line| line.split_once(":").map(|line| line.1))?.to_owned();
    let bundle = lines.clone().find(|line| line.starts_with(GROUP_ATTRIBUTE_PREFIX)).and_then(|line| line.split_once(":").map(|line| line.1))?.to_owned();

    let mut media_lines = lines.skip_while(|line| !line.starts_with(MEDIA_LINE_PREFIX)).filter(|line| WHITELISTED_ATTRIBUTES.iter().any(|item| line.starts_with(item)));

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

    let media_descriptors = media_descriptors.into_iter().map(|descriptor| {
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
    }).collect::<Option<Vec<MediaDescription>>>()?;


    Some(SDP {
        ice_pwd: remote_password,
        ice_username: remote_username,
        group: bundle,
        media_descriptions: media_descriptors,
    })
}

pub fn create_sdp_receive_answer(sdp: &SDP, credentials: &SessionCredentials, fingerprint: &str) -> String {
    let SDP { group, media_descriptions, .. } = &sdp;
    let SessionCredentials { host_password, host_username, .. } = &credentials;

    let session_description = format!("v=0\r\n\
o=sigma 2616320411 0 IN IP4 127.0.0.1\r\n\
a=group:{group}\r\n\
a=setup:passive\r\n\
a=ice-ufrag:{host_username}\r\n\
a=ice-pwd:{host_password}\r\n\
a=ice-options:ice2\r\n\
a=ice-lite\r\n\
a=fingerprint:sha-256 {fingerprint}\r\n");

    let media_description = media_descriptions.into_iter().map(|media| {
        let media_header = format!("m={media_type} 52000 {proto} {fmt}\r\n\
        c=IN IP4 127.0.0.1\r\n\
        a=candidate:1 1 UDP 2122317823 127.0.0.1 52000 typ host\r\n\
        a=end-of-candidates\r\n\
        a=recvonly\r\n", media_type = media.media_type, proto = media.protocol, fmt = media.format);

        let rest = media.attributes.join("\r\n");
        media_header + &rest
    }).collect::<Vec<String>>().join("\r\n");


    session_description + &media_description
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

const ICE_USERNAME_ATTRIBUTE_PREFIX: &str = "a=ice-ufrag:";
const ICE_PASSWORD_ATTRIBUTE_PREFIX: &str = "a=ice-pwd:";
const GROUP_ATTRIBUTE_PREFIX: &str = "a=group:";
const MEDIA_LINE_PREFIX: &str = "m=";

const WHITELISTED_ATTRIBUTES: [&str; 8] = ["m=", "a=ssrc", "a=msid", "a=rtcp-mux", "a=rtpmap", "a=fmtp", "a=mid", "a=rtcp"];