use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Evaluates whether a host string points to a local or LAN address.
///
/// Supports:
/// - Loopback addresses (`127.0.0.0/8`, `::1`, `localhost`, `*.localhost`, `0.0.0.0`)
/// - Private IPv4 addresses (RFC 1918: `10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`)
/// - Link-local IPv4 addresses (RFC 3927: `169.254.0.0/16`)
/// - Shared address space / CGNAT (RFC 6598: `100.64.0.0/10`, e.g. Tailscale)
/// - Private / Link-local IPv6 addresses (RFC 4193 unique local `fc00::/7`, RFC 4291 link-local `fe80::/10`, IPv4-mapped)
/// - Local and LAN domain names (`.local`, `.localdomain`, `.lan`, `.home.arpa`, `.internal`, or single-label hostnames)
pub fn is_local_or_lan_host_str(host: &str) -> bool {
    let host = host.trim_matches(['[', ']']).trim();
    if host.is_empty() {
        return false;
    }

    let host_without_zone = host.split('%').next().unwrap_or(host);

    if let Ok(ip) = host_without_zone.parse::<IpAddr>() {
        return is_local_or_lan_ip(ip);
    }

    is_local_or_lan_hostname(host)
}

pub fn is_local_or_lan_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => is_local_or_lan_ipv4(ipv4),
        IpAddr::V6(ipv6) => is_local_or_lan_ipv6(ipv6),
    }
}

pub fn is_local_or_lan_ipv4(ip: Ipv4Addr) -> bool {
    if ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
    {
        return true;
    }
    let octets = ip.octets();
    octets[0] == 100 && (64..=127).contains(&octets[1])
}

pub fn is_local_or_lan_ipv6(ip: Ipv6Addr) -> bool {
    if ip.is_loopback() || ip.is_unspecified() {
        return true;
    }
    if ip.to_ipv4().is_some_and(is_local_or_lan_ipv4) {
        return true;
    }
    let octets = ip.octets();
    if (octets[0] & 0xfe) == 0xfc {
        return true;
    }
    if octets[0] == 0xfe && (octets[1] & 0xc0) == 0x80 {
        return true;
    }
    false
}

pub fn is_local_or_lan_hostname(host: &str) -> bool {
    let host = host.strip_suffix('.').unwrap_or(host);
    let host_lower = host.to_ascii_lowercase();

    if host_lower.is_empty() || host_lower.starts_with('.') {
        return false;
    }

    if host_lower == "localhost"
        || host_lower == "localhost.localdomain"
        || host_lower.ends_with(".localhost")
        || host_lower.ends_with(".local")
        || host_lower.ends_with(".localdomain")
        || host_lower.ends_with(".lan")
        || host_lower.ends_with(".home.arpa")
        || host_lower.ends_with(".internal")
        || !host_lower.contains('.')
    {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_loopback_and_unspecified_hosts() {
        for host in [
            "localhost",
            "localhost.localdomain",
            "service.localhost",
            "127.0.0.1",
            "127.1.2.3",
            "0.0.0.0",
            "::1",
            "[::1]",
            "::",
        ] {
            assert!(
                is_local_or_lan_host_str(host),
                "{host} should be recognized as local/LAN"
            );
        }
    }

    #[test]
    fn detects_private_and_link_local_ipv4() {
        for host in [
            "10.0.0.1",
            "10.255.255.254",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.1.1",
            "192.168.254.10",
            "169.254.1.1",
            "100.64.0.1",
            "100.127.255.254",
        ] {
            assert!(
                is_local_or_lan_host_str(host),
                "{host} should be recognized as local/LAN"
            );
        }
    }

    #[test]
    fn detects_private_and_link_local_ipv6() {
        for host in [
            "fc00::1",
            "[fc00::1]",
            "fd12:3456:789a::1",
            "[fd12:3456:789a::1]",
            "fe80::1",
            "[fe80::1]",
            "::ffff:192.168.1.5",
            "[::ffff:192.168.1.5]",
            "fe80::1%eth0",
            "[fe80::1%eth0]",
        ] {
            assert!(
                is_local_or_lan_host_str(host),
                "{host} should be recognized as local/LAN"
            );
        }
    }

    #[test]
    fn detects_lan_hostnames() {
        for host in [
            "nas",
            "my-pc",
            "nas.local",
            "server.lan",
            "home.home.arpa",
            "nas.localdomain",
            "docker.internal",
            "nas.local.",
        ] {
            assert!(
                is_local_or_lan_host_str(host),
                "{host} should be recognized as local/LAN"
            );
        }
    }

    #[test]
    fn rejects_public_remote_hosts() {
        for host in [
            "example.com",
            "api.openai.com",
            "dav.jianguoyun.com",
            "8.8.8.8",
            "1.1.1.1",
            "100.63.255.255",
            "100.128.0.1",
            "172.15.255.255",
            "172.32.0.1",
            "2001:db8::1",
            "[2001:db8::1]",
            "",
            "   ",
            ".",
            ".local",
        ] {
            assert!(
                !is_local_or_lan_host_str(host),
                "{host} should NOT be recognized as local/LAN"
            );
        }
    }
}
