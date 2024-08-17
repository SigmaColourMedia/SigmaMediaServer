use std::io::Read;

use byteorder::ByteOrder;
use openh264::decoder::Decoder;

use crate::depacketizer::NALDecoder;
use crate::rtp_dump::get_rtp_packets;

mod depacketizer;
mod nal;
mod rtp;
mod rtp_dump;

fn main() {
    let rtp_packets = get_rtp_packets();

    let mut nal_decoder = NALDecoder::new();
    let mut decoder = Decoder::new().unwrap();

    for packet in rtp_packets {
        let packet_num = packet.sequence_number;
        if let Some(nal) = nal_decoder.decode_nal_unit(packet) {
            match decoder.decode(&nal) {
                Ok(item) => {
                    println!("decoded {}", packet_num);
                    if let Some(item) = item {
                        println!("got decoded item")
                    }
                }
                Err(crashed_decoder) => {
                    println!("crashed {} for {}", crashed_decoder, packet_num)
                }
            }
        }
    }
}
