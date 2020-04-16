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


#[allow(clippy::double_parens)] // For stylistic reasons
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
/// `skipword` will be skipped. Each word is treated as big endian.
fn sum_be_words(data: &[u8], skipword: usize) -> u32 {
    let skipword = std::cmp::min(skipword, data.len() / 2 - 1);
    data.chunks(2)
        .map(|word| match *word {
            [w] => w as u16,
            [wh, wl] => u16::from_be_bytes([wh, wl]),
            _ => unreachable!(),
        })
        .enumerate()
        .filter_map(|(i, w)| if i == skipword { None } else { Some(w as u32) })
        .fold(0, u32::wrapping_add)
}
