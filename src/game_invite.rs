//! Private game invitations. Debug output must never contain access material.
use std::net::SocketAddr;

use crate::protocol::TableId;

pub const DEFAULT_SERVER: &str = crate::game_stream::LAN_SERVER;

pub fn game_server_address(value: &str) -> Result<SocketAddr, &'static str> {
    let address: SocketAddr = value
        .parse()
        .map_err(|_| "Use a server IP address and port")?;
    let private = match address.ip() {
        std::net::IpAddr::V4(ip) => ip.is_private() || ip.is_loopback(),
        std::net::IpAddr::V6(ip) => ip.is_loopback() || (ip.segments()[0] & 0xfe00) == 0xfc00,
    };
    if !private || address.port() == 0 {
        return Err("Use a private network server and nonzero port");
    }
    Ok(address)
}

pub fn local_server_address(value: &str) -> Result<SocketAddr, &'static str> {
    let address: SocketAddr = value
        .parse()
        .map_err(|_| "Use a server IP address and port")?;
    if !address.ip().is_loopback() || address.port() == 0 {
        return Err("This build supports a local dedicated server only; use a loopback address and nonzero port");
    }
    Ok(address)
}

pub struct GameInvite {
    pub address: SocketAddr,
    pub table_id: TableId,
    pub access: String,
}

impl GameInvite {
    pub fn parse(value: &str) -> Result<Self, &'static str> {
        if value.len() > 160 {
            return Err("Invite is too long");
        }
        let value = value.trim();
        let fields: Vec<_> = value.split('|').collect();
        let (address, table, access) = if fields.len() == 4 && fields[0] == "SB2" {
            (local_server_address(fields[1])?, fields[2], fields[3])
        } else {
            let fields: Vec<_> = value.split(':').collect();
            if fields.len() != 4 || fields[0] != "SB1" {
                return Err("Use a complete SB2 or SB1 game invite");
            }
            let port = fields[1]
                .parse::<u16>()
                .map_err(|_| "Invalid invite port")?;
            let address = local_server_address(&format!("127.0.0.1:{port}"))?;
            (address, fields[2], fields[3])
        };
        let id = table.parse::<u64>().map_err(|_| "Invalid invite table")?;
        if id == 0
            || access.is_empty()
            || access.len() > 64
            || !access
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err("Invalid invite table or access code");
        }
        Ok(Self {
            address,
            table_id: TableId(id),
            access: access.to_string(),
        })
    }

    pub fn encode(&self) -> String {
        format!("SB2|{}|{}|{}", self.address, self.table_id.0, self.access)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invitations_preserve_endpoint_and_legacy_access() {
        for address in ["127.0.0.1:7777", "[::1]:7777"] {
            let invite = GameInvite {
                address: local_server_address(address).unwrap(),
                table_id: TableId(9),
                access: "private-code".into(),
            };
            let parsed = GameInvite::parse(&invite.encode()).unwrap();
            assert_eq!(parsed.address, invite.address);
            assert_eq!(parsed.table_id, invite.table_id);
            assert_eq!(parsed.access, invite.access);
        }
        assert_eq!(
            GameInvite::parse("SB1:7777:9:private-code")
                .unwrap()
                .address
                .port(),
            7777
        );
    }

    #[test]
    fn invalid_invites_fail_without_echoing_secrets_or_enabling_remote_access() {
        for value in [
            "SB2|192.168.1.5:7777|1|secret",
            "SB2|127.0.0.1:0|1|secret",
            "SB1:0:1:secret",
            "SB1:7777:0:secret",
            "SB1:7777:1:secret:extra",
            "SB2|127.0.0.1:7777|1|",
        ] {
            let error = GameInvite::parse(value)
                .err()
                .expect("must reject malformed invite");
            assert!(!error.contains("secret"));
        }
        assert!(GameInvite::parse("SB2|127.0.0.1:7777|1|sec\nret").is_err());
    }
}
