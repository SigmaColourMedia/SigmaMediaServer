use openh264::formats::YUVSource;
use openh264::nal_units;

use crate::access_unit_decoder::AccessUnitDecoder;
use crate::rtp::RTPPacket;

pub struct ThumbnailExtractor {
    last_picture: Option<ImageData>,
    au_decoder: AccessUnitDecoder,
    h264_decoder: openh264::decoder::Decoder,
}

impl ThumbnailExtractor {
    pub fn new() -> Self {
        ThumbnailExtractor {
            au_decoder: AccessUnitDecoder::new(),
            last_picture: None,
            h264_decoder: openh264::decoder::Decoder::new()
                .expect("OpenH264 decoder should initialize"),
        }
    }
    // Returns Some if new thumbnail image is available
    pub fn try_extract_thumbnail(&mut self, packet: &[u8]) -> Option<()> {
        let rtp_packet = RTPPacket::try_from(packet).ok()?;

        let access_unit = self.au_decoder.process_packet(rtp_packet)?;

        for nal in nal_units(&access_unit) {
            let yuv_data = self.h264_decoder.decode(nal).ok().flatten()?;
            let (width, height) = yuv_data.dimensions();
            let mut image_buffer = vec![0u8; width * height * 3]; // Setup buffer for image of size w*h*3
            yuv_data.write_rgb8(&mut image_buffer);

            self.last_picture = Some(ImageData {
                data_buffer: image_buffer,
                height: height as u16,
                width: width as u16,
            });
            return Some(());
        }
        None
    }
}

pub struct ImageData {
    pub data_buffer: Vec<u8>,
    pub width: u16,
    pub height: u16,
}
