use std::collections::HashSet;
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
    capabilities: HashSet<String>,
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
        let fingerprint =
            Fingerprint::try_from(format!("fingerprint {}", fingerprint_hash).as_str())
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
    pub fn accept_stream_offer(&self, raw_data: &str) -> Result<NegotiatedSession, SDPParseError> {
        let sdp = Self::get_sdp(raw_data)?;
        self.parse_stream_offer(sdp)
    }
    const ACCEPTED_VIDEO_CODEC: VideoCodec = VideoCodec::H264;
    const ACCEPTED_AUDIO_CODEC: AudioCodec = AudioCodec::Opus;

    /** Gets ICE credentials from the SDP. Uses session-level credentials if no media-level credentials were provided.
    If media-level credentials were provided, check if they match across media-streams and if so resolve to ICECredentials.
    */
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
        if audio_media_username.is_some() || video_media_username.is_some() {
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

    /** Get AudioSession based on audio-media-level SDPLines. Resolve codecs based on supported streamer codecs.
     */
    fn get_streamer_audio_session(
        audio_media_section: &Vec<SDPLine>,
    ) -> Result<AudioSession, SDPParseError> {
        // Check if audio stream is demuxed
        let is_rtcp_demuxed = audio_media_section
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
        let is_sendonly_direction = audio_media_section
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

        let remote_audio_ssrc = audio_media_section
            .iter()
            .find_map(|item| match item {
                SDPLine::Attribute(attr) => match attr {
                    Attribute::MediaSSRC(media_ssrc) => Some(media_ssrc.ssrc),
                    _ => None,
                },
                _ => None,
            })
            .ok_or(SDPParseError::MissingStreamSSRC)?;

        let accepted_codec_payload_number = audio_media_section
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

    fn get_streamer_video_session(
        video_media: &Vec<SDPLine>,
    ) -> Result<VideoSession, SDPParseError> {
        // Check if stream is demuxed
        let is_rtcp_demuxed = video_media
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
        let is_sendonly_direction = video_media
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
        let remote_video_ssrc = video_media
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
        let accepted_codec_payload_number = video_media
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
        let video_capabilities = video_media
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

    fn get_media_ids(sdp: &SDP) -> Result<(MediaID, MediaID), SDPParseError> {
        let bundle_group = sdp
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

        let expected_audio_mid = MediaID {
            id: bundle_group
                .iter()
                .nth(0)
                .ok_or(SDPParseError::MalformedSDPLine)?
                .to_string(),
        };

        let expected_video_mid = MediaID {
            id: bundle_group
                .iter()
                .nth(1)
                .ok_or(SDPParseError::MalformedSDPLine)?
                .to_string(),
        };

        let actual_audio_id = sdp
            .audio_section
            .iter()
            .find_map(|item| match item {
                SDPLine::Attribute(attr) => match attr {
                    Attribute::MediaID(media_id) => Some(media_id),
                    _ => None,
                },
                _ => None,
            })
            .ok_or(SDPParseError::InvalidMediaID)?;

        if expected_audio_mid.ne(actual_audio_id) {
            return Err(SDPParseError::InvalidMediaID);
        }

        let actual_video_id = sdp
            .video_section
            .iter()
            .find_map(|item| match item {
                SDPLine::Attribute(attr) => match attr {
                    Attribute::MediaID(media_id) => Some(media_id),
                    _ => None,
                },
                _ => None,
            })
            .ok_or(SDPParseError::InvalidMediaID)?;

        if expected_video_mid.ne(actual_video_id) {
            return Err(SDPParseError::InvalidMediaID);
        }

        return Ok((expected_audio_mid, expected_video_mid));
    }

    fn parse_stream_offer(&self, sdp_offer: SDP) -> Result<NegotiatedSession, SDPParseError> {
        // Check if stream is bundled and get media stream ids
        let (audio_mid, video_mid) = Self::get_media_ids(&sdp_offer)?;

        let ice_credentials =
            Self::get_ice_credentials(&sdp_offer).ok_or(SDPParseError::MissingICECredentials)?;
        let audio_session = Self::get_streamer_audio_session(&sdp_offer.audio_section)?;
        let video_session = Self::get_streamer_video_session(&sdp_offer.video_section)?;

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
            SDPLine::Attribute(Attribute::MediaGroup(MediaGroup::Bundle(vec![
                audio_mid.id.clone(),
                video_mid.id.clone(),
            ]))),
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

    fn get_viewer_audio_session(
        audio_media: &Vec<SDPLine>,
        streamer_session: &AudioSession,
    ) -> Result<AudioSession, SDPParseError> {
        // Check if audio stream is demuxed
        let is_rtcp_demuxed = audio_media
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

        // Check if stream is recvonly
        let is_recvonly_direction = audio_media
            .iter()
            .find_map(|item| match item {
                SDPLine::Attribute(attr) => match attr {
                    Attribute::ReceiveOnly => Some(()),
                    _ => None,
                },
                _ => None,
            })
            .is_some();

        if !is_recvonly_direction {
            return Err(SDPParseError::InvalidStreamDirection);
        }

        let legal_audio_codec = &streamer_session.codec;

        let resolved_payload_number = audio_media
            .iter()
            .find_map(|item| match item {
                SDPLine::Attribute(attr) => match attr {
                    Attribute::RTPMap(rtpmap) => {
                        if rtpmap
                            .codec
                            .eq(&MediaCodec::Audio(legal_audio_codec.clone()))
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

        let remote_ssrc = audio_media
            .iter()
            .find_map(|item| match item {
                SDPLine::Attribute(attr) => match attr {
                    Attribute::MediaSSRC(media_ssrc) => Some(media_ssrc),
                    _ => None,
                },
                _ => None,
            })
            .ok_or(SDPParseError::MissingStreamSSRC)?;

        Ok(AudioSession {
            codec: legal_audio_codec.clone(),
            payload_number: resolved_payload_number,
            host_ssrc: get_random_ssrc(),
            remote_ssrc: remote_ssrc.ssrc,
        })
    }

    fn get_viewer_video_session(
        video_media: &Vec<SDPLine>,
        streamer_session: &VideoSession,
    ) -> Result<VideoSession, SDPParseError> {
        // Check if stream is demuxed
        let is_rtcp_demuxed = video_media
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

        // Check if stream is recvonly
        let is_recvonly_direction = video_media
            .iter()
            .find_map(|item| match item {
                SDPLine::Attribute(attr) => match attr {
                    Attribute::ReceiveOnly => Some(()),
                    _ => None,
                },
                _ => None,
            })
            .is_some();

        if !is_recvonly_direction {
            return Err(SDPParseError::InvalidStreamDirection);
        }

        /*
        Here we start to look for a payload number that matches both streamer video codec and streamer video capabilities
         */
        // Only the negotiated streamer video codec is considered a legal option
        let legal_video_codec = &streamer_session.codec;

        // Get all payload numbers matching legal Video codec
        let available_payload_numbers = video_media
            .iter()
            .filter_map(|item| match item {
                SDPLine::Attribute(attr) => match attr {
                    Attribute::RTPMap(rtpmap) => {
                        if rtpmap
                            .codec
                            .eq(&MediaCodec::Video(legal_video_codec.clone()))
                        {
                            return Some(rtpmap.payload_number);
                        }
                        None
                    }
                    _ => None,
                },
                _ => None,
            })
            .collect::<Vec<usize>>();

        // Only the negotiated streamer video FMTP is considered a legal option
        let legal_video_fmtp = &streamer_session.capabilities;

        // Filter out all FMTPs not matching the available payload numbers and then look for one matching the legal FMTP
        // The filter could be skipped, but then we have no guarantee that this FMTP actually points to the proper codec
        let resolved_payload_number = video_media
            .iter()
            .filter_map(|item| match item {
                SDPLine::Attribute(attr) => match attr {
                    Attribute::FMTP(fmtp) => {
                        if available_payload_numbers.contains(&fmtp.payload_number) {
                            return Some(fmtp);
                        }
                        None
                    }
                    _ => None,
                },
                _ => None,
            })
            .find_map(|fmtp| {
                if fmtp.format_capability.eq(legal_video_fmtp) {
                    return Some(fmtp.payload_number);
                }
                None
            })
            .ok_or(SDPParseError::UnsupportedMediaCodecs)?;

        let remote_ssrc = video_media
            .iter()
            .find_map(|item| match item {
                SDPLine::Attribute(attr) => match attr {
                    Attribute::MediaSSRC(media_ssrc) => Some(media_ssrc),
                    _ => None,
                },
                _ => None,
            })
            .ok_or(SDPParseError::MissingStreamSSRC)?;

        Ok(VideoSession {
            capabilities: legal_video_fmtp.clone(),
            host_ssrc: get_random_ssrc(),
            remote_ssrc: remote_ssrc.ssrc,
            payload_number: resolved_payload_number,
            codec: legal_video_codec.clone(),
        })
    }

    fn parse_viewer_offer(
        &self,
        viewer_sdp: SDP,
        streamer_session: &NegotiatedSession,
    ) -> Result<NegotiatedSession, SDPParseError> {
        let ice_credentials =
            Self::get_ice_credentials(&viewer_sdp).ok_or(SDPParseError::MissingICECredentials)?;
        let (audio_mid, video_mid) = Self::get_media_ids(&viewer_sdp)?;
        let audio_session = Self::get_viewer_audio_session(
            &viewer_sdp.audio_section,
            &streamer_session.audio_session,
        )?;
        let video_session = Self::get_viewer_video_session(
            &viewer_sdp.video_section,
            &streamer_session.video_session,
        )?;

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
            SDPLine::Attribute(Attribute::MediaGroup(MediaGroup::Bundle(vec![
                audio_mid.id.clone(),
                video_mid.id.clone(),
            ]))),
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
            SDPLine::Attribute(Attribute::SendOnly),
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
            SDPLine::Attribute(Attribute::SendOnly),
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
            use std::collections::HashSet;
            use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
            use std::str::FromStr;

            use crate::line_parsers::{
                Attribute, AudioCodec, Candidate, ConnectionData, Fingerprint, FMTP,
                HashFunction, ICEOption, ICEOptions, ICEPassword, ICEUsername, MediaCodec,
                MediaDescription, MediaGroup, MediaID, MediaSSRC, MediaTransportProtocol, MediaType,
                Originator, RTPMap, SDPLine, SessionTime, VideoCodec,
            };
            use crate::resolvers::SDPResolver;

            const VALID_SDP: &str = "v=0\r\no=rtc 3767197920 0 IN IP4 127.0.0.1\r\ns=-\r\nt=0 0\r\na=group:BUNDLE 0 1\r\na=group:LS 0 1\r\na=msid-semantic:WMS *\r\na=setup:actpass\r\na=ice-ufrag:E2Fr\r\na=ice-pwd:OpQzg1PAwUdeOB244chlgd\r\na=ice-options:trickle\r\na=fingerprint:sha-256 EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B\r\nm=audio 4557 UDP/TLS/RTP/SAVPF 111\r\nc=IN IP4 192.168.0.198\r\na=mid:0\r\na=sendonly\r\na=ssrc:1349455989 cname:0X2NGAsK9XcmnsuZ\r\na=ssrc:1349455989 msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-audio\r\na=msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-audio\r\na=rtcp-mux\r\na=rtpmap:111 opus/48000/2\r\na=fmtp:111 minptime=10;maxaveragebitrate=96000;stereo=1;sprop-stereo=1;useinbandfec=1\r\na=candidate:1 1 UDP 2015363327 192.168.0.198 4557 typ host\r\na=candidate:2 1 UDP 2015363583 fe80::6c3d:5b42:1532:2f9a 10007 typ host\r\na=end-of-candidates\r\nm=video 4557 UDP/TLS/RTP/SAVPF 96\r\nc=IN IP4 192.168.0.198\r\na=mid:1\r\na=sendonly\r\na=ssrc:1349455990 cname:0X2NGAsK9XcmnsuZ\r\na=ssrc:1349455990 msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-video\r\na=msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-video\r\na=rtcp-mux\r\na=rtpmap:96 H264/90000\r\na=rtcp-fb:96 nack\r\na=rtcp-fb:96 nack pli\r\na=rtcp-fb:96 goog-remb\r\na=fmtp:96 profile-level-id=42e01f;packetization-mode=1;level-asymmetry-allowed=1\r\n";

            #[test]
            fn resolves_valid_sdp() {
                let result = SDPResolver::get_sdp(VALID_SDP).expect("Should resolve to OK");

                let expected_session_media = vec![
                    SDPLine::ProtocolVersion("0".to_string()),
                    SDPLine::Originator(Originator {
                        username: "rtc".to_string(),
                        session_id: "3767197920".to_string(),
                        session_version: "0".to_string(),
                        ip_addr: IpAddr::V4(Ipv4Addr::from([127, 0, 0, 1])),
                    }),
                    SDPLine::SessionName("-".to_string()),
                    SDPLine::SessionTime(SessionTime {
                        start_time: 0,
                        end_time: 0,
                    }),
                    SDPLine::Attribute(Attribute::MediaGroup(MediaGroup::Bundle(vec![
                        "0".to_string(),
                        "1".to_string(),
                    ]))),
                    SDPLine::Attribute(Attribute::MediaGroup(MediaGroup::LipSync(vec![
                        "0".to_string(),
                        "1".to_string(),
                    ]))),
                    SDPLine::Attribute(Attribute::Unrecognized),
                    SDPLine::Attribute(Attribute::Unrecognized),
                    SDPLine::Attribute(Attribute::ICEUsername(ICEUsername {
                        username: "E2Fr".to_string(),
                    })),
                    SDPLine::Attribute(Attribute::ICEPassword(ICEPassword {
                        password: "OpQzg1PAwUdeOB244chlgd".to_string(),
                    })),
                    SDPLine::Attribute(Attribute::ICEOptions(ICEOptions {
                        options: vec![ICEOption::Trickle],
                    })),
                    SDPLine::Attribute(Attribute::Fingerprint(Fingerprint{
                        hash_function: HashFunction::SHA256,
                        hash: "EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B".to_string()
                    }))
                ];

                let expected_audio_media = vec![
                    SDPLine::MediaDescription(MediaDescription {
                        media_type: MediaType::Audio,
                        transport_port: 4557,
                        transport_protocol: MediaTransportProtocol::DtlsSrtp,
                        media_format_description: vec![111],
                    }),
                    SDPLine::ConnectionData(ConnectionData {
                        ip: IpAddr::V4(Ipv4Addr::from([192, 168, 0, 198])),
                    }),
                    SDPLine::Attribute(Attribute::MediaID(MediaID {
                        id: "0".to_string(),
                    })),
                    SDPLine::Attribute(Attribute::SendOnly),
                    SDPLine::Attribute(Attribute::MediaSSRC(MediaSSRC { ssrc: 1349455989 })),
                    SDPLine::Attribute(Attribute::MediaSSRC(MediaSSRC { ssrc: 1349455989 })),
                    SDPLine::Attribute(Attribute::Unrecognized),
                    SDPLine::Attribute(Attribute::RTCPMux),
                    SDPLine::Attribute(Attribute::RTPMap(RTPMap {
                        codec: MediaCodec::Audio(AudioCodec::Opus),
                        payload_number: 111,
                    })),
                    SDPLine::Attribute(Attribute::FMTP(FMTP {
                        payload_number: 111,
                        format_capability: HashSet::from([
                            "minptime=10".to_string(),
                            "maxaveragebitrate=96000".to_string(),
                            "stereo=1".to_string(),
                            "sprop-stereo=1".to_string(),
                            "useinbandfec=1".to_string(),
                        ]),
                    })),
                    SDPLine::Attribute(Attribute::Candidate(Candidate {
                        connection_address: IpAddr::V4(Ipv4Addr::from([192, 168, 0, 198])),
                        port: 4557,
                        priority: 2015363327,
                        component_id: 1,
                        foundation: "1".to_string(),
                    })),
                    SDPLine::Attribute(Attribute::Candidate(Candidate {
                        connection_address: IpAddr::V6(
                            Ipv6Addr::from_str("fe80::6c3d:5b42:1532:2f9a")
                                .expect("IPv6 string representation should be correct"),
                        ),
                        port: 10007,
                        priority: 2015363583,
                        component_id: 1,
                        foundation: "2".to_string(),
                    })),
                    SDPLine::Attribute(Attribute::EndOfCandidates),
                ];

                let expected_video_session = vec![
                    SDPLine::MediaDescription(MediaDescription {
                        media_type: MediaType::Video,
                        transport_port: 4557,
                        transport_protocol: MediaTransportProtocol::DtlsSrtp,
                        media_format_description: vec![96],
                    }),
                    SDPLine::ConnectionData(ConnectionData {
                        ip: IpAddr::V4(Ipv4Addr::from([192, 168, 0, 198])),
                    }),
                    SDPLine::Attribute(Attribute::MediaID(MediaID {
                        id: "1".to_string(),
                    })),
                    SDPLine::Attribute(Attribute::SendOnly),
                    SDPLine::Attribute(Attribute::MediaSSRC(MediaSSRC { ssrc: 1349455990 })),
                    SDPLine::Attribute(Attribute::MediaSSRC(MediaSSRC { ssrc: 1349455990 })),
                    SDPLine::Attribute(Attribute::Unrecognized),
                    SDPLine::Attribute(Attribute::RTCPMux),
                    SDPLine::Attribute(Attribute::RTPMap(RTPMap {
                        codec: MediaCodec::Video(VideoCodec::H264),
                        payload_number: 96,
                    })),
                    SDPLine::Attribute(Attribute::Unrecognized),
                    SDPLine::Attribute(Attribute::Unrecognized),
                    SDPLine::Attribute(Attribute::Unrecognized),
                    SDPLine::Attribute(Attribute::FMTP(FMTP {
                        payload_number: 96,
                        format_capability: HashSet::from([
                            "profile-level-id=42e01f".to_string(),
                            "packetization-mode=1".to_string(),
                            "level-asymmetry-allowed=1".to_string(),
                        ]),
                    })),
                ];

                assert!(
                    result.session_section.eq(&expected_session_media),
                    "Resolved session media should match expected session media"
                );
                assert!(
                    result.audio_section.eq(&expected_audio_media),
                    "Resolved audio media should match expected audio media"
                );
                assert!(
                    result.video_section.eq(&expected_video_session),
                    "Resolved video media should match expected video media"
                );
            }

            #[test]
            fn rejects_sdp_with_extra_media() {
                let invalid_sdp = "v=0\r\no=rtc 3767197920 0 IN IP4 127.0.0.1\r\ns=-\r\nt=0 0\r\na=group:BUNDLE 0 1\r\na=group:LS 0 1\r\na=msid-semantic:WMS *\r\na=setup:actpass\r\na=ice-ufrag:E2Fr\r\na=ice-pwd:OpQzg1PAwUdeOB244chlgd\r\na=ice-options:trickle\r\na=fingerprint:sha-256 EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B\r\nm=audio 4557 UDP/TLS/RTP/SAVPF 111\r\nc=IN IP4 192.168.0.198\r\na=mid:0\r\na=sendonly\r\na=ssrc:1349455989 cname:0X2NGAsK9XcmnsuZ\r\na=ssrc:1349455989 msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-audio\r\na=msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-audio\r\na=rtcp-mux\r\na=rtpmap:111 opus/48000/2\r\na=fmtp:111 minptime=10;maxaveragebitrate=96000;stereo=1;sprop-stereo=1;useinbandfec=1\r\na=candidate:1 1 UDP 2015363327 192.168.0.198 4557 typ host\r\na=candidate:2 1 UDP 2015363583 fe80::6c3d:5b42:1532:2f9a 10007 typ host\r\na=end-of-candidates\r\nm=video 4557 UDP/TLS/RTP/SAVPF 96\r\nc=IN IP4 192.168.0.198\r\na=mid:1\r\na=sendonly\r\na=ssrc:1349455990 cname:0X2NGAsK9XcmnsuZ\r\na=ssrc:1349455990 msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-video\r\na=msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-video\r\na=rtcp-mux\r\na=rtpmap:96 H264/90000\r\na=rtcp-fb:96 nack\r\na=rtcp-fb:96 nack pli\r\na=rtcp-fb:96 goog-remb\r\na=fmtp:96 profile-level-id=42e01f;packetization-mode=1;level-asymmetry-allowed=1\r\nm=video 4557 UDP/TLS/RTP/SAVPF 96\r\nc=IN IP4 192.168.0.198\r\na=mid:1\r\na=sendonly\r\na=ssrc:1349455990 cname:0X2NGAsK9XcmnsuZ\r\na=ssrc:1349455990 msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-video\r\na=msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-video\r\na=rtcp-mux\r\na=rtpmap:96 H264/90000\r\na=rtcp-fb:96 nack\r\na=rtcp-fb:96 nack pli\r\na=rtcp-fb:96 goog-remb\r\na=fmtp:96 profile-level-id=42e01f;packetization-mode=1;level-asymmetry-allowed=1\r\n";

                SDPResolver::get_sdp(invalid_sdp).expect_err("Should reject SDP");
            }

            #[test]
            fn rejects_sdp_with_unrecognized_media() {
                let invalid_sdp = "v=0\r\no=rtc 3767197920 0 IN IP4 127.0.0.1\r\ns=-\r\nt=0 0\r\na=group:BUNDLE 0 1\r\na=group:LS 0 1\r\na=msid-semantic:WMS *\r\na=setup:actpass\r\na=ice-ufrag:E2Fr\r\na=ice-pwd:OpQzg1PAwUdeOB244chlgd\r\na=ice-options:trickle\r\na=fingerprint:sha-256 EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B\r\nm=text 4557 UDP/TLS/RTP/SAVPF 111\r\nc=IN IP4 192.168.0.198\r\na=mid:0\r\na=sendonly\r\na=ssrc:1349455989 cname:0X2NGAsK9XcmnsuZ\r\na=ssrc:1349455989 msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-audio\r\na=msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-audio\r\na=rtcp-mux\r\na=rtpmap:111 opus/48000/2\r\na=fmtp:111 minptime=10;maxaveragebitrate=96000;stereo=1;sprop-stereo=1;useinbandfec=1\r\na=candidate:1 1 UDP 2015363327 192.168.0.198 4557 typ host\r\na=candidate:2 1 UDP 2015363583 fe80::6c3d:5b42:1532:2f9a 10007 typ host\r\na=end-of-candidates\r\nm=video 4557 UDP/TLS/RTP/SAVPF 96\r\nc=IN IP4 192.168.0.198\r\na=mid:1\r\na=sendonly\r\na=ssrc:1349455990 cname:0X2NGAsK9XcmnsuZ\r\na=ssrc:1349455990 msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-video\r\na=msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-video\r\na=rtcp-mux\r\na=rtpmap:96 H264/90000\r\na=rtcp-fb:96 nack\r\na=rtcp-fb:96 nack pli\r\na=rtcp-fb:96 goog-remb\r\na=fmtp:96 profile-level-id=42e01f;packetization-mode=1;level-asymmetry-allowed=1\r\n";
                SDPResolver::get_sdp(invalid_sdp).expect_err("Should reject SDP");
            }

            #[test]
            fn rejects_sdp_with_one_media_section() {
                let invalid_sdp = "v=0\r\no=rtc 3767197920 0 IN IP4 127.0.0.1\r\ns=-\r\nt=0 0\r\na=group:BUNDLE 0 1\r\na=group:LS 0 1\r\na=msid-semantic:WMS *\r\na=setup:actpass\r\na=ice-ufrag:E2Fr\r\na=ice-pwd:OpQzg1PAwUdeOB244chlgd\r\na=ice-options:trickle\r\na=fingerprint:sha-256 EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B\r\nm=audio 4557 UDP/TLS/RTP/SAVPF 111\r\nc=IN IP4 192.168.0.198\r\na=mid:0\r\na=sendonly\r\na=ssrc:1349455989 cname:0X2NGAsK9XcmnsuZ\r\na=ssrc:1349455989 msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-audio\r\na=msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-audio\r\na=rtcp-mux\r\na=rtpmap:111 opus/48000/2\r\na=fmtp:111 minptime=10;maxaveragebitrate=96000;stereo=1;sprop-stereo=1;useinbandfec=1\r\na=candidate:1 1 UDP 2015363327 192.168.0.198 4557 typ host\r\na=candidate:2 1 UDP 2015363583 fe80::6c3d:5b42:1532:2f9a 10007 typ host\r\na=end-of-candidates\r\n";
                SDPResolver::get_sdp(invalid_sdp).expect_err("Should reject SDP");
            }

            #[test]
            fn rejects_sdp_with_incorrect_session_media_items_order() {
                let invalid_sdp = "v=0\r\no=rtc 3767197920 0 IN IP4 127.0.0.1\r\nt=0 0\r\ns=-\r\na=group:BUNDLE 0 1\r\na=group:LS 0 1\r\na=msid-semantic:WMS *\r\na=setup:actpass\r\na=ice-ufrag:E2Fr\r\na=ice-pwd:OpQzg1PAwUdeOB244chlgd\r\na=ice-options:trickle\r\na=fingerprint:sha-256 EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B\r\nm=audio 4557 UDP/TLS/RTP/SAVPF 111\r\nc=IN IP4 192.168.0.198\r\na=mid:0\r\na=sendonly\r\na=ssrc:1349455989 cname:0X2NGAsK9XcmnsuZ\r\na=ssrc:1349455989 msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-audio\r\na=msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-audio\r\na=rtcp-mux\r\na=rtpmap:111 opus/48000/2\r\na=fmtp:111 minptime=10;maxaveragebitrate=96000;stereo=1;sprop-stereo=1;useinbandfec=1\r\na=candidate:1 1 UDP 2015363327 192.168.0.198 4557 typ host\r\na=candidate:2 1 UDP 2015363583 fe80::6c3d:5b42:1532:2f9a 10007 typ host\r\na=end-of-candidates\r\nm=video 4557 UDP/TLS/RTP/SAVPF 96\r\nc=IN IP4 192.168.0.198\r\na=mid:1\r\na=sendonly\r\na=ssrc:1349455990 cname:0X2NGAsK9XcmnsuZ\r\na=ssrc:1349455990 msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-video\r\na=msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-video\r\na=rtcp-mux\r\na=rtpmap:96 H264/90000\r\na=rtcp-fb:96 nack\r\na=rtcp-fb:96 nack pli\r\na=rtcp-fb:96 goog-remb\r\na=fmtp:96 profile-level-id=42e01f;packetization-mode=1;level-asymmetry-allowed=1\r\n";
                SDPResolver::get_sdp(invalid_sdp).expect_err("Should reject SDP");
            }

            #[test]
            fn rejects_sdp_with_missing_required_session_media_items() {
                let invalid_sdp = "v=0\r\no=rtc 3767197920 0 IN IP4 127.0.0.1\r\ns=-\r\na=group:BUNDLE 0 1\r\na=group:LS 0 1\r\na=msid-semantic:WMS *\r\na=setup:actpass\r\na=ice-ufrag:E2Fr\r\na=ice-pwd:OpQzg1PAwUdeOB244chlgd\r\na=ice-options:trickle\r\na=fingerprint:sha-256 EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B\r\nm=audio 4557 UDP/TLS/RTP/SAVPF 111\r\nc=IN IP4 192.168.0.198\r\na=mid:0\r\na=sendonly\r\na=ssrc:1349455989 cname:0X2NGAsK9XcmnsuZ\r\na=ssrc:1349455989 msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-audio\r\na=msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-audio\r\na=rtcp-mux\r\na=rtpmap:111 opus/48000/2\r\na=fmtp:111 minptime=10;maxaveragebitrate=96000;stereo=1;sprop-stereo=1;useinbandfec=1\r\na=candidate:1 1 UDP 2015363327 192.168.0.198 4557 typ host\r\na=candidate:2 1 UDP 2015363583 fe80::6c3d:5b42:1532:2f9a 10007 typ host\r\na=end-of-candidates\r\nm=video 4557 UDP/TLS/RTP/SAVPF 96\r\nc=IN IP4 192.168.0.198\r\na=mid:1\r\na=sendonly\r\na=ssrc:1349455990 cname:0X2NGAsK9XcmnsuZ\r\na=ssrc:1349455990 msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-video\r\na=msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-video\r\na=rtcp-mux\r\na=rtpmap:96 H264/90000\r\na=rtcp-fb:96 nack\r\na=rtcp-fb:96 nack pli\r\na=rtcp-fb:96 goog-remb\r\na=fmtp:96 profile-level-id=42e01f;packetization-mode=1;level-asymmetry-allowed=1\r\n";
                SDPResolver::get_sdp(invalid_sdp).expect_err("Should reject SDP");
            }
        }

        mod get_ice_credentials {
            use crate::line_parsers::{Attribute, ICEPassword, ICEUsername, SDPLine};
            use crate::resolvers::{SDP, SDPResolver};

            #[test]
            fn resolves_sdp_with_default_credentials() {
                let expected_ice_username = ICEUsername {
                    username: "test".to_string(),
                };

                let expected_ice_password = ICEPassword {
                    password: "test".to_string(),
                };

                let sdp = SDP {
                    session_section: vec![
                        SDPLine::Attribute(Attribute::ICEUsername(expected_ice_username.clone())),
                        SDPLine::Attribute(Attribute::ICEPassword(expected_ice_password.clone())),
                    ],
                    video_section: vec![],
                    audio_section: vec![],
                };

                let ice_credentials =
                    SDPResolver::get_ice_credentials(&sdp).expect("Should resolve ICE credentials");

                assert_eq!(
                    ice_credentials.remote_username, expected_ice_username.username,
                    "Remote username should match expected username"
                );
                assert_eq!(
                    ice_credentials.remote_password, expected_ice_password.password,
                    "Remote password should match expected password"
                );
            }

            #[test]
            fn resolves_sdp_with_media_credentials() {
                let expected_ice_username = ICEUsername {
                    username: "test".to_string(),
                };

                let expected_ice_password = ICEPassword {
                    password: "test".to_string(),
                };

                let sdp = SDP {
                    session_section: vec![],
                    video_section: vec![
                        SDPLine::Attribute(Attribute::ICEUsername(expected_ice_username.clone())),
                        SDPLine::Attribute(Attribute::ICEPassword(expected_ice_password.clone())),
                    ],
                    audio_section: vec![
                        SDPLine::Attribute(Attribute::ICEUsername(expected_ice_username.clone())),
                        SDPLine::Attribute(Attribute::ICEPassword(expected_ice_password.clone())),
                    ],
                };

                let ice_credentials =
                    SDPResolver::get_ice_credentials(&sdp).expect("Should resolve ICE credentials");

                assert_eq!(
                    ice_credentials.remote_username, expected_ice_username.username,
                    "Remote username should match expected username"
                );
                assert_eq!(
                    ice_credentials.remote_password, expected_ice_password.password,
                    "Remote password should match expected password"
                );
            }

            #[test]
            fn selects_media_level_ice_credentials_over_defaults() {
                let expected_ice_username = ICEUsername {
                    username: "test".to_string(),
                };

                let expected_ice_password = ICEPassword {
                    password: "test".to_string(),
                };

                let sdp = SDP {
                    session_section: vec![
                        SDPLine::Attribute(Attribute::ICEUsername(ICEUsername {
                            username: "default-username".to_string(),
                        })),
                        SDPLine::Attribute(Attribute::ICEPassword(ICEPassword {
                            password: "default-password".to_string(),
                        })),
                    ],
                    video_section: vec![
                        SDPLine::Attribute(Attribute::ICEUsername(expected_ice_username.clone())),
                        SDPLine::Attribute(Attribute::ICEPassword(expected_ice_password.clone())),
                    ],
                    audio_section: vec![
                        SDPLine::Attribute(Attribute::ICEUsername(expected_ice_username.clone())),
                        SDPLine::Attribute(Attribute::ICEPassword(expected_ice_password.clone())),
                    ],
                };

                let ice_credentials =
                    SDPResolver::get_ice_credentials(&sdp).expect("Should resolve ICE credentials");

                assert_eq!(
                    ice_credentials.remote_username, expected_ice_username.username,
                    "Remote username should match expected username"
                );
                assert_eq!(
                    ice_credentials.remote_password, expected_ice_password.password,
                    "Remote password should match expected password"
                );
            }

            #[test]
            fn rejects_sdp_with_partial_media_credentials() {
                let expected_ice_username = ICEUsername {
                    username: "test".to_string(),
                };

                let expected_ice_password = ICEPassword {
                    password: "test".to_string(),
                };
                let sdp = SDP {
                    session_section: vec![],
                    video_section: vec![
                        SDPLine::Attribute(Attribute::ICEUsername(expected_ice_username.clone())),
                        SDPLine::Attribute(Attribute::ICEPassword(expected_ice_password.clone())),
                    ],
                    audio_section: vec![],
                };

                let ice_credentials = SDPResolver::get_ice_credentials(&sdp);

                assert!(ice_credentials.is_none(), "Should reject SDP")
            }

            #[test]
            fn rejects_sdp_with_partial_media_credentials_and_default_credentials() {
                let expected_ice_username = ICEUsername {
                    username: "test".to_string(),
                };

                let expected_ice_password = ICEPassword {
                    password: "test".to_string(),
                };
                let sdp = SDP {
                    session_section: vec![
                        SDPLine::Attribute(Attribute::ICEUsername(ICEUsername {
                            username: "default-username".to_string(),
                        })),
                        SDPLine::Attribute(Attribute::ICEPassword(ICEPassword {
                            password: "default-password".to_string(),
                        })),
                    ],
                    video_section: vec![
                        SDPLine::Attribute(Attribute::ICEUsername(expected_ice_username.clone())),
                        SDPLine::Attribute(Attribute::ICEPassword(expected_ice_password.clone())),
                    ],
                    audio_section: vec![],
                };

                let ice_credentials = SDPResolver::get_ice_credentials(&sdp);

                assert!(ice_credentials.is_none(), "Should reject SDP")
            }

            #[test]
            fn rejects_sdp_without_ice_credentials() {
                let sdp = SDP {
                    session_section: vec![],
                    video_section: vec![],
                    audio_section: vec![],
                };

                let ice_credentials = SDPResolver::get_ice_credentials(&sdp);

                assert!(ice_credentials.is_none(), "Should reject SDP")
            }
        }

        mod get_media_ids {
            use crate::line_parsers::{Attribute, MediaGroup, MediaID, SDPLine};
            use crate::resolvers::{SDP, SDPResolver};

            #[test]
            fn gets_media_ids_of_valid_sdp() {
                let expected_audio_id = MediaID {
                    id: "0".to_string(),
                };
                let expected_video_id = MediaID {
                    id: "1".to_string(),
                };

                let sdp = SDP {
                    session_section: vec![SDPLine::Attribute(Attribute::MediaGroup(
                        MediaGroup::Bundle(vec!["0".to_string(), "1".to_string()]),
                    ))],
                    audio_section: vec![SDPLine::Attribute(Attribute::MediaID(
                        expected_audio_id.clone(),
                    ))],
                    video_section: vec![SDPLine::Attribute(Attribute::MediaID(
                        expected_video_id.clone(),
                    ))],
                };

                let (actual_audio_id, actual_video_id) =
                    SDPResolver::get_media_ids(&sdp).expect("Should resolve media ids");

                assert_eq!(
                    actual_audio_id, expected_audio_id,
                    "Audio media ids should match"
                );
                assert_eq!(
                    actual_video_id, expected_video_id,
                    "Video media ids should match"
                )
            }

            #[test]
            fn rejects_if_mediaid_doesnt_match_bundle() {
                let sdp = SDP {
                    session_section: vec![SDPLine::Attribute(Attribute::MediaGroup(
                        MediaGroup::Bundle(vec!["0".to_string(), "1".to_string()]),
                    ))],
                    audio_section: vec![SDPLine::Attribute(Attribute::MediaID(MediaID {
                        id: "0".to_string(),
                    }))],
                    video_section: vec![SDPLine::Attribute(Attribute::MediaID(MediaID {
                        id: "2".to_string(),
                    }))],
                };

                SDPResolver::get_media_ids(&sdp).expect_err("Should reject SDP");
            }
            #[test]
            fn rejects_if_missing_bundle() {
                let sdp = SDP {
                    session_section: vec![],
                    audio_section: vec![SDPLine::Attribute(Attribute::MediaID(MediaID {
                        id: "0".to_string(),
                    }))],
                    video_section: vec![SDPLine::Attribute(Attribute::MediaID(MediaID {
                        id: "1".to_string(),
                    }))],
                };

                SDPResolver::get_media_ids(&sdp).expect_err("Should reject SDP");
            }
        }
        mod get_streamer_audio_session {
            use std::collections::HashSet;

            use crate::line_parsers::{
                Attribute, AudioCodec, FMTP, MediaCodec, MediaSSRC, RTPMap, SDPLine,
            };
            use crate::resolvers::SDPResolver;

            #[test]
            fn resolves_valid_sdp() {
                let expected_payload_number: usize = 96;
                let expected_ssrc: u32 = 1;
                let audio_media = vec![
                    SDPLine::Attribute(Attribute::SendOnly),
                    SDPLine::Attribute(Attribute::RTCPMux),
                    SDPLine::Attribute(Attribute::FMTP(FMTP {
                        payload_number: expected_payload_number,
                        format_capability: HashSet::new(),
                    })),
                    SDPLine::Attribute(Attribute::RTPMap(RTPMap {
                        payload_number: expected_payload_number,
                        codec: MediaCodec::Audio(AudioCodec::Opus),
                    })),
                    SDPLine::Attribute(Attribute::MediaSSRC(MediaSSRC {
                        ssrc: expected_ssrc,
                    })),
                ];
                let audio_session = SDPResolver::get_streamer_audio_session(&audio_media)
                    .expect("Should resolve to OK");

                assert_eq!(audio_session.codec, AudioCodec::Opus);
                assert_eq!(audio_session.payload_number, expected_payload_number);
                assert_eq!(audio_session.remote_ssrc, expected_ssrc);
            }

            #[test]
            fn reject_media_with_missing_ssrc() {
                let expected_payload_number: usize = 96;
                let audio_media = vec![
                    SDPLine::Attribute(Attribute::SendOnly),
                    SDPLine::Attribute(Attribute::RTCPMux),
                    SDPLine::Attribute(Attribute::FMTP(FMTP {
                        payload_number: expected_payload_number,
                        format_capability: HashSet::new(),
                    })),
                    SDPLine::Attribute(Attribute::RTPMap(RTPMap {
                        payload_number: expected_payload_number,
                        codec: MediaCodec::Audio(AudioCodec::Opus),
                    })),
                ];

                SDPResolver::get_streamer_audio_session(&audio_media)
                    .expect_err("Should reject audio media");
            }

            #[test]
            fn reject_media_with_missing_rtmp() {
                let expected_payload_number: usize = 96;
                let audio_media = vec![
                    SDPLine::Attribute(Attribute::SendOnly),
                    SDPLine::Attribute(Attribute::RTCPMux),
                    SDPLine::Attribute(Attribute::FMTP(FMTP {
                        payload_number: expected_payload_number,
                        format_capability: HashSet::new(),
                    })),
                    SDPLine::Attribute(Attribute::MediaSSRC(MediaSSRC { ssrc: 1 })),
                ];

                SDPResolver::get_streamer_audio_session(&audio_media)
                    .expect_err("Should reject audio media");
            }

            #[test]
            fn reject_media_with_invalid_direction() {
                let expected_payload_number: usize = 96;
                let audio_media = vec![
                    SDPLine::Attribute(Attribute::ReceiveOnly),
                    SDPLine::Attribute(Attribute::RTCPMux),
                    SDPLine::Attribute(Attribute::RTPMap(RTPMap {
                        payload_number: expected_payload_number,
                        codec: MediaCodec::Audio(AudioCodec::Opus),
                    })),
                    SDPLine::Attribute(Attribute::MediaSSRC(MediaSSRC { ssrc: 1 })),
                ];

                SDPResolver::get_streamer_audio_session(&audio_media)
                    .expect_err("Should reject audio media");
            }

            #[test]
            fn reject_non_demuxed_media() {
                let expected_payload_number: usize = 96;
                let audio_media = vec![
                    SDPLine::Attribute(Attribute::SendOnly),
                    SDPLine::Attribute(Attribute::RTPMap(RTPMap {
                        payload_number: expected_payload_number,
                        codec: MediaCodec::Audio(AudioCodec::Opus),
                    })),
                    SDPLine::Attribute(Attribute::MediaSSRC(MediaSSRC { ssrc: 1 })),
                ];

                SDPResolver::get_streamer_audio_session(&audio_media)
                    .expect_err("Should reject audio media");
            }

            #[test]
            fn reject_media_with_unsupported_codec() {
                let expected_payload_number: usize = 96;
                let audio_media = vec![
                    SDPLine::Attribute(Attribute::SendOnly),
                    SDPLine::Attribute(Attribute::RTCPMux),
                    SDPLine::Attribute(Attribute::RTPMap(RTPMap {
                        payload_number: expected_payload_number,
                        codec: MediaCodec::Unsupported,
                    })),
                    SDPLine::Attribute(Attribute::MediaSSRC(MediaSSRC { ssrc: 1 })),
                ];

                SDPResolver::get_streamer_audio_session(&audio_media)
                    .expect_err("Should reject audio media");
            }
        }

        mod get_streamer_video_session {
            use std::collections::HashSet;

            use crate::line_parsers::{
                Attribute, FMTP, MediaCodec, MediaSSRC, RTPMap, SDPLine, VideoCodec,
            };
            use crate::resolvers::SDPResolver;

            #[test]
            fn resolves_valid_media() {
                let expected_payload_number: usize = 96;
                let expected_ssrc: u32 = 1;
                let expected_capabilities = HashSet::from(["profile-test".to_string()]);
                let video_media = vec![
                    SDPLine::Attribute(Attribute::SendOnly),
                    SDPLine::Attribute(Attribute::RTCPMux),
                    SDPLine::Attribute(Attribute::FMTP(FMTP {
                        payload_number: expected_payload_number,
                        format_capability: expected_capabilities.clone(),
                    })),
                    SDPLine::Attribute(Attribute::RTPMap(RTPMap {
                        payload_number: expected_payload_number,
                        codec: MediaCodec::Video(VideoCodec::H264),
                    })),
                    SDPLine::Attribute(Attribute::MediaSSRC(MediaSSRC {
                        ssrc: expected_ssrc,
                    })),
                ];

                let video_session = SDPResolver::get_streamer_video_session(&video_media)
                    .expect("Should resolve video media");

                assert_eq!(video_session.codec, VideoCodec::H264);
                assert_eq!(video_session.payload_number, expected_payload_number);
                assert_eq!(video_session.remote_ssrc, expected_ssrc);
                assert_eq!(video_session.capabilities, expected_capabilities);
            }

            #[test]
            fn rejects_media_with_missing_ssrc() {
                let expected_payload_number: usize = 96;
                let expected_capabilities = HashSet::from(["profile-test".to_string()]);
                let video_media = vec![
                    SDPLine::Attribute(Attribute::SendOnly),
                    SDPLine::Attribute(Attribute::RTCPMux),
                    SDPLine::Attribute(Attribute::FMTP(FMTP {
                        payload_number: expected_payload_number,
                        format_capability: expected_capabilities.clone(),
                    })),
                    SDPLine::Attribute(Attribute::RTPMap(RTPMap {
                        payload_number: expected_payload_number,
                        codec: MediaCodec::Video(VideoCodec::H264),
                    })),
                ];

                SDPResolver::get_streamer_video_session(&video_media)
                    .expect_err("Should reject media");
            }
            #[test]
            fn rejects_media_with_unsupported_codec() {
                let expected_payload_number: usize = 96;
                let expected_ssrc: u32 = 1;
                let expected_capabilities = HashSet::from(["profile-test".to_string()]);
                let video_media = vec![
                    SDPLine::Attribute(Attribute::SendOnly),
                    SDPLine::Attribute(Attribute::RTCPMux),
                    SDPLine::Attribute(Attribute::FMTP(FMTP {
                        payload_number: expected_payload_number,
                        format_capability: expected_capabilities.clone(),
                    })),
                    SDPLine::Attribute(Attribute::RTPMap(RTPMap {
                        payload_number: expected_payload_number,
                        codec: MediaCodec::Unsupported,
                    })),
                    SDPLine::Attribute(Attribute::MediaSSRC(MediaSSRC {
                        ssrc: expected_ssrc,
                    })),
                ];

                SDPResolver::get_streamer_video_session(&video_media)
                    .expect_err("Should reject media");
            }

            #[test]
            fn rejects_media_with_missing_fmtp() {
                let expected_payload_number: usize = 96;
                let expected_ssrc: u32 = 1;
                let video_media = vec![
                    SDPLine::Attribute(Attribute::SendOnly),
                    SDPLine::Attribute(Attribute::RTCPMux),
                    SDPLine::Attribute(Attribute::RTPMap(RTPMap {
                        payload_number: expected_payload_number,
                        codec: MediaCodec::Video(VideoCodec::H264),
                    })),
                    SDPLine::Attribute(Attribute::MediaSSRC(MediaSSRC {
                        ssrc: expected_ssrc,
                    })),
                ];

                SDPResolver::get_streamer_video_session(&video_media)
                    .expect_err("Should reject media");
            }

            #[test]
            fn rejects_non_demuxed_media() {
                let expected_payload_number: usize = 96;
                let expected_ssrc: u32 = 1;
                let expected_capabilities = HashSet::from(["profile-test".to_string()]);

                let video_media = vec![
                    SDPLine::Attribute(Attribute::SendOnly),
                    SDPLine::Attribute(Attribute::RTPMap(RTPMap {
                        payload_number: expected_payload_number,
                        codec: MediaCodec::Video(VideoCodec::H264),
                    })),
                    SDPLine::Attribute(Attribute::MediaSSRC(MediaSSRC {
                        ssrc: expected_ssrc,
                    })),
                    SDPLine::Attribute(Attribute::FMTP(FMTP {
                        payload_number: expected_payload_number,
                        format_capability: expected_capabilities,
                    })),
                ];

                SDPResolver::get_streamer_video_session(&video_media)
                    .expect_err("Should reject media");
            }

            #[test]
            fn rejects_invalid_direction_media() {
                let expected_payload_number: usize = 96;
                let expected_ssrc: u32 = 1;
                let expected_capabilities = HashSet::from(["profile-test".to_string()]);

                let video_media = vec![
                    SDPLine::Attribute(Attribute::ReceiveOnly),
                    SDPLine::Attribute(Attribute::RTCPMux),
                    SDPLine::Attribute(Attribute::RTPMap(RTPMap {
                        payload_number: expected_payload_number,
                        codec: MediaCodec::Video(VideoCodec::H264),
                    })),
                    SDPLine::Attribute(Attribute::MediaSSRC(MediaSSRC {
                        ssrc: expected_ssrc,
                    })),
                    SDPLine::Attribute(Attribute::FMTP(FMTP {
                        payload_number: expected_payload_number,
                        format_capability: expected_capabilities,
                    })),
                ];

                SDPResolver::get_streamer_video_session(&video_media)
                    .expect_err("Should reject media");
            }
        }

        mod get_viewer_audio_session {
            use crate::line_parsers::{
                Attribute, AudioCodec, MediaCodec, MediaSSRC, RTPMap, SDPLine,
            };
            use crate::resolvers::{AudioSession, SDPResolver};

            fn init_streamer_session() -> AudioSession {
                let audio_session = AudioSession {
                    codec: AudioCodec::Opus,
                    remote_ssrc: 2,
                    host_ssrc: 1,
                    payload_number: 111,
                };

                audio_session
            }

            #[test]
            fn resolves_valid_media() {
                let streamer_session = init_streamer_session();

                let expected_payload_number = 96;
                let expected_ssrc = 2;

                let audio_media = vec![
                    SDPLine::Attribute(Attribute::ReceiveOnly),
                    SDPLine::Attribute(Attribute::RTCPMux),
                    SDPLine::Attribute(Attribute::MediaSSRC(MediaSSRC {
                        ssrc: expected_ssrc,
                    })),
                    SDPLine::Attribute(Attribute::RTPMap(RTPMap {
                        codec: MediaCodec::Audio(streamer_session.codec.clone()),
                        payload_number: expected_payload_number,
                    })),
                ];

                let audio_session =
                    SDPResolver::get_viewer_audio_session(&audio_media, &streamer_session)
                        .expect("Should resolve media");

                assert_eq!(audio_session.codec, streamer_session.codec);
                assert_eq!(audio_session.payload_number, expected_payload_number);
                assert_eq!(audio_session.remote_ssrc, expected_ssrc)
            }

            #[test]
            fn rejects_media_mismatching_streamer_codec() {
                let streamer_session = init_streamer_session();

                let expected_payload_number = 96;
                let expected_ssrc = 2;

                let audio_media = vec![
                    SDPLine::Attribute(Attribute::ReceiveOnly),
                    SDPLine::Attribute(Attribute::RTCPMux),
                    SDPLine::Attribute(Attribute::MediaSSRC(MediaSSRC {
                        ssrc: expected_ssrc,
                    })),
                    SDPLine::Attribute(Attribute::RTPMap(RTPMap {
                        codec: MediaCodec::Unsupported,
                        payload_number: expected_payload_number,
                    })),
                ];

                SDPResolver::get_viewer_audio_session(&audio_media, &streamer_session)
                    .expect_err("Should reject media");
            }

            #[test]
            fn rejects_media_missing_ssrc() {
                let streamer_session = init_streamer_session();

                let expected_payload_number = 96;

                let audio_media = vec![
                    SDPLine::Attribute(Attribute::ReceiveOnly),
                    SDPLine::Attribute(Attribute::RTCPMux),
                    SDPLine::Attribute(Attribute::RTPMap(RTPMap {
                        codec: MediaCodec::Audio(streamer_session.codec.clone()),
                        payload_number: expected_payload_number,
                    })),
                ];

                SDPResolver::get_viewer_audio_session(&audio_media, &streamer_session)
                    .expect_err("Should reject media");
            }

            #[test]
            fn rejects_media_with_invalid_media_direction() {
                let streamer_session = init_streamer_session();

                let expected_payload_number = 96;
                let expected_ssrc = 2;

                let audio_media = vec![
                    SDPLine::Attribute(Attribute::SendOnly),
                    SDPLine::Attribute(Attribute::RTCPMux),
                    SDPLine::Attribute(Attribute::MediaSSRC(MediaSSRC {
                        ssrc: expected_ssrc,
                    })),
                    SDPLine::Attribute(Attribute::RTPMap(RTPMap {
                        codec: MediaCodec::Audio(streamer_session.codec.clone()),
                        payload_number: expected_payload_number,
                    })),
                ];

                SDPResolver::get_viewer_audio_session(&audio_media, &streamer_session)
                    .expect_err("Should reject media");
            }

            #[test]
            fn rejects_non_demuxed_media() {
                let streamer_session = init_streamer_session();

                let expected_payload_number = 96;
                let expected_ssrc = 2;

                let audio_media = vec![
                    SDPLine::Attribute(Attribute::ReceiveOnly),
                    SDPLine::Attribute(Attribute::MediaSSRC(MediaSSRC {
                        ssrc: expected_ssrc,
                    })),
                    SDPLine::Attribute(Attribute::RTPMap(RTPMap {
                        codec: MediaCodec::Audio(streamer_session.codec.clone()),
                        payload_number: expected_payload_number,
                    })),
                ];

                SDPResolver::get_viewer_audio_session(&audio_media, &streamer_session)
                    .expect_err("Should reject media");
            }
        }

        // mod parse_stream_offer {
        //     use std::net::{IpAddr, Ipv4Addr, SocketAddr};
        //
        //     use crate::line_parsers::{Attribute, ICEPassword, ICEUsername, MediaGroup, SDPLine};
        //     use crate::resolvers::{SDP, SDPResolver};
        //
        //     fn init_sdp_resolver() -> SDPResolver {
        //         let fingerprint = "sha-256 EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B";
        //         let ip_address = IpAddr::V4(Ipv4Addr::LOCALHOST);
        //         let socket_addr = SocketAddr::new(ip_address, 5200);
        //         SDPResolver::new(fingerprint, socket_addr)
        //     }
        //     #[test]
        //     fn resolves_valid_offer() {
        //         let sdp_resolver = init_sdp_resolver();
        //
        //         let expected_ice_username = ICEUsername {
        //             username: "test".to_string(),
        //         };
        //         let expected_ice_password = ICEPassword {
        //             password: "test".to_string(),
        //         };
        //
        //         let sdp_offer = SDP {
        //             session_section: vec![
        //                 SDPLine::Attribute(Attribute::ICEUsername(expected_ice_username.clone())),
        //                 SDPLine::Attribute(Attribute::ICEPassword(expected_ice_password.clone())),
        //                 SDPLine::Attribute(Attribute::MediaGroup(MediaGroup::Bundle(vec![
        //                     "0".to_string(),
        //                     "1".to_string(),
        //                 ]))),
        //             ],
        //             audio_section: vec![],
        //             video_section: vec![],
        //         };
        //     }
        // }
    }
}
