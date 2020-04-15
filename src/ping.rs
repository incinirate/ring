use std::io::{Result, Error, ErrorKind};
use std::net::{IpAddr, SocketAddr};
use std::time::{Instant, Duration};
use std::ops::Add;

use rand::random;

use socket2::{Socket, Domain, Protocol, SockAddr};
use dns_lookup::{lookup_addr};

use crate::{packet, util};

struct GenericIPHeader {
    datagram_length: u16,
    data_offset: u8,
    ttl: Option<u8>,
}

#[derive(PartialEq)]
pub enum ReplyType {
    Reply,
    TimeLimitExceeded,
}

pub struct PongResult {
    pub address: IpAddr,
    pub hostname: Option<String>,

    pub sequence: u16,
    pub ttl: Option<u8>,
    pub size: u16,
    pub rtt: Duration,
    pub mtype: ReplyType,
}

pub struct Pinger {
    address: IpAddr,
    socket: Socket,
    sock_addr: SockAddr,
    coder: bincode::Config,

    session: u16,  // Used as 'identifier' word to match echo requests/replies
    sequence: u16, // Used as 'sequence number' word to match echo requests/replies
}

const ECHO_REQUEST_V4: u8 = 8;
const ECHO_REQUEST_V6: u8 = 128;
const ECHO_REPLY_V4: u8 = 0;
const ECHO_REPLY_V6: u8 = 129;
const TIMEOUT_V4: u8 = 11;
const TIMEOUT_V6: u8 = 3;

impl Pinger {
    pub fn new(address: IpAddr) -> Result<Self> {
        // First obtain the raw socket
        let domain = if address.is_ipv6() { Domain::ipv6() } else { Domain::ipv4() };
        let protocol = if address.is_ipv6() { Protocol::icmpv6() } else { Protocol::icmpv4() };
        let stype = socket2::Type::raw().cloexec();
        let socket = Socket::new(domain, stype, Some(protocol))?;

        let sock_address = SocketAddr::from((address, 0));

        let mut coder = bincode::config();
        coder.big_endian(); // ICMP Packet Header uses big endian
        
        Ok(Pinger {
            address,
            socket, coder,
            sock_addr: SockAddr::from(sock_address),
            session: random::<u16>(), sequence: 0 
        })
    }

    // Sends out a ping, returns the icmp_seq (sequence num) used
    pub fn ping(&mut self) -> Result<u16> {
        self.sequence += 1; // Each new ping updates the sequence
        let pack = packet::ICMPEchoPacket {
            message_type: if self.address.is_ipv6() { ECHO_REQUEST_V6 } else { ECHO_REQUEST_V4 },
            message_code: 0,
            checksum: 0,
            identifier: self.session,
            sequence_num: self.sequence,
        };

        let mut payload = self.coder.serialize(&pack).unwrap();
        let payload = payload.as_mut_slice(); // Socket Interface expects a slice, not a vec
        util::set_checksum(payload, 1);

        self.socket.send_to(payload, &self.sock_addr).and(Ok(self.sequence))
    }

    pub fn receive_pong(&self, sequence_num: u16, timeout: Duration) -> Result<PongResult> {
        let begin_time = Instant::now();
        let end_time = begin_time.add(timeout);

        loop {
            let relative_timeout = end_time.duration_since(Instant::now());

            let mut buf = [0; 4096]; // We want the buffer to be fresh every time
            self.socket.set_read_timeout(Some(relative_timeout))?;
            let (_bytes, from) = self.socket.recv_from(&mut buf[..])?;

            let header = if self.address.is_ipv6() {
                // The socket doesn't put the header into our buffer
                // so unfortunately we cannot extract the ttl (or hop_limit as it's called in ipv6)

                GenericIPHeader {
                    datagram_length: 8,
                    data_offset: 0,
                    ttl: None
                }
            } else {
                let ip_packet = match self.coder.deserialize::<packet::IPv4Header>(&buf) {
                    Ok(p) => p,
                    Err(e) => {
                        return Err(Error::new(ErrorKind::InvalidData, e.to_string()));
                    }
                };

                // Get the 'header length' portion of the u8, which is encoded as u8/4 (bits/32)
                let data_offset = 4 * (ip_packet.version_and_header_len & 0x0F); 
            
                GenericIPHeader { 
                    datagram_length: ip_packet.datagram_length,
                    data_offset,
                    ttl: Some(ip_packet.ttl),
                }
            };

            // The IMCP portion will be located after the IP Header
            let icmp_packet = &buf[header.data_offset as usize..];
            let icmp_packet = match self.coder.deserialize::<packet::ICMPEchoPacket>(icmp_packet) {
                Ok(p) => p,
                Err(e) => {
                    return Err(Error::new(ErrorKind::InvalidData, e.to_string()));
                }
            };

            // Make sure that this is the right type of packet
            let mtype: ReplyType;
            if self.address.is_ipv6() {
                if icmp_packet.message_type == ECHO_REPLY_V6 { mtype = ReplyType::Reply }
                else if icmp_packet.message_type == TIMEOUT_V6 { mtype = ReplyType::TimeLimitExceeded }
                else { continue };
            } else {
                if icmp_packet.message_type == ECHO_REPLY_V4 { mtype = ReplyType::Reply }
                else if icmp_packet.message_type == TIMEOUT_V4 { mtype = ReplyType::TimeLimitExceeded }
                else { continue };
            }

            if mtype == ReplyType::Reply {
                // Check that this is the packet that we were looking for
                if icmp_packet.identifier != self.session { continue };
                if icmp_packet.sequence_num != sequence_num { continue };
            }

            // It was! Construct a Pong Result
            return Ok(PongResult {
                address: self.address,
                hostname: lookup_addr(&from.as_std().unwrap().ip()).ok(),
            
                sequence: icmp_packet.sequence_num,
                ttl: header.ttl,
                size: header.datagram_length - header.data_offset as u16,
                rtt: Instant::now().duration_since(begin_time),
                mtype,
            })
        }
    }

    pub fn set_ttl(&mut self, ttl: u32) -> Result<()> {
        self.socket.set_ttl(ttl)
    }
}
