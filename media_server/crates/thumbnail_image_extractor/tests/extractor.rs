use openh264::formats::YUVSource;
use openh264::nal_units;

use thumbnail_image_extractor::{
    AccessUnitDecoder, get_rtp_packets, get_rtp_packets_raw, ThumbnailExtractor,
};

#[test]
fn finds_yuv_data() {
    let test_packets = get_rtp_packets();
    let mut au_decoder = AccessUnitDecoder::new();
    let mut decoder = openh264::decoder::Decoder::new().unwrap();

    let access_units = test_packets
        .into_iter()
        .map(|packet| au_decoder.process_packet(packet))
        .filter_map(|unit| match unit {
            None => None,
            Some(unit) => Some(unit),
        })
        .collect::<Vec<_>>();

    let mut oks = vec![];

    for unit in access_units {
        for nal in nal_units(&unit) {
            if let Ok(maybe_yuv) = decoder.decode(&nal) {
                if let Some(yuv) = maybe_yuv {
                    oks.push(yuv.dimensions())
                }
            }
        }
    }

    assert_eq!(oks.is_empty(), false);
}

#[test]
fn find_yuv_in_extract() {
    let test_packets = get_rtp_packets_raw();
    let mut extractor = ThumbnailExtractor::new();

    let mut oks = vec![];

    for packet in test_packets {
        if let Some(_) = extractor.try_extract_thumbnail(&packet) {
            oks.push(true)
        }
    }
    println!("{}", oks.len());

    assert_eq!(oks.is_empty(), false);
}
