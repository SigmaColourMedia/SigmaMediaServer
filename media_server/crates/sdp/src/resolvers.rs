use std::net::SocketAddr;

use rand::{Rng, thread_rng};
use rand::distributions::Alphanumeric;

use crate::line_parsers::{
    AudioCodec, Candidate, Fingerprint, FMTP, ICEPassword, ICEUsername, MediaSSRC, MediaType,
    SDPLine, SDPParseError, VideoCodec,
};

fn test() {}
#[derive(Debug, Clone)]
pub struct ResolvedSDP {
    host_ice_username: ICEUsername,
    host_ice_password: ICEPassword,
    remote_ice_username: ICEUsername,
    remote_ice_password: ICEPassword,
    video_codec: VideoCodec,
    video_payload_number: usize,
    video_capability: FMTP,
    remote_video_ssrc: MediaSSRC,
    audio_codec: AudioCodec,
    audio_payload_number: usize,
    remote_audio_ssrc: MediaSSRC,
    agent_type: AgentType,
}

#[derive(Debug)]
struct SDP {
    session_section: Vec<SDPLine>,
    video_section: Vec<SDPLine>,
    audio_section: Vec<SDPLine>,
}

#[derive(Debug, Clone)]
enum AgentType {
    Streamer,
    Viewer,
}

#[derive(Debug, PartialEq)]
pub enum StreamerOfferSDPParseError {
    UnsupportedMediaCodecs,
    DemuxRequired,
    MissingVideoProfileSettings,
    MissingRemoteSSRC,
    UnsupportedMediaDirection,
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

    // pub fn accept_streamer_sdp(
    //     &self,
    //     offer: SDPOffer,
    // ) -> Result<ResolvedSDP, StreamerOfferSDPParseError> {
    //     let find_payload_number_by_media_codec =
    //         |media_attributes: &Vec<Attribute>, media_codec: MediaCodec| {
    //             media_attributes
    //                 .iter()
    //                 .find_map(|attr| match attr {
    //                     Attribute::RTPMap(rtpmap) => {
    //                         if rtpmap.codec.eq(&media_codec) {
    //                             return Some(rtpmap);
    //                         }
    //                         return None;
    //                     }
    //                     _ => None,
    //                 })
    //                 .ok_or(StreamerOfferSDPParseError::UnsupportedMediaCodecs)
    //         };
    //
    //     // Check if video and audio is demuxed
    //     let is_video_demuxed = offer
    //         .video_media_description
    //         .iter()
    //         .find(|attr| match attr {
    //             Attribute::RTCPMux => true,
    //             _ => false,
    //         })
    //         .is_some();
    //
    //     if !is_video_demuxed {
    //         return Err(StreamerOfferSDPParseError::DemuxRequired);
    //     }
    //     let is_audio_demuxed = offer
    //         .audio_media_description
    //         .iter()
    //         .find(|attr| match attr {
    //             Attribute::RTCPMux => true,
    //             _ => false,
    //         })
    //         .is_some();
    //     if !is_audio_demuxed {
    //         return Err(StreamerOfferSDPParseError::DemuxRequired);
    //     }
    //
    //     // Check if media direction is set to sendonly
    //     let is_video_sendonly = offer
    //         .video_media_description
    //         .iter()
    //         .find(|attr| match attr {
    //             Attribute::SendOnly => true,
    //             _ => false,
    //         })
    //         .is_some();
    //     if !is_video_sendonly {
    //         return Err(StreamerOfferSDPParseError::UnsupportedMediaDirection);
    //     }
    //
    //     let is_audio_sendonly = offer
    //         .audio_media_description
    //         .iter()
    //         .find(|attr| match attr {
    //             Attribute::SendOnly => true,
    //             _ => false,
    //         })
    //         .is_some();
    //     if !is_audio_sendonly {
    //         return Err(StreamerOfferSDPParseError::UnsupportedMediaDirection);
    //     }
    //
    //     let supported_video_rtpmap = find_payload_number_by_media_codec(
    //         offer.video_media_description.as_ref(),
    //         MediaCodec::Video(VideoCodec::H264),
    //     )?
    //     .clone();
    //     let supported_audio_rtpmap = find_payload_number_by_media_codec(
    //         offer.audio_media_description.as_ref(),
    //         MediaCodec::Audio(AudioCodec::Opus),
    //     )?
    //     .clone();
    //
    //     let supported_video_fmtp = offer
    //         .video_media_description
    //         .iter()
    //         .find_map(|attr| match attr {
    //             Attribute::FMTP(fmtp) => {
    //                 if fmtp
    //                     .payload_number
    //                     .eq(&supported_audio_rtpmap.payload_number)
    //                 {
    //                     return Some(fmtp.clone());
    //                 }
    //                 return None;
    //             }
    //             _ => None,
    //         })
    //         .ok_or(StreamerOfferSDPParseError::MissingVideoProfileSettings)?;
    //
    //     let video_ssrc = offer
    //         .video_media_description
    //         .iter()
    //         .find_map(|attr| match attr {
    //             Attribute::MediaSSRC(ssrc) => Some(ssrc.clone()),
    //             _ => None,
    //         })
    //         .ok_or(StreamerOfferSDPParseError::MissingRemoteSSRC)?;
    //     let audio_ssrc = offer
    //         .audio_media_description
    //         .iter()
    //         .find_map(|attr| match attr {
    //             Attribute::MediaSSRC(ssrc) => Some(ssrc.clone()),
    //             _ => None,
    //         })
    //         .ok_or(StreamerOfferSDPParseError::MissingRemoteSSRC)?;
    //
    //     Ok(ResolvedSDP {
    //         agent_type: AgentType::Streamer,
    //         fingerprint: self.fingerprint.clone(),
    //         video_mapping: supported_video_rtpmap,
    //         audio_mapping: supported_audio_rtpmap,
    //         host_ice_password: ICEPassword {
    //             password: get_random_string(20),
    //         },
    //         host_ice_username: ICEUsername {
    //             username: get_random_string(4),
    //         },
    //         remote_ice_username: offer.ice_username,
    //         remote_ice_password: offer.ice_password,
    //         video_capability: supported_video_fmtp,
    //         remote_video_ssrc: video_ssrc,
    //         remote_audio_ssrc: audio_ssrc,
    //     })
    // }
}

mod tests {
    mod sdp_resolver {
        mod get_sdp {
            use crate::resolvers::SDPResolver;

            const VALID_SDP: &str = "v=0\r\no=rtc 3767197920 0 IN IP4 127.0.0.1\r\ns=-\r\nt=0 0\r\na=group:BUNDLE 0 1\r\na=group:LS 0 1\r\na=msid-semantic:WMS *\r\na=setup:actpass\r\na=ice-ufrag:E2Fr\r\na=ice-pwd:OpQzg1PAwUdeOB244chlgd\r\na=ice-options:trickle\r\na=fingerprint:sha-256 EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B\r\nm=audio 4557 UDP/TLS/RTP/SAVPF 111\r\nc=IN IP4 192.168.0.198\r\na=mid:0\r\na=sendonly\r\na=ssrc:1349455989 cname:0X2NGAsK9XcmnsuZ\r\na=ssrc:1349455989 msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-audio\r\na=msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-audio\r\na=rtcp-mux\r\na=rtpmap:111 opus/48000/2\r\na=fmtp:111 minptime=10;maxaveragebitrate=96000;stereo=1;sprop-stereo=1;useinbandfec=1\r\na=candidate:1 1 UDP 2015363327 192.168.0.198 4557 typ host\r\na=candidate:2 1 UDP 2015363583 fe80::6c3d:5b42:1532:2f9a 10007 typ host\r\na=end-of-candidates\r\nm=video 4557 UDP/TLS/RTP/SAVPF 96\r\nc=IN IP4 192.168.0.198\r\na=mid:1\r\na=sendonly\r\na=ssrc:1349455990 cname:0X2NGAsK9XcmnsuZ\r\na=ssrc:1349455990 msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-video\r\na=msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-video\r\na=rtcp-mux\r\na=rtpmap:96 H264/90000\r\na=rtcp-fb:96 nack\r\na=rtcp-fb:96 nack pli\r\na=rtcp-fb:96 goog-remb\r\na=fmtp:96 profile-level-id=42e01f;packetization-mode=1;level-asymmetry-allowed=1\r\n";

            #[test]
            fn resolves_valid_sdp() {
                let result = SDPResolver::get_sdp(VALID_SDP).expect("Should return valid SDP");
                println!("{:?}", result)
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
