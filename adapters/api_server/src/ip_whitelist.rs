use ipnet::IpNet;
use std::net::IpAddr;

use crate::ApiServerConfigurationError;

pub fn parse_ip_whitelist(whitelist_str: &str) -> Result<Vec<IpNet>, ApiServerConfigurationError> {
    let rules = whitelist_str
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    let mut nets = Vec::new();

    for rule in rules {
        if rule == "localhost" {
            nets.push("127.0.0.0/8".parse().unwrap());
            nets.push("::1/128".parse().unwrap());
        } else if let Ok(net) = rule.parse::<IpNet>() {
            nets.push(net);
        } else if let Ok(exact_ip) = rule.parse::<IpAddr>() {
            nets.push(IpNet::new(exact_ip, if exact_ip.is_ipv4() { 32 } else { 128 }).unwrap());
        } else if rule.contains('*') {
            if rule == "*" {
                nets.push("0.0.0.0/0".parse().unwrap());
                nets.push("::/0".parse().unwrap());
            } else if rule.ends_with(".*") {
                let prefix = rule.trim_end_matches(".*");
                let parts: Vec<&str> = prefix.split('.').collect();
                if parts.len() == 1 {
                    let ip_str = format!("{}.0.0.0", parts[0]);
                    if let Ok(ip) = ip_str.parse::<IpAddr>() {
                        nets.push(IpNet::new(ip, 8).unwrap());
                        continue;
                    }
                } else if parts.len() == 2 {
                    let ip_str = format!("{}.{}.0.0", parts[0], parts[1]);
                    if let Ok(ip) = ip_str.parse::<IpAddr>() {
                        nets.push(IpNet::new(ip, 16).unwrap());
                        continue;
                    }
                } else if parts.len() == 3 {
                    let ip_str = format!("{}.{}.{}.0", parts[0], parts[1], parts[2]);
                    if let Ok(ip) = ip_str.parse::<IpAddr>() {
                        nets.push(IpNet::new(ip, 24).unwrap());
                        continue;
                    }
                }
                return Err(ApiServerConfigurationError::InvalidIpWildcard {
                    rule: rule.to_string(),
                });
            } else {
                return Err(ApiServerConfigurationError::InvalidIpWildcard {
                    rule: rule.to_string(),
                });
            }
        } else {
            return Err(ApiServerConfigurationError::InvalidIpRule {
                rule: rule.to_string(),
            });
        }
    }

    if nets.is_empty() {
        nets.push("127.0.0.0/8".parse().unwrap());
        nets.push("::1/128".parse().unwrap());
    }

    Ok(nets)
}
