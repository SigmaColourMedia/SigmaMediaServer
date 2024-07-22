use std::net::SocketAddr;

use rand::{Rng, RngCore, thread_rng};
use rand::distributions::Alphanumeric;

use crate::line_parsers::{
    Attribute, AudioCodec, Candidate, ConnectionData, Fingerprint, FMTP, ICEOption,
    ICEOptions, ICEPassword, ICEUsername, MediaCodec, MediaDescription, MediaGroup, MediaID,
    MediaSSRC, MediaTransportProtocol, MediaType, Originator, RTPMap, SDPLine, SDPParseError,
    SessionTime, VideoCodec,
};

#[derive(Debug)]
struct SDP {
    session_section: Vec<SDPLine>,
    video_section: Vec<SDPLine>,
    audio_section: Vec<SDPLine>,
}

#[derive(Debug)]
struct NegotiatedSession {
    sdp_answer: SDP,
    ice_credentials: ICECredentials,
    video_session: VideoSession,
    audio_session: AudioSession,
}
#[derive(Debug)]
struct ICECredentials {
    host_username: String,
    host_password: String,
    remote_username: String,
    remote_password: String,
}
#[derive(Debug)]
struct VideoSession {
    codec: VideoCodec,
    payload_number: usize,
    host_ssrc: u32,
    remote_ssrc: u32,
    capabilities: Vec<String>,
}

#[derive(Debug)]
struct AudioSession {
    codec: AudioCodec,
    payload_number: usize,
    host_ssrc: u32,
    remote_ssrc: u32,
}

struct SDPResolver {
    fingerprint: Fingerprint,
    candidate: Candidate,
}

fn get_random_string(size: usize) -> String {
    thread_rng()
        .sample_iter(Alphanumeric)
        .take(size)
        .map(char::from)
        .collect()
}

fn get_random_ssrc() -> u32 {
    thread_rng().next_u32()
}

impl From<SDP> for String {
    fn from(value: SDP) -> Self {
        let video = value
            .video_section
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
            .join("\r\n");
        let audio = value
            .audio_section
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
            .join("\r\n");
        let session = value
            .session_section
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
            .join("\r\n");

        format!("{}\r\n{}\r\n{}\r\n", session, audio, video)
    }
}

impl SDPResolver {
    pub fn new(fingerprint_hash: &str, udp_socket: SocketAddr) -> Self {
        let fingerprint = Fingerprint::try_from(fingerprint_hash)
            .expect("Fingerprint should be in form of \"hash-function hash\"");
        let candidate = Candidate {
            foundation: "1".to_string(),
            component_id: 1,
            priority: 2015363327,
            connection_address: udp_socket.ip(),
            port: udp_socket.port(),
        };

        SDPResolver {
            fingerprint,
            candidate,
        }
    }
    const ACCEPTED_VIDEO_CODEC: VideoCodec = VideoCodec::H264;
    const ACCEPTED_AUDIO_CODEC: AudioCodec = AudioCodec::Opus;

    fn get_ice_credentials(sdp: &SDP) -> Option<ICECredentials> {
        let get_ice_username = |section: &Vec<SDPLine>| {
            section.iter().find_map(|line| match line {
                SDPLine::Attribute(attr) => match attr {
                    Attribute::ICEUsername(ice_username) => Some(ice_username.clone()),
                    _ => None,
                },
                _ => None,
            })
        };
        let get_ice_password = |section: &Vec<SDPLine>| {
            section.iter().find_map(|line| match line {
                SDPLine::Attribute(attr) => match attr {
                    Attribute::ICEPassword(ice_password) => Some(ice_password.clone()),
                    _ => None,
                },
                _ => None,
            })
        };

        // Look for ICE credentials in session section. These serve as default values, are overridden by media-level ICE credentials, are not required.
        let default_username = get_ice_username(&sdp.session_section);
        let default_password = get_ice_password(&sdp.session_section);

        let audio_media_username = get_ice_username(&sdp.audio_section);
        let audio_media_password = get_ice_password(&sdp.audio_section);

        let video_media_username = get_ice_username(&sdp.video_section);
        let video_media_password = get_ice_password(&sdp.video_section);

        // If media-level ICE credentials are present, then they need to be the same for all data streams
        if audio_media_username.is_some() {
            let audio_media_username = audio_media_username?;
            let audio_media_password = audio_media_password?;
            let video_media_username = video_media_username?;
            let video_media_password = video_media_password?;

            if audio_media_username.ne(&video_media_username)
                || audio_media_password.ne(&video_media_password)
            {
                return None;
            }

            return Some(ICECredentials {
                remote_username: audio_media_username.username.to_string(),
                remote_password: audio_media_password.password.to_string(),
                host_username: get_random_string(4),
                host_password: get_random_string(22),
            });
        }

        return Some(ICECredentials {
            remote_username: default_username?.username.to_string(),
            remote_password: default_password?.password.to_string(),
            host_username: get_random_string(4),
            host_password: get_random_string(22),
        });
    }

    fn get_audio_session(
        session_sdp: &Vec<SDPLine>,
        target_media_id: &MediaID,
    ) -> Result<AudioSession, SDPParseError> {
        let audio_mid = session_sdp
            .iter()
            .find_map(|item| match item {
                SDPLine::Attribute(attr) => match attr {
                    Attribute::MediaID(media_id) => Some(media_id),
                    _ => None,
                },
                _ => None,
            })
            .ok_or(SDPParseError::InvalidMediaID)?;

        if audio_mid.ne(&target_media_id) {
            return Err(SDPParseError::InvalidMediaID);
        }

        // Check if audio stream is demuxed
        let is_rtcp_demuxed = session_sdp
            .iter()
            .find_map(|item| match item {
                SDPLine::Attribute(attr) => match attr {
                    Attribute::RTCPMux => Some(()),
                    _ => None,
                },
                _ => None,
            })
            .is_some();

        if !is_rtcp_demuxed {
            return Err(SDPParseError::DemuxRequired);
        }

        // Check if stream is sendonly
        let is_sendonly_direction = session_sdp
            .iter()
            .find_map(|item| match item {
                SDPLine::Attribute(attr) => match attr {
                    Attribute::SendOnly => Some(()),
                    _ => None,
                },
                _ => None,
            })
            .is_some();

        if !is_sendonly_direction {
            return Err(SDPParseError::InvalidStreamDirection);
        }

        let remote_audio_ssrc = session_sdp
            .iter()
            .find_map(|item| match item {
                SDPLine::Attribute(attr) => match attr {
                    Attribute::MediaSSRC(media_ssrc) => Some(media_ssrc.ssrc),
                    _ => None,
                },
                _ => None,
            })
            .ok_or(SDPParseError::MissingStreamSSRC)?;

        let accepted_codec_payload_number = session_sdp
            .iter()
            .find_map(|item| match item {
                SDPLine::Attribute(attr) => match attr {
                    Attribute::RTPMap(rtpmap) => {
                        if rtpmap
                            .codec
                            .eq(&MediaCodec::Audio(Self::ACCEPTED_AUDIO_CODEC))
                        {
                            return Some(rtpmap.payload_number);
                        }
                        None
                    }
                    _ => None,
                },
                _ => None,
            })
            .ok_or(SDPParseError::UnsupportedMediaCodecs)?;

        Ok(AudioSession {
            codec: Self::ACCEPTED_AUDIO_CODEC,
            payload_number: accepted_codec_payload_number,
            remote_ssrc: remote_audio_ssrc,
            host_ssrc: get_random_ssrc(),
        })
    }

    fn get_video_session(
        session_sdp: &Vec<SDPLine>,
        target_media_id: &MediaID,
    ) -> Result<VideoSession, SDPParseError> {
        let media_id = session_sdp
            .iter()
            .find_map(|item| match item {
                SDPLine::Attribute(attr) => match attr {
                    Attribute::MediaID(media_id) => Some(media_id),
                    _ => None,
                },
                _ => None,
            })
            .ok_or(SDPParseError::InvalidMediaID)?;

        if media_id.ne(&target_media_id) {
            return Err(SDPParseError::InvalidMediaID);
        }

        // Check if stream is demuxed
        let is_rtcp_demuxed = session_sdp
            .iter()
            .find_map(|item| match item {
                SDPLine::Attribute(attr) => match attr {
                    Attribute::RTCPMux => Some(()),
                    _ => None,
                },
                _ => None,
            })
            .is_some();

        if !is_rtcp_demuxed {
            return Err(SDPParseError::DemuxRequired);
        }

        // Check if stream is sendonly
        let is_sendonly_direction = session_sdp
            .iter()
            .find_map(|item| match item {
                SDPLine::Attribute(attr) => match attr {
                    Attribute::SendOnly => Some(()),
                    _ => None,
                },
                _ => None,
            })
            .is_some();

        if !is_sendonly_direction {
            return Err(SDPParseError::InvalidStreamDirection);
        }

        // Check for stream ssrc
        let remote_video_ssrc = session_sdp
            .iter()
            .find_map(|item| match item {
                SDPLine::Attribute(attr) => match attr {
                    Attribute::MediaSSRC(media_ssrc) => Some(media_ssrc.ssrc),
                    _ => None,
                },
                _ => None,
            })
            .ok_or(SDPParseError::MissingStreamSSRC)?;

        // Check if supported codec is present
        // todo Pick highest available video capabilities
        let accepted_codec_payload_number = session_sdp
            .iter()
            .find_map(|item| match item {
                SDPLine::Attribute(attr) => match attr {
                    Attribute::RTPMap(rtpmap) => {
                        if rtpmap
                            .codec
                            .eq(&MediaCodec::Video(Self::ACCEPTED_VIDEO_CODEC))
                        {
                            return Some(rtpmap.payload_number);
                        }
                        None
                    }
                    _ => None,
                },
                _ => None,
            })
            .ok_or(SDPParseError::UnsupportedMediaCodecs)?;

        // Get FMTP value
        let video_capabilities = session_sdp
            .iter()
            .find_map(|item| match item {
                SDPLine::Attribute(attr) => match attr {
                    Attribute::FMTP(fmtp) => {
                        if fmtp.payload_number.eq(&accepted_codec_payload_number) {
                            return Some(fmtp.format_capability.clone());
                        }
                        None
                    }
                    _ => None,
                },
                _ => None,
            })
            .ok_or(SDPParseError::MissingVideoCapabilities)?;

        Ok(VideoSession {
            codec: Self::ACCEPTED_VIDEO_CODEC,
            capabilities: video_capabilities,
            payload_number: accepted_codec_payload_number,
            remote_ssrc: remote_video_ssrc,
            host_ssrc: get_random_ssrc(),
        })
    }
    pub fn accept_stream_offer(&self, raw_data: &str) -> Result<NegotiatedSession, SDPParseError> {
        let sdp = Self::get_sdp(raw_data)?;
        self.parse_stream_offer(sdp)
    }

    fn parse_stream_offer(&self, sdp_offer: SDP) -> Result<NegotiatedSession, SDPParseError> {
        // Check if stream is bundled and get media stream ids
        let bundle_group = sdp_offer
            .session_section
            .iter()
            .find_map(|item| match item {
                SDPLine::Attribute(attr) => match attr {
                    Attribute::MediaGroup(media_group) => match media_group {
                        MediaGroup::Bundle(group) => Some(group),
                        MediaGroup::LipSync(_) => None,
                    },
                    _ => None,
                },
                _ => None,
            })
            .ok_or(SDPParseError::BundleRequired)?;

        let audio_mid = MediaID {
            id: bundle_group
                .iter()
                .nth(0)
                .ok_or(SDPParseError::MalformedSDPLine)?
                .to_string(),
        };

        let video_mid = MediaID {
            id: bundle_group
                .iter()
                .nth(1)
                .ok_or(SDPParseError::MalformedSDPLine)?
                .to_string(),
        };

        let ice_credentials =
            Self::get_ice_credentials(&sdp_offer).ok_or(SDPParseError::MissingICECredentials)?;
        let audio_session = Self::get_audio_session(&sdp_offer.audio_section, &audio_mid)?;
        let video_session = Self::get_video_session(&sdp_offer.video_section, &video_mid)?;

        let session_section = vec![
            SDPLine::ProtocolVersion("0".to_string()),
            SDPLine::Originator(Originator {
                username: "smid".to_string(),
                ip_addr: self.candidate.connection_address.clone(),
                session_version: "0".to_string(),
                session_id: "3767197920".to_string(), // todo Handle unique NTP-like timestamps
            }),
            SDPLine::SessionName("smid".to_string()),
            SDPLine::SessionTime(SessionTime {
                start_time: 0,
                end_time: 0,
            }),
            SDPLine::Attribute(Attribute::MediaGroup(MediaGroup::Bundle(
                bundle_group.clone(),
            ))),
            SDPLine::Attribute(Attribute::ICEUsername(ICEUsername {
                username: ice_credentials.host_username.clone(),
            })),
            SDPLine::Attribute(Attribute::ICEPassword(ICEPassword {
                password: ice_credentials.host_password.clone(),
            })),
            SDPLine::Attribute(Attribute::ICEOptions(ICEOptions {
                options: vec![ICEOption::ICE2],
            })),
            SDPLine::Attribute(Attribute::ICELite),
            SDPLine::Attribute(Attribute::Fingerprint(self.fingerprint.clone())),
        ];

        let audio_section = vec![
            SDPLine::MediaDescription(MediaDescription {
                transport_port: self.candidate.port as usize,
                media_type: MediaType::Audio,
                transport_protocol: MediaTransportProtocol::DtlsSrtp,
                media_format_description: vec![audio_session.payload_number],
            }),
            SDPLine::ConnectionData(ConnectionData {
                ip: self.candidate.connection_address,
            }),
            SDPLine::Attribute(Attribute::ReceiveOnly),
            SDPLine::Attribute(Attribute::RTCPMux),
            SDPLine::Attribute(Attribute::MediaID(audio_mid)),
            SDPLine::Attribute(Attribute::Candidate(self.candidate.clone())),
            SDPLine::Attribute(Attribute::EndOfCandidates),
            SDPLine::Attribute(Attribute::RTPMap(RTPMap {
                codec: MediaCodec::Audio(audio_session.codec.clone()),
                payload_number: audio_session.payload_number,
            })),
            SDPLine::Attribute(Attribute::MediaSSRC(MediaSSRC {
                ssrc: audio_session.host_ssrc,
            })),
        ];

        let video_section = vec![
            SDPLine::MediaDescription(MediaDescription {
                transport_port: self.candidate.port as usize,
                media_type: MediaType::Video,
                transport_protocol: MediaTransportProtocol::DtlsSrtp,
                media_format_description: vec![video_session.payload_number],
            }),
            SDPLine::ConnectionData(ConnectionData {
                ip: self.candidate.connection_address,
            }),
            SDPLine::Attribute(Attribute::ReceiveOnly),
            SDPLine::Attribute(Attribute::RTCPMux),
            SDPLine::Attribute(Attribute::MediaID(video_mid)),
            SDPLine::Attribute(Attribute::RTPMap(RTPMap {
                codec: MediaCodec::Video(video_session.codec.clone()),
                payload_number: audio_session.payload_number,
            })),
            SDPLine::Attribute(Attribute::MediaSSRC(MediaSSRC {
                ssrc: video_session.host_ssrc,
            })),
            SDPLine::Attribute(Attribute::FMTP(FMTP {
                payload_number: video_session.payload_number,
                format_capability: video_session.capabilities.clone(),
            })),
        ];

        let sdp_answer = SDP {
            session_section,
            audio_section,
            video_section,
        };

        Ok(NegotiatedSession {
            ice_credentials,
            audio_session,
            video_session,
            sdp_answer,
        })
    }

    /**
    Parse raw string data to SDP struct. SDP struct is split into session, audio and video section, with each section having ownership over corresponding SDPLine elements.
    Check if session section is properly formatted.
    Only two media sections are legal and the first one needs to be audio. This is a completely arbitrary decision
    that serves to ease parser implementations.
        */
    fn get_sdp(raw_data: &str) -> Result<SDP, SDPParseError> {
        let sdp_lines = raw_data
            .lines()
            .map(SDPLine::try_from)
            .collect::<Result<Vec<SDPLine>, SDPParseError>>()?;

        println!("input lines {:?}\r\n", sdp_lines);

        let next_line = sdp_lines
            .iter()
            .nth(0)
            .ok_or(SDPParseError::SequenceError)?;
        if next_line.ne(&SDPLine::ProtocolVersion("0".to_string())) {
            return Err(SDPParseError::SequenceError);
        }

        let next_line = sdp_lines
            .iter()
            .nth(1)
            .ok_or(SDPParseError::SequenceError)?;
        if !matches!(next_line, SDPLine::Originator(_)) {
            return Err(SDPParseError::SequenceError);
        }

        let next_line = sdp_lines
            .iter()
            .nth(2)
            .ok_or(SDPParseError::SequenceError)?;
        if !matches!(next_line, SDPLine::SessionName(_)) {
            return Err(SDPParseError::SequenceError);
        }

        let next_line = sdp_lines
            .iter()
            .nth(3)
            .ok_or(SDPParseError::SequenceError)?;
        if !matches!(next_line, SDPLine::SessionTime(_)) {
            return Err(SDPParseError::SequenceError);
        }

        let media_descriptors = sdp_lines
            .iter()
            .filter_map(|sdp_line| match sdp_line {
                SDPLine::MediaDescription(media_descriptor) => Some(media_descriptor),
                _ => None,
            })
            .collect::<Vec<_>>();

        let has_two_media_descriptors = media_descriptors.iter().count().eq(&2);
        if !has_two_media_descriptors {
            return Err(SDPParseError::UnsupportedMediaCount);
        }

        let first_media = *media_descriptors
            .iter()
            .nth(0)
            .expect("Media descriptors should have 2 elements");
        let is_first_media_audio = first_media.media_type.eq(&MediaType::Audio);

        if !is_first_media_audio {
            return Err(SDPParseError::SequenceError);
        }

        let second_media = *media_descriptors
            .iter()
            .nth(1)
            .expect("Media descriptors should have 2 elements");
        let is_second_media_video = second_media.media_type.eq(&MediaType::Video);

        if !is_second_media_video {
            return Err(SDPParseError::SequenceError);
        }

        let session_section = sdp_lines
            .iter()
            .take_while(|item| match item {
                SDPLine::MediaDescription(media) => media.ne(first_media),
                _ => true,
            })
            .map(Clone::clone)
            .collect::<Vec<_>>();

        let audio_section = sdp_lines
            .iter()
            .skip_while(|item| match item {
                SDPLine::MediaDescription(media) => media.ne(first_media),
                _ => true,
            })
            .take_while(|item| match item {
                SDPLine::MediaDescription(media) => media.ne(second_media),
                _ => true,
            })
            .map(Clone::clone)
            .collect::<Vec<_>>();

        let video_section = sdp_lines
            .iter()
            .skip_while(|&item| match item {
                SDPLine::MediaDescription(media) => media.ne(second_media),
                _ => true,
            })
            .map(Clone::clone)
            .collect::<Vec<_>>();

        Ok(SDP {
            session_section,
            audio_section,
            video_section,
        })
    }
}

mod tests {
    mod sdp_resolver {
        mod get_sdp {
            use std::net::{IpAddr, Ipv4Addr, SocketAddr};

            use crate::resolvers::SDPResolver;

            const VALID_SDP: &str = "v=0\r\no=rtc 3767197920 0 IN IP4 127.0.0.1\r\ns=-\r\nt=0 0\r\na=group:BUNDLE 0 1\r\na=group:LS 0 1\r\na=msid-semantic:WMS *\r\na=setup:actpass\r\na=ice-ufrag:E2Fr\r\na=ice-pwd:OpQzg1PAwUdeOB244chlgd\r\na=ice-options:trickle\r\na=fingerprint:sha-256 EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B\r\nm=audio 4557 UDP/TLS/RTP/SAVPF 111\r\nc=IN IP4 192.168.0.198\r\na=mid:0\r\na=sendonly\r\na=ssrc:1349455989 cname:0X2NGAsK9XcmnsuZ\r\na=ssrc:1349455989 msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-audio\r\na=msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-audio\r\na=rtcp-mux\r\na=rtpmap:111 opus/48000/2\r\na=fmtp:111 minptime=10;maxaveragebitrate=96000;stereo=1;sprop-stereo=1;useinbandfec=1\r\na=candidate:1 1 UDP 2015363327 192.168.0.198 4557 typ host\r\na=candidate:2 1 UDP 2015363583 fe80::6c3d:5b42:1532:2f9a 10007 typ host\r\na=end-of-candidates\r\nm=video 4557 UDP/TLS/RTP/SAVPF 96\r\nc=IN IP4 192.168.0.198\r\na=mid:1\r\na=sendonly\r\na=ssrc:1349455990 cname:0X2NGAsK9XcmnsuZ\r\na=ssrc:1349455990 msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-video\r\na=msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-video\r\na=rtcp-mux\r\na=rtpmap:96 H264/90000\r\na=rtcp-fb:96 nack\r\na=rtcp-fb:96 nack pli\r\na=rtcp-fb:96 goog-remb\r\na=fmtp:96 profile-level-id=42e01f;packetization-mode=1;level-asymmetry-allowed=1\r\n";

            #[test]
            fn resolves_valid_streamer_offer() {
                let socket_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 52000);
                let resolver = SDPResolver::new("fingerprint:sha-256 EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B", socket_addr);
                let result = resolver
                    .accept_stream_offer(VALID_SDP)
                    .expect("Should resolve to OK");
                println!("{}", String::from(result.sdp_answer))
            }
        }
    }
}

//
// mod tests {
//     mod accept_streamer_sdp {
//         use crate::line_parsers::{
//             Attribute, AudioCodec, FMTP, MediaCodec, MediaSSRC, RTPMap, SDPOffer, VideoCodec,
//         };
//         use crate::resolvers::{accept_streamer_sdp, StreamerOfferSDPParseError};
//
//         #[test]
//         fn rejects_empty_media_attributes() {
//             let offer: SDPOffer = SDPOffer {
//                 ice_username: "test".to_string(),
//                 ice_password: "test".to_string(),
//                 video_media_description: vec![],
//                 audio_media_description: vec![],
//             };
//
//             let result = accept_streamer_sdp(offer);
//
//             assert!(result.is_err())
//         }
//
//         #[test]
//         fn rejects_recvonly_offer() {
//             let video_attributes: Vec<Attribute> = vec![
//                 Attribute::ReceiveOnly,
//                 Attribute::RTCPMux,
//                 Attribute::MediaSSRC(MediaSSRC {
//                     ssrc: "video-ssrc".to_string(),
//                 }),
//                 Attribute::FMTP(FMTP {
//                     payload_number: 96,
//                     format_capability: vec!["fake-profile-level".to_string()],
//                 }),
//                 Attribute::RTPMap(RTPMap {
//                     codec: MediaCodec::Video(VideoCodec::H264),
//                     payload_number: 96,
//                 }),
//             ];
//
//             let audio_attributes: Vec<Attribute> = vec![
//                 Attribute::ReceiveOnly,
//                 Attribute::RTCPMux,
//                 Attribute::MediaSSRC(MediaSSRC {
//                     ssrc: "audio-ssrc".to_string(),
//                 }),
//                 Attribute::RTPMap(RTPMap {
//                     codec: MediaCodec::Audio(AudioCodec::Opus),
//                     payload_number: 111,
//                 }),
//             ];
//
//             let offer_username = "username";
//             let offer_password = "password";
//
//             let offer = SDPOffer {
//                 ice_username: offer_username.to_string(),
//                 ice_password: offer_password.to_string(),
//                 audio_media_description: audio_attributes,
//                 video_media_description: video_attributes,
//             };
//
//             let result = accept_streamer_sdp(offer).expect_err("Should reject offer");
//
//             assert_eq!(
//                 result,
//                 StreamerOfferSDPParseError::UnsupportedMediaDirection,
//                 "Should fail with UnsupportedMediaDirection error"
//             )
//         }
//
//         #[test]
//         fn rejects_video_recvonly_offer() {
//             let video_attributes: Vec<Attribute> = vec![
//                 Attribute::ReceiveOnly,
//                 Attribute::RTCPMux,
//                 Attribute::MediaSSRC(MediaSSRC {
//                     ssrc: "video-ssrc".to_string(),
//                 }),
//                 Attribute::FMTP(FMTP {
//                     payload_number: 96,
//                     format_capability: vec!["fake-profile-level".to_string()],
//                 }),
//                 Attribute::RTPMap(RTPMap {
//                     codec: MediaCodec::Video(VideoCodec::H264),
//                     payload_number: 96,
//                 }),
//             ];
//
//             let audio_attributes: Vec<Attribute> = vec![
//                 Attribute::SendOnly,
//                 Attribute::RTCPMux,
//                 Attribute::MediaSSRC(MediaSSRC {
//                     ssrc: "audio-ssrc".to_string(),
//                 }),
//                 Attribute::RTPMap(RTPMap {
//                     codec: MediaCodec::Audio(AudioCodec::Opus),
//                     payload_number: 111,
//                 }),
//             ];
//
//             let offer_username = "username";
//             let offer_password = "password";
//
//             let offer = SDPOffer {
//                 ice_username: offer_username.to_string(),
//                 ice_password: offer_password.to_string(),
//                 audio_media_description: audio_attributes,
//                 video_media_description: video_attributes,
//             };
//
//             let result = accept_streamer_sdp(offer).expect_err("Should reject offer");
//
//             assert_eq!(
//                 result,
//                 StreamerOfferSDPParseError::UnsupportedMediaDirection,
//                 "Should fail with UnsupportedMediaDirection error"
//             )
//         }
//
//         #[test]
//         fn rejects_audio_recvonly_offer() {
//             let video_attributes: Vec<Attribute> = vec![
//                 Attribute::SendOnly,
//                 Attribute::RTCPMux,
//                 Attribute::MediaSSRC(MediaSSRC {
//                     ssrc: "video-ssrc".to_string(),
//                 }),
//                 Attribute::FMTP(FMTP {
//                     payload_number: 96,
//                     format_capability: vec!["fake-profile-level".to_string()],
//                 }),
//                 Attribute::RTPMap(RTPMap {
//                     codec: MediaCodec::Video(VideoCodec::H264),
//                     payload_number: 96,
//                 }),
//             ];
//
//             let audio_attributes: Vec<Attribute> = vec![
//                 Attribute::ReceiveOnly,
//                 Attribute::RTCPMux,
//                 Attribute::MediaSSRC(MediaSSRC {
//                     ssrc: "audio-ssrc".to_string(),
//                 }),
//                 Attribute::RTPMap(RTPMap {
//                     codec: MediaCodec::Audio(AudioCodec::Opus),
//                     payload_number: 111,
//                 }),
//             ];
//
//             let offer_username = "username";
//             let offer_password = "password";
//
//             let offer = SDPOffer {
//                 ice_username: offer_username.to_string(),
//                 ice_password: offer_password.to_string(),
//                 audio_media_description: audio_attributes,
//                 video_media_description: video_attributes,
//             };
//
//             let result = accept_streamer_sdp(offer).expect_err("Should reject offer");
//
//             assert_eq!(
//                 result,
//                 StreamerOfferSDPParseError::UnsupportedMediaDirection,
//                 "Should fail with UnsupportedMediaDirection error"
//             )
//         }
//
//         #[test]
//         fn rejects_non_muxed_offer() {
//             let video_attributes: Vec<Attribute> = vec![
//                 Attribute::SendOnly,
//                 Attribute::MediaSSRC(MediaSSRC {
//                     ssrc: "video-ssrc".to_string(),
//                 }),
//                 Attribute::FMTP(FMTP {
//                     payload_number: 96,
//                     format_capability: vec!["fake-profile-level".to_string()],
//                 }),
//                 Attribute::RTPMap(RTPMap {
//                     codec: MediaCodec::Video(VideoCodec::H264),
//                     payload_number: 96,
//                 }),
//             ];
//
//             let audio_attributes: Vec<Attribute> = vec![
//                 Attribute::SendOnly,
//                 Attribute::MediaSSRC(MediaSSRC {
//                     ssrc: "audio-ssrc".to_string(),
//                 }),
//                 Attribute::RTPMap(RTPMap {
//                     codec: MediaCodec::Audio(AudioCodec::Opus),
//                     payload_number: 111,
//                 }),
//             ];
//
//             let offer_username = "username";
//             let offer_password = "password";
//
//             let offer = SDPOffer {
//                 ice_username: offer_username.to_string(),
//                 ice_password: offer_password.to_string(),
//                 audio_media_description: audio_attributes,
//                 video_media_description: video_attributes,
//             };
//
//             let result = accept_streamer_sdp(offer).expect_err("Should reject offer");
//
//             assert_eq!(
//                 result,
//                 StreamerOfferSDPParseError::DemuxRequired,
//                 "Should fail with DemuxRequired error"
//             )
//         }
//
//         #[test]
//         fn rejects_offer_with_unsupported_video_codecs() {
//             let video_attributes: Vec<Attribute> = vec![
//                 Attribute::SendOnly,
//                 Attribute::RTCPMux,
//                 Attribute::MediaSSRC(MediaSSRC {
//                     ssrc: "video-ssrc".to_string(),
//                 }),
//                 Attribute::FMTP(FMTP {
//                     payload_number: 96,
//                     format_capability: vec!["fake-profile-level".to_string()],
//                 }),
//                 Attribute::RTPMap(RTPMap {
//                     codec: MediaCodec::Unsupported,
//                     payload_number: 96,
//                 }),
//             ];
//
//             let audio_attributes: Vec<Attribute> = vec![
//                 Attribute::SendOnly,
//                 Attribute::RTCPMux,
//                 Attribute::MediaSSRC(MediaSSRC {
//                     ssrc: "audio-ssrc".to_string(),
//                 }),
//                 Attribute::RTPMap(RTPMap {
//                     codec: MediaCodec::Audio(AudioCodec::Opus),
//                     payload_number: 111,
//                 }),
//             ];
//
//             let offer_username = "username";
//             let offer_password = "password";
//
//             let offer = SDPOffer {
//                 ice_username: offer_username.to_string(),
//                 ice_password: offer_password.to_string(),
//                 audio_media_description: audio_attributes,
//                 video_media_description: video_attributes,
//             };
//
//             let result = accept_streamer_sdp(offer).expect_err("Should reject offer");
//
//             assert_eq!(
//                 result,
//                 StreamerOfferSDPParseError::UnsupportedMediaCodecs,
//                 "Should fail with UnsupportedMediaCodecs error"
//             )
//         }
//
//         #[test]
//         fn rejects_offer_with_unsupported_audio_codecs() {
//             let video_attributes: Vec<Attribute> = vec![
//                 Attribute::SendOnly,
//                 Attribute::RTCPMux,
//                 Attribute::MediaSSRC(MediaSSRC {
//                     ssrc: "video-ssrc".to_string(),
//                 }),
//                 Attribute::FMTP(FMTP {
//                     payload_number: 96,
//                     format_capability: vec!["fake-profile-level".to_string()],
//                 }),
//                 Attribute::RTPMap(RTPMap {
//                     codec: MediaCodec::Video(VideoCodec::H264),
//                     payload_number: 96,
//                 }),
//             ];
//
//             let audio_attributes: Vec<Attribute> = vec![
//                 Attribute::SendOnly,
//                 Attribute::RTCPMux,
//                 Attribute::MediaSSRC(MediaSSRC {
//                     ssrc: "audio-ssrc".to_string(),
//                 }),
//                 Attribute::RTPMap(RTPMap {
//                     codec: MediaCodec::Unsupported,
//                     payload_number: 111,
//                 }),
//             ];
//
//             let offer_username = "username";
//             let offer_password = "password";
//
//             let offer = SDPOffer {
//                 ice_username: offer_username.to_string(),
//                 ice_password: offer_password.to_string(),
//                 audio_media_description: audio_attributes,
//                 video_media_description: video_attributes,
//             };
//
//             let result = accept_streamer_sdp(offer).expect_err("Should reject offer");
//
//             assert_eq!(
//                 result,
//                 StreamerOfferSDPParseError::UnsupportedMediaCodecs,
//                 "Should fail with UnsupportedMediaCodecs error"
//             )
//         }
//
//         #[test]
//         fn rejects_offer_with_missing_video_ssrc() {
//             let video_attributes: Vec<Attribute> = vec![
//                 Attribute::SendOnly,
//                 Attribute::RTCPMux,
//                 Attribute::FMTP(FMTP {
//                     payload_number: 96,
//                     format_capability: vec!["fake-profile-level".to_string()],
//                 }),
//                 Attribute::RTPMap(RTPMap {
//                     codec: MediaCodec::Video(VideoCodec::H264),
//                     payload_number: 96,
//                 }),
//             ];
//
//             let audio_attributes: Vec<Attribute> = vec![
//                 Attribute::SendOnly,
//                 Attribute::RTCPMux,
//                 Attribute::MediaSSRC(MediaSSRC {
//                     ssrc: "audio-ssrc".to_string(),
//                 }),
//                 Attribute::RTPMap(RTPMap {
//                     codec: MediaCodec::Audio(AudioCodec::Opus),
//                     payload_number: 111,
//                 }),
//             ];
//
//             let offer_username = "username";
//             let offer_password = "password";
//
//             let offer = SDPOffer {
//                 ice_username: offer_username.to_string(),
//                 ice_password: offer_password.to_string(),
//                 audio_media_description: audio_attributes,
//                 video_media_description: video_attributes,
//             };
//
//             let result = accept_streamer_sdp(offer).expect_err("Should reject offer");
//
//             assert_eq!(
//                 result,
//                 StreamerOfferSDPParseError::MissingRemoteSSRC,
//                 "Should fail with MissingRemoteSSRC error"
//             )
//         }
//
//         #[test]
//         fn rejects_offer_with_missing_audio_ssrc() {
//             let video_attributes: Vec<Attribute> = vec![
//                 Attribute::SendOnly,
//                 Attribute::RTCPMux,
//                 Attribute::MediaSSRC(MediaSSRC {
//                     ssrc: "video-ssrc".to_string(),
//                 }),
//                 Attribute::FMTP(FMTP {
//                     payload_number: 96,
//                     format_capability: vec!["fake-profile-level".to_string()],
//                 }),
//                 Attribute::RTPMap(RTPMap {
//                     codec: MediaCodec::Video(VideoCodec::H264),
//                     payload_number: 96,
//                 }),
//             ];
//
//             let audio_attributes: Vec<Attribute> = vec![
//                 Attribute::SendOnly,
//                 Attribute::RTCPMux,
//                 Attribute::RTPMap(RTPMap {
//                     codec: MediaCodec::Audio(AudioCodec::Opus),
//                     payload_number: 111,
//                 }),
//             ];
//
//             let offer_username = "username";
//             let offer_password = "password";
//
//             let offer = SDPOffer {
//                 ice_username: offer_username.to_string(),
//                 ice_password: offer_password.to_string(),
//                 audio_media_description: audio_attributes,
//                 video_media_description: video_attributes,
//             };
//
//             let result = accept_streamer_sdp(offer).expect_err("Should reject offer");
//
//             assert_eq!(
//                 result,
//                 StreamerOfferSDPParseError::MissingRemoteSSRC,
//                 "Should fail with MissingRemoteSSRC error"
//             )
//         }
//
//         #[test]
//         fn rejects_offer_with_missing_video_fmtp() {
//             let video_attributes: Vec<Attribute> = vec![
//                 Attribute::SendOnly,
//                 Attribute::RTCPMux,
//                 Attribute::MediaSSRC(MediaSSRC {
//                     ssrc: "video-ssrc".to_string(),
//                 }),
//                 Attribute::RTPMap(RTPMap {
//                     codec: MediaCodec::Video(VideoCodec::H264),
//                     payload_number: 96,
//                 }),
//             ];
//
//             let audio_attributes: Vec<Attribute> = vec![
//                 Attribute::SendOnly,
//                 Attribute::RTCPMux,
//                 Attribute::MediaSSRC(MediaSSRC {
//                     ssrc: "audio-ssrc".to_string(),
//                 }),
//                 Attribute::RTPMap(RTPMap {
//                     codec: MediaCodec::Audio(AudioCodec::Opus),
//                     payload_number: 111,
//                 }),
//             ];
//
//             let offer_username = "username";
//             let offer_password = "password";
//
//             let offer = SDPOffer {
//                 ice_username: offer_username.to_string(),
//                 ice_password: offer_password.to_string(),
//                 audio_media_description: audio_attributes,
//                 video_media_description: video_attributes,
//             };
//
//             let result = accept_streamer_sdp(offer).expect_err("Should reject offer");
//
//             assert_eq!(
//                 result,
//                 StreamerOfferSDPParseError::MissingVideoProfileSettings,
//                 "Should fail with MissingVideoProfileSettings error"
//             )
//         }
//
//         #[test]
//         fn resolves_valid_offer() {
//             let valid_video_media_attributes: Vec<Attribute> = vec![
//                 Attribute::SendOnly,
//                 Attribute::RTCPMux,
//                 Attribute::MediaSSRC(MediaSSRC {
//                     ssrc: "video-ssrc".to_string(),
//                 }),
//                 Attribute::FMTP(FMTP {
//                     payload_number: 96,
//                     format_capability: vec!["fake-profile-level".to_string()],
//                 }),
//                 Attribute::RTPMap(RTPMap {
//                     codec: MediaCodec::Video(VideoCodec::H264),
//                     payload_number: 96,
//                 }),
//             ];
//
//             let valid_audio_media_attributes: Vec<Attribute> = vec![
//                 Attribute::SendOnly,
//                 Attribute::RTCPMux,
//                 Attribute::MediaSSRC(MediaSSRC {
//                     ssrc: "audio-ssrc".to_string(),
//                 }),
//                 Attribute::RTPMap(RTPMap {
//                     codec: MediaCodec::Audio(AudioCodec::Opus),
//                     payload_number: 111,
//                 }),
//             ];
//
//             let offer_username = "username";
//             let offer_password = "password";
//
//             let offer = SDPOffer {
//                 ice_username: offer_username.to_string(),
//                 ice_password: offer_password.to_string(),
//                 audio_media_description: valid_audio_media_attributes,
//                 video_media_description: valid_video_media_attributes,
//             };
//             let result = accept_streamer_sdp(offer).expect("Should accept SDP offer");
//
//             assert_eq!(
//                 result.video_codec,
//                 VideoCodec::H264,
//                 "Video codec should be H264"
//             );
//
//             assert_eq!(
//                 result.video_payload_number, 96,
//                 "Video payload number should be 96"
//             );
//             assert_eq!(
//                 result.video_capability,
//                 FMTP {
//                     payload_number: 96,
//                     format_capability: vec!["fake-profile-level".to_string()]
//                 },
//                 "Video FMTP should match offer FMTP with payload number 96"
//             );
//             assert_eq!(
//                 result.remote_video_ssrc,
//                 MediaSSRC {
//                     ssrc: "video-ssrc".to_string()
//                 },
//                 "Video MediaSSRC should match the offer MediaSSRC"
//             );
//
//             assert_eq!(
//                 result.audio_codec,
//                 AudioCodec::Opus,
//                 "Audio codec should be Opus"
//             );
//             assert_eq!(
//                 result.audio_payload_number, 111,
//                 "Audio payload number should be 111"
//             );
//             assert_eq!(
//                 result.remote_audio_ssrc,
//                 MediaSSRC {
//                     ssrc: "audio-ssrc".to_string()
//                 },
//                 "Audio MediaSSRC should match the offer MediaSSRC"
//             );
//
//             assert_eq!(
//                 result.remote_ice_username, offer_username,
//                 "Remote ICE username should match offer username"
//             );
//             assert_eq!(
//                 result.remote_ice_password, offer_password,
//                 "Remote ICE password should match offer password"
//             );
//         }
//     }
// }
