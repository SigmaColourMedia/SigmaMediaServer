use std::io::Read;

use byteorder::ByteOrder;

use crate::rtp_dump::get_rtp_packets;

mod rtp;
mod rtp_dump;

fn main() {
    let rtp_packets = get_rtp_packets();

    for packet in rtp_packets {
        println!("packet {:?}", packet);
        break;
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
