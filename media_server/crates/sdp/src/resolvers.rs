// use rand::{Rng, thread_rng};
// use rand::distributions::Alphanumeric;
//
// use crate::line_parsers::{
//     Attribute, AudioCodec, Fingerprint, FMTP, ICEPassword, ICEUsername, MediaCodec,
//     MediaSSRC, parse_fingerprint, RTPMap, SDPOffer, VideoCodec,
// };
//
fn test() {}
// #[derive(Debug)]
// pub struct ResolvedSDP {
//     host_ice_username: ICEUsername,
//     host_ice_password: ICEPassword,
//     remote_ice_username: ICEUsername,
//     remote_ice_password: ICEPassword,
//     fingerprint: Fingerprint,
//     video_mapping: RTPMap,
//     video_capability: FMTP,
//     remote_video_ssrc: MediaSSRC,
//     audio_mapping: RTPMap,
//     remote_audio_ssrc: MediaSSRC,
//     agent_type: AgentType,
// }
// #[derive(Debug, PartialEq)]
// pub enum StreamerOfferSDPParseError {
//     UnsupportedMediaCodecs,
//     DemuxRequired,
//     MissingVideoProfileSettings,
//     MissingRemoteSSRC,
//     UnsupportedMediaDirection,
// }
//
// #[derive(Debug)]
// enum AgentType {
//     Streamer,
//     Viewer,
// }
//
// struct SDPResolver {
//     fingerprint: Fingerprint,
// }
//
// pub fn get_random_string(size: usize) -> String {
//     thread_rng()
//         .sample_iter(Alphanumeric)
//         .take(size)
//         .map(char::from)
//         .collect()
// }
//
// impl SDPResolver {
//     pub fn new(fingerprint_hash: &str) -> Self {
//         let fingerprint = parse_fingerprint(fingerprint_hash)
//             .expect("Fingerprint should be in form of \"hash-function hash\"");
//         SDPResolver { fingerprint }
//     }
//     pub fn accept_streamer_sdp(
//         &self,
//         offer: SDPOffer,
//     ) -> Result<ResolvedSDP, StreamerOfferSDPParseError> {
//         let find_payload_number_by_media_codec =
//             |media_attributes: &Vec<Attribute>, media_codec: MediaCodec| {
//                 media_attributes
//                     .iter()
//                     .find_map(|attr| match attr {
//                         Attribute::RTPMap(rtpmap) => {
//                             if rtpmap.codec.eq(&media_codec) {
//                                 return Some(rtpmap);
//                             }
//                             return None;
//                         }
//                         _ => None,
//                     })
//                     .ok_or(StreamerOfferSDPParseError::UnsupportedMediaCodecs)
//             };
//
//         // Check if video and audio is demuxed
//         let is_video_demuxed = offer
//             .video_media_description
//             .iter()
//             .find(|attr| match attr {
//                 Attribute::RTCPMux => true,
//                 _ => false,
//             })
//             .is_some();
//
//         if !is_video_demuxed {
//             return Err(StreamerOfferSDPParseError::DemuxRequired);
//         }
//         let is_audio_demuxed = offer
//             .audio_media_description
//             .iter()
//             .find(|attr| match attr {
//                 Attribute::RTCPMux => true,
//                 _ => false,
//             })
//             .is_some();
//         if !is_audio_demuxed {
//             return Err(StreamerOfferSDPParseError::DemuxRequired);
//         }
//
//         // Check if media direction is set to sendonly
//         let is_video_sendonly = offer
//             .video_media_description
//             .iter()
//             .find(|attr| match attr {
//                 Attribute::SendOnly => true,
//                 _ => false,
//             })
//             .is_some();
//         if !is_video_sendonly {
//             return Err(StreamerOfferSDPParseError::UnsupportedMediaDirection);
//         }
//
//         let is_audio_sendonly = offer
//             .audio_media_description
//             .iter()
//             .find(|attr| match attr {
//                 Attribute::SendOnly => true,
//                 _ => false,
//             })
//             .is_some();
//         if !is_audio_sendonly {
//             return Err(StreamerOfferSDPParseError::UnsupportedMediaDirection);
//         }
//
//         let supported_video_rtpmap = find_payload_number_by_media_codec(
//             offer.video_media_description.as_ref(),
//             MediaCodec::Video(VideoCodec::H264),
//         )?
//         .clone();
//         let supported_audio_rtpmap = find_payload_number_by_media_codec(
//             offer.audio_media_description.as_ref(),
//             MediaCodec::Audio(AudioCodec::Opus),
//         )?
//         .clone();
//
//         let supported_video_fmtp = offer
//             .video_media_description
//             .iter()
//             .find_map(|attr| match attr {
//                 Attribute::FMTP(fmtp) => {
//                     if fmtp
//                         .payload_number
//                         .eq(&supported_audio_rtpmap.payload_number)
//                     {
//                         return Some(fmtp.clone());
//                     }
//                     return None;
//                 }
//                 _ => None,
//             })
//             .ok_or(StreamerOfferSDPParseError::MissingVideoProfileSettings)?;
//
//         let video_ssrc = offer
//             .video_media_description
//             .iter()
//             .find_map(|attr| match attr {
//                 Attribute::MediaSSRC(ssrc) => Some(ssrc.clone()),
//                 _ => None,
//             })
//             .ok_or(StreamerOfferSDPParseError::MissingRemoteSSRC)?;
//         let audio_ssrc = offer
//             .audio_media_description
//             .iter()
//             .find_map(|attr| match attr {
//                 Attribute::MediaSSRC(ssrc) => Some(ssrc.clone()),
//                 _ => None,
//             })
//             .ok_or(StreamerOfferSDPParseError::MissingRemoteSSRC)?;
//
//         Ok(ResolvedSDP {
//             agent_type: AgentType::Streamer,
//             fingerprint: self.fingerprint.clone(),
//             video_mapping: supported_video_rtpmap,
//             audio_mapping: supported_audio_rtpmap,
//             host_ice_password: get_random_string(20),
//             host_ice_username: get_random_string(4),
//             remote_ice_username: offer.ice_username,
//             remote_ice_password: offer.ice_password,
//             video_capability: supported_video_fmtp,
//             remote_video_ssrc: video_ssrc,
//             remote_audio_ssrc: audio_ssrc,
//         })
//     }
// }
//
// pub fn accept_streamer_sdp(offer: SDPOffer) -> Result<ResolvedSDP, StreamerOfferSDPParseError> {
//     let find_payload_number_by_media_codec =
//         |media_attributes: &Vec<Attribute>, media_codec: MediaCodec| {
//             media_attributes
//                 .iter()
//                 .find_map(|attr| match attr {
//                     Attribute::RTPMap(rtpmap) => {
//                         if rtpmap.codec.eq(&media_codec) {
//                             return Some(rtpmap.payload_number);
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
//     let supported_video_codec_payload_number = find_payload_number_by_media_codec(
//         offer.video_media_description.as_ref(),
//         MediaCodec::Video(VideoCodec::H264),
//     )?;
//     let supported_audio_codec_payload_number = find_payload_number_by_media_codec(
//         offer.audio_media_description.as_ref(),
//         MediaCodec::Audio(AudioCodec::Opus),
//     )?;
//
//     let supported_video_fmtp = offer
//         .video_media_description
//         .iter()
//         .find_map(|attr| match attr {
//             Attribute::FMTP(fmtp) => {
//                 if fmtp
//                     .payload_number
//                     .eq(&supported_video_codec_payload_number)
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
//         audio_codec: AudioCodec::Opus,
//         remote_ice_username: offer.ice_username,
//         remote_ice_password: offer.ice_password,
//         video_capability: supported_video_fmtp,
//         audio_payload_number: supported_audio_codec_payload_number,
//         video_payload_number: supported_video_codec_payload_number,
//         video_codec: VideoCodec::H264,
//         remote_video_ssrc: video_ssrc,
//         remote_audio_ssrc: audio_ssrc,
//     })
// }
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
