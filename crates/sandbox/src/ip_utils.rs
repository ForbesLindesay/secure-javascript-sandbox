// To avoid using unstable #![feature(ip)] the implementation is copied here.

pub(crate) trait IpUtils {
    fn is_global_ext(&self) -> bool;
}

impl IpUtils for std::net::IpAddr {
    fn is_global_ext(&self) -> bool {
        match self {
            std::net::IpAddr::V4(ipv4) => ipv4.is_global_ext(),
            std::net::IpAddr::V6(ipv6) => ipv6.is_global_ext(),
        }
    }
}

trait IpUtilsV4Helper {
    fn is_shared_ext(&self) -> bool;
    fn is_benchmarking_ext(&self) -> bool;
    fn is_reserved_ext(&self) -> bool;
}
impl IpUtilsV4Helper for std::net::Ipv4Addr {
    fn is_shared_ext(&self) -> bool {
        self.octets()[0] == 100 && (self.octets()[1] & 0b1100_0000 == 0b0100_0000)
    }
    fn is_benchmarking_ext(&self) -> bool {
        self.octets()[0] == 198 && (self.octets()[1] & 0xfe) == 18
    }
    fn is_reserved_ext(&self) -> bool {
        self.octets()[0] & 0xF0 == 0xF0 && !self.is_broadcast()
    }
}
impl IpUtils for std::net::Ipv4Addr {
    fn is_global_ext(&self) -> bool {
        !(self.octets()[0] == 0 // "This network"
            || self.is_private()
            || self.is_shared_ext()
            || self.is_loopback()
            || self.is_link_local()
            // addresses reserved for future protocols (`192.0.0.0/24`)
            // .9 and .10 are documented as globally reachable so they're excluded
            || (
                self.octets()[0] == 192 && self.octets()[1] == 0 && self.octets()[2] == 0
                && self.octets()[3] != 9 && self.octets()[3] != 10
            )
            || self.is_documentation()
            || self.is_benchmarking_ext()
            || self.is_reserved_ext()
            || self.is_broadcast())
    }
}

trait IpUtilsV6Helper {
    fn is_documentation_ext(&self) -> bool;
}
impl IpUtilsV6Helper for std::net::Ipv6Addr {
    fn is_documentation_ext(&self) -> bool {
        matches!(
            self.segments(),
            [0x2001, 0xdb8, ..] | [0x3fff, 0..=0x0fff, ..]
        )
    }
}
impl IpUtils for std::net::Ipv6Addr {
    fn is_global_ext(&self) -> bool {
        !(self.is_unspecified()
            || self.is_loopback()
            // IPv4-mapped Address (`::ffff:0:0/96`)
            || matches!(self.segments(), [0, 0, 0, 0, 0, 0xffff, _, _])
            // IPv4-IPv6 Translat. (`64:ff9b:1::/48`)
            || matches!(self.segments(), [0x64, 0xff9b, 1, _, _, _, _, _])
            // Discard-Only Address Block (`100::/64`)
            || matches!(self.segments(), [0x100, 0, 0, 0, _, _, _, _])
            // IETF Protocol Assignments (`2001::/23`)
            || (matches!(self.segments(), [0x2001, b, _, _, _, _, _, _] if b < 0x200)
                && !(
                    // Port Control Protocol Anycast (`2001:1::1`)
                    u128::from_be_bytes(self.octets()) == 0x2001_0001_0000_0000_0000_0000_0000_0001
                    // Traversal Using Relays around NAT Anycast (`2001:1::2`)
                    || u128::from_be_bytes(self.octets()) == 0x2001_0001_0000_0000_0000_0000_0000_0002
                    // AMT (`2001:3::/32`)
                    || matches!(self.segments(), [0x2001, 3, _, _, _, _, _, _])
                    // AS112-v6 (`2001:4:112::/48`)
                    || matches!(self.segments(), [0x2001, 4, 0x112, _, _, _, _, _])
                    // ORCHIDv2 (`2001:20::/28`)
                    // Drone Remote ID Protocol Entity Tags (DETs) Prefix (`2001:30::/28`)`
                    || matches!(self.segments(), [0x2001, b, _, _, _, _, _, _] if (0x20..=0x3F).contains(&b))
                ))
            // 6to4 (`2002::/16`) – it's not explicitly documented as globally reachable,
            // IANA says N/A.
            || matches!(self.segments(), [0x2002, _, _, _, _, _, _, _])
            || self.is_documentation_ext()
            // Segment Routing (SRv6) SIDs (`5f00::/16`)
            || matches!(self.segments(), [0x5f00, ..])
            || self.is_unique_local()
            || self.is_unicast_link_local())
    }
}
