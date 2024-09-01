use std::io::Read;

use byteorder::ByteOrder;
use jpeg_encoder::{ColorType, Encoder};
use openh264::decoder::Decoder;
use openh264::formats::YUVSource;
use openh264::nal_units;

use crate::access_unit_decoder::AccessUnitDecoder;
use crate::rtp_dump::get_rtp_packets;

mod access_unit_decoder;
mod nal;
mod rtp;
mod rtp_dump;

fn main() {
    let rtp_packets = get_rtp_packets();
    let mut decoder = AccessUnitDecoder::new();

    let access_units = rtp_packets
        .into_iter()
        .map(|packet| decoder.process_packet(packet))
        .filter_map(|access_unit| access_unit)
        .collect::<Vec<_>>();
    let mut decoder = Decoder::new().unwrap();

    for access_unit in access_units {
        for nal in nal_units(&access_unit) {
            match decoder.decode(nal) {
                Ok(decoded) => {
                    if let Some(yuv) = decoded {
                        let dimensions = yuv.dimensions();
                        let encoder = Encoder::new_file("./some.jpeg", 50).unwrap();
                        let mut data = vec![0; dimensions.0 * dimensions.1 * 3];
                        yuv.write_rgb8(&mut data);
                        encoder
                            .encode(
                                &data,
                                dimensions.0 as u16,
                                dimensions.1 as u16,
                                ColorType::Rgb,
                            )
                            .unwrap();
                        panic!("got aaa hehe")
                    }
                }
                Err(err) => {}
            }
        }
    }
}
