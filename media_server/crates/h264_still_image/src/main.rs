use std::io::Read;

use byteorder::ByteOrder;

use crate::nal::get_nal_packet;
use crate::rtp_dump::get_rtp_packets;

mod nal;
mod rtp;
mod rtp_dump;

fn main() {
    let rtp_packets = get_rtp_packets();

    let mut ten = 0;
    for packet in rtp_packets {
        let a = get_nal_packet(packet.payload.as_slice()).unwrap();
        println!("received {}", a);
    }
}

// let mut out = fs::read("../../wireshark2.bin").unwrap();
//
// let config = DecoderConfig::new().debug(true);
// let mut decoder = Decoder::with_api_config(OpenH264API::from_source(), config).unwrap();
//
// let mut new_buff: Vec<u8> = vec![];
// new_buff.append(&mut vec![0u8, 0u8, 1u8]);
// new_buff.append(&mut Vec::from(&out[0..]));
//
// for a in nal_units(&new_buff) {
// println!("hehehe {:?}", a);
//
// let maybe_yuv = decoder.decode(&a).unwrap();
// println!("{:?}", maybe_yuv)
// }
