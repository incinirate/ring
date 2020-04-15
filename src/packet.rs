use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct ICMPEchoPacket {
    pub message_type: u8,
    pub message_code: u8,
    pub checksum: u16,
    pub identifier: u16,
    pub sequence_num: u16,
}

#[derive(Serialize, Deserialize)]
pub struct IPv4Header {
    pub version_and_header_len: u8,
    pub type_of_service: u8,
    pub datagram_length: u16,
    pub ip_identifier: u16,
    pub flags_and_5frag_offset: u8, // flags are u3
    pub rest_of_frag_offset: u8,
    pub ttl: u8,
    pub protocol: u8,
    pub checksum: u16,
    pub source_ip: u32,
    pub destination_ip: u32,
}
