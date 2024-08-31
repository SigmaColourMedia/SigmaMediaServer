use std::fs::File;
use std::io::{BufReader, Read};

use byteorder::{ByteOrder, NetworkEndian};

use crate::rtp::RTPPacket;

/**
RTP-dump format:
- File starts with utf-8 encoded string `#!rtpplay1.0 address/port\n`
- Followed by RD Header

 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|             Start of recording GMT seconds                    |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|             Start of recording GMT microseconds               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|             Network source                                    |
+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+
|          port                 |            padding            |
+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+

- Then each RTP packet is followed by RD_T header
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|  RTP packet length + header   |         RTP packet length     |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                  Offset                                       |
+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+=+
 */

pub fn get_rtp_packets() -> Vec<RTPPacket> {
    let rtp_dump = File::open("../../wireshark-dump4.rtp").unwrap();
    let mut reader = BufReader::new(rtp_dump);
    let mut rtp_dump_header = vec![0u8; RTP_DUMP_HEADER_LEN + RD_HEADER_LEN]; // skip heading string + RT_D header
    reader.read_exact(&mut rtp_dump_header).unwrap();

    let mut rt_header_buffer = vec![0u8; 8];
    let mut rtp_buffer = vec![0u8; 3000];

    let mut rtp_packets = Vec::with_capacity(3000);

    while let Ok(_) = reader.read_exact(&mut rt_header_buffer) {
        let rt_header = get_rt_header(&rt_header_buffer);
        rtp_buffer.resize(rt_header.rtp_length as usize, 0);

        reader.read_exact(&mut rtp_buffer).unwrap();

        let buffer: &[u8] = &rtp_buffer;
        let rtp_packet = RTPPacket::try_from(buffer).unwrap();
        rtp_packets.push(rtp_packet)
    }

    rtp_packets
}

fn get_rt_header(buffer: &[u8]) -> RTHeader {
    let rtp_length = NetworkEndian::read_u16(&buffer[2..4]);
    RTHeader { rtp_length }
}

static RTP_DUMP_HEADER_LEN: usize = 28;
static RD_HEADER_LEN: usize = 16;
struct RTHeader {
    rtp_length: u16,
}
