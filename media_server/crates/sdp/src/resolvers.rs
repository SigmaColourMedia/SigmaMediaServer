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

pub fn parse_streamer_sdp_offer(
    offer: SDPOffer,
) -> Result<ResolvedSDP, StreamerOfferSDPParseError> {
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
