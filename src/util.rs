use std::slice;
use std::io::{Result, Error, ErrorKind};
use std::net::{ToSocketAddrs, IpAddr};

pub fn resolve_dest(dest: &str) -> Result<IpAddr> {
    match format!("{}:0", dest).to_socket_addrs() {
        Ok(mut addrs) => {
            if let Some(addr) = addrs.next() {
                Ok(addr.ip())
            } else {
                Err(Error::new(ErrorKind::NotConnected, "empty iter"))
            }
        }

        Err(e) => Err(e)
    }
}


pub fn set_checksum(data: &mut [u8], location: usize) {
    let sum = get_checksum(data, location);
    data[location*2    ] = ((sum & 0xFF00) >> 8) as u8;
    data[location*2 + 1] = ((sum & 0x00FF)     ) as u8;
}

pub fn get_checksum(data: &[u8], location: usize) -> u16 {
    let mut sum = sum_be_words(data, location);
    while sum >> 16 != 0 {
        sum = (sum >> 16) + (sum & 0xFFFF);
    }

    !sum as u16 // The checksum field should be the ones complement of the sum
}

/// Sum all words (16 bit chunks) in the given data. The word at word offset
/// `skipword` will be skipped. Each word is treated as big endian. Must be
/// called with u16-aligned data.
/// From https://docs.rs/pnet_packet/0.25.0/src/pnet_packet/util.rs.html#149
fn sum_be_words(data: &[u8], mut skipword: usize) -> u32 {
    if data.len() == 0 { return 0 }
    debug_assert_eq!(0, data.as_ptr() as usize % 2, "Cannot sum mis-aligned words at {:p}", data.as_ptr());
    let len = data.len();
    let wdata: &[u16] = unsafe { slice::from_raw_parts(data.as_ptr() as *const u16, len / 2) };
    skipword = ::std::cmp::min(skipword, wdata.len());

    let mut sum = 0u32;
    let mut i = 0;
    while i < skipword {
        sum += u16::from_be(unsafe { *wdata.get_unchecked(i) }) as u32;
        i += 1;
    }
    i += 1;
    while i < wdata.len() {
        sum += u16::from_be(unsafe { *wdata.get_unchecked(i) }) as u32;
        i += 1;
    }
    // If the length is odd, make sure to checksum the final byte
    if len & 1 != 0 {
        sum += (unsafe { *data.get_unchecked(len - 1) } as u32) << 8;
    }

    sum
}
