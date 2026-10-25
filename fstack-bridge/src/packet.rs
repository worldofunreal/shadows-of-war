//! Small, allocation-free L2/L3/L4 helpers used by the RX dispatcher.

/// Return the worker queue selected by a TCP destination port.
///
/// Dynamic relay ports are deliberately the routing key: `port % nb_queues`
/// is the worker that owns the lobby. Packets that cannot be parsed, are not
/// TCP, are fragmented, or target a privileged port stay on their RSS queue.
pub fn tcp_destination_queue(data: &[u8], nb_queues: u16, min_port: u16) -> Option<u16> {
    if nb_queues == 0 {
        return None;
    }
    let port = tcp_destination_port(data)?;
    (port >= min_port).then_some(port % nb_queues)
}

/// Read the destination used by listener ownership, including VLAN and IPv6.
/// Fragmented packets remain with the stack that can reassemble them.
pub fn tcp_destination_port(data: &[u8]) -> Option<u16> {
    if data.len() < 14 {
        return None;
    }

    let mut l3 = 14usize;
    let mut ether_type = u16::from_be_bytes([data[12], data[13]]);
    while matches!(ether_type, 0x8100 | 0x88a8 | 0x9100) {
        if data.len() < l3 + 4 {
            return None;
        }
        ether_type = u16::from_be_bytes([data[l3 + 2], data[l3 + 3]]);
        l3 += 4;
    }
    let l4 = match ether_type {
        0x0800 => {
            if data.len() < l3 + 20 || data[l3] >> 4 != 4 { return None; }
            let ihl = ((data[l3] & 0x0f) as usize) * 4;
            let fragment = u16::from_be_bytes([data[l3 + 6], data[l3 + 7]]);
            if ihl < 20 || fragment & 0x3fff != 0 || data[l3 + 9] != 6 { return None; }
            l3 + ihl
        }
        0x86dd => {
            if data.len() < l3 + 40 || data[l3] >> 4 != 6 { return None; }
            let mut next = data[l3 + 6];
            let mut offset = l3 + 40;
            for _ in 0..8 {
                if next == 6 { break; }
                if data.len() < offset + 2 { return None; }
                let size = match next {
                    0 | 43 | 60 => (data[offset + 1] as usize + 1) * 8,
                    51 => (data[offset + 1] as usize + 2) * 4,
                    _ => return None,
                };
                next = data[offset];
                offset += size;
            }
            if next != 6 { return None; }
            offset
        }
        _ => return None,
    };
    if data.len() < l4 + 4 { return None; }
    Some(u16::from_be_bytes([data[l4 + 2], data[l4 + 3]]))
}

#[cfg(test)]
mod tests {
    use super::tcp_destination_queue;

    fn packet(dst_port: u16, vlan: bool) -> Vec<u8> {
        let l3 = if vlan { 18 } else { 14 };
        let mut bytes = vec![0u8; l3 + 20 + 20];
        bytes[12..14].copy_from_slice(&(if vlan { 0x8100u16 } else { 0x0800u16 }).to_be_bytes());
        if vlan {
            bytes[16..18].copy_from_slice(&0x0800u16.to_be_bytes());
        }
        bytes[l3] = 0x45;
        bytes[l3 + 9] = 6;
        bytes[l3 + 20 + 2..l3 + 20 + 4].copy_from_slice(&dst_port.to_be_bytes());
        bytes
    }

    #[test]
    fn routes_ipv6_extensions_and_rejects_truncated_or_fragmented_headers() {
        let mut bytes = vec![0u8; 14 + 40 + 8 + 20];
        bytes[12..14].copy_from_slice(&0x86ddu16.to_be_bytes());
        bytes[14] = 0x60;
        bytes[20] = 60; // destination options, followed by TCP
        bytes[54] = 6;
        bytes[64..66].copy_from_slice(&4097u16.to_be_bytes());
        assert_eq!(tcp_destination_queue(&bytes, 3, 1024), Some(2));
        assert_eq!(tcp_destination_queue(&bytes[..65], 3, 1024), None);
        bytes[20] = 44;
        assert_eq!(tcp_destination_queue(&bytes, 3, 1024), None);
    }

    #[test]
    fn routes_dynamic_port_by_modulo() {
        assert_eq!(
            tcp_destination_queue(&packet(1024, false), 4, 1024),
            Some(0)
        );
        assert_eq!(
            tcp_destination_queue(&packet(65535, false), 4, 1024),
            Some(3)
        );
        assert_eq!(tcp_destination_queue(&packet(1023, false), 4, 1024), None);
    }

    #[test]
    fn handles_vlan_and_leaves_non_tcp_traffic_local() {
        assert_eq!(tcp_destination_queue(&packet(4098, true), 4, 1024), Some(2));
        let mut udp = packet(4098, false);
        udp[23] = 17;
        assert_eq!(tcp_destination_queue(&udp, 4, 1024), None);
        let mut fragmented = packet(4098, false);
        fragmented[20..22].copy_from_slice(&0x2000u16.to_be_bytes());
        assert_eq!(tcp_destination_queue(&fragmented, 4, 1024), None);
    }
}
