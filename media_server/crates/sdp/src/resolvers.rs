use crate::line_parsers::{
    Attribute, AudioCodec, FMTP, MediaCodec, MediaSSRC, SDPOffer, VideoCodec,
};

#[derive(Debug)]
pub struct ResolvedSDP {
    remote_ice_username: String,
    remote_ice_password: String,
    video_codec: VideoCodec,
    video_payload_number: usize,
    audio_codec: AudioCodec,
    audio_payload_number: usize,
    remote_video_ssrc: MediaSSRC,
    remote_audio_ssrc: MediaSSRC,
    video_capability: FMTP,
}
#[derive(Debug)]
pub enum StreamerOfferSDPParseError {
    UnsupportedMediaCodecs,
    UnsupportedProfileSettings,
    MissingVideoProfileSettings,
    MissingRemoteSSRC,
    UnsupportedMediaDirection,
}

pub fn accept_streamer_sdp(offer: SDPOffer) -> Result<ResolvedSDP, StreamerOfferSDPParseError> {
    let find_payload_number_by_media_codec =
        |media_attributes: &Vec<Attribute>, media_codec: MediaCodec| {
            media_attributes
                .iter()
                .find_map(|attr| match attr {
                    Attribute::RTPMap(rtpmap) => {
                        if matches!(&rtpmap.codec, media_codec) {
                            return Some(rtpmap.payload_number);
                        }
                        return None;
                    }
                    _ => None,
                })
                .ok_or(StreamerOfferSDPParseError::UnsupportedMediaCodecs)
        };

    // Check if media direction is set to sendonly
    let is_video_sendonly = offer
        .video_media_description
        .iter()
        .find(|attr| match attr {
            Attribute::SendOnly => true,
            _ => false,
        })
        .is_some();
    if !is_video_sendonly {
        return Err(StreamerOfferSDPParseError::UnsupportedMediaDirection);
    }

    let is_audio_sendonly = offer
        .audio_media_description
        .iter()
        .find(|attr| match attr {
            Attribute::SendOnly => true,
            _ => false,
        })
        .is_some();
    if !is_audio_sendonly {
        return Err(StreamerOfferSDPParseError::UnsupportedMediaDirection);
    }

    let supported_video_codec_payload_number = find_payload_number_by_media_codec(
        offer.video_media_description.as_ref(),
        MediaCodec::Video(VideoCodec::H264),
    )?;
    let supported_audio_codec_payload_number = find_payload_number_by_media_codec(
        offer.audio_media_description.as_ref(),
        MediaCodec::Audio(AudioCodec::Opus),
    )?;

    let supported_video_fmtp = offer
        .video_media_description
        .iter()
        .find_map(|attr| match attr {
            Attribute::FMTP(fmtp) => {
                if fmtp
                    .payload_number
                    .eq(&supported_video_codec_payload_number)
                {
                    return Some(fmtp.clone());
                }
                return None;
            }
            _ => None,
        })
        .ok_or(StreamerOfferSDPParseError::MissingVideoProfileSettings)?;

    let video_ssrc = offer
        .video_media_description
        .iter()
        .find_map(|attr| match attr {
            Attribute::MediaSSRC(ssrc) => Some(ssrc.clone()),
            _ => None,
        })
        .ok_or(StreamerOfferSDPParseError::MissingRemoteSSRC)?;
    let audio_ssrc = offer
        .audio_media_description
        .iter()
        .find_map(|attr| match attr {
            Attribute::MediaSSRC(ssrc) => Some(ssrc.clone()),
            _ => None,
        })
        .ok_or(StreamerOfferSDPParseError::MissingRemoteSSRC)?;

    Ok(ResolvedSDP {
        audio_codec: AudioCodec::Opus,
        remote_ice_username: offer.ice_username,
        remote_ice_password: offer.ice_password,
        video_capability: supported_video_fmtp,
        audio_payload_number: supported_audio_codec_payload_number,
        video_payload_number: supported_video_codec_payload_number,
        video_codec: VideoCodec::H264,
        remote_video_ssrc: video_ssrc,
        remote_audio_ssrc: audio_ssrc,
    })
}

mod tests {
    mod accept_streamer_sdp {
        use crate::line_parsers::{
            Attribute, AudioCodec, FMTP, MediaCodec, MediaSSRC, RTPMap, SDPOffer, VideoCodec,
        };
        use crate::resolvers::accept_streamer_sdp;

        #[test]
        fn rejects_empty_media_attributes() {
            let offer: SDPOffer = SDPOffer {
                ice_username: "test".to_string(),
                ice_password: "test".to_string(),
                video_media_description: vec![],
                audio_media_description: vec![],
            };

            let result = accept_streamer_sdp(offer);

            assert!(result.is_err())
        }

        #[test]
        fn resolves_valid_offer() {
            let valid_video_media_attributes: Vec<Attribute> = vec![
                Attribute::SendOnly,
                Attribute::RTCPMux,
                Attribute::MediaSSRC(MediaSSRC {
                    ssrc: "video-ssrc".to_string(),
                }),
                Attribute::FMTP(FMTP {
                    payload_number: 96,
                    format_capability: vec!["fake-profile-level".to_string()],
                }),
                Attribute::RTPMap(RTPMap {
                    codec: MediaCodec::Video(VideoCodec::H264),
                    payload_number: 96,
                }),
            ];

            let valid_audio_media_attributes: Vec<Attribute> = vec![
                Attribute::SendOnly,
                Attribute::RTCPMux,
                Attribute::MediaSSRC(MediaSSRC {
                    ssrc: "audio-ssrc".to_string(),
                }),
                Attribute::RTPMap(RTPMap {
                    codec: MediaCodec::Audio(AudioCodec::Opus),
                    payload_number: 111,
                }),
            ];

            let offer_username = "username";
            let offer_password = "password";

            let offer = SDPOffer {
                ice_username: offer_username.to_string(),
                ice_password: offer_password.to_string(),
                audio_media_description: valid_audio_media_attributes,
                video_media_description: valid_video_media_attributes,
            };
            let result = accept_streamer_sdp(offer).expect("Should accept SDP offer");

            assert_eq!(
                result.video_codec,
                VideoCodec::H264,
                "Video codec should be H264"
            );

            assert_eq!(
                result.video_payload_number, 96,
                "Video payload number should be 96"
            );
            assert_eq!(
                result.video_capability,
                FMTP {
                    payload_number: 96,
                    format_capability: vec!["fake-profile-level".to_string()]
                },
                "Video FMTP should match offer FMTP with payload number 96"
            );
            assert_eq!(
                result.remote_video_ssrc,
                MediaSSRC {
                    ssrc: "video-ssrc".to_string()
                },
                "Video MediaSSRC should match the offer MediaSSRC"
            );

            assert_eq!(
                result.audio_codec,
                AudioCodec::Opus,
                "Audio codec should be Opus"
            );
            assert_eq!(
                result.audio_payload_number, 111,
                "Audio payload number should be 111"
            );
            assert_eq!(
                result.remote_audio_ssrc,
                MediaSSRC {
                    ssrc: "audio-ssrc".to_string()
                },
                "Audio MediaSSRC should match the offer MediaSSRC"
            );

            assert_eq!(
                result.remote_ice_username, offer_username,
                "Remote ICE username should match offer username"
            );
            assert_eq!(
                result.remote_ice_password, offer_password,
                "Remote ICE password should match offer password"
            );
        }
    }
}
