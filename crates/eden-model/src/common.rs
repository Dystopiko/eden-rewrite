use ipnet::{IpNet, Ipv4Net, Ipv6Net};
use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::net::IpAddr;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "approval_status", rename_all = "lowercase")]
pub enum ApprovalStatus {
    #[default]
    Pending,
    Approved,
    Revoked,
}

/// This function normalizes an IP address into a CIDR representation suitable for
/// grouping and access control purposes (e.g. login/IP tracking or abuse detection).
///
/// # Normalization rules
/// - **IPv4 addresses** are normalized to a `/24` network.
/// - **IPv6 addresses** are normalized to a `/56` network (complies with [RFC 6177]).
///
/// [RFC 6177]: https://www.rfc-editor.org/rfc/rfc6177
#[must_use]
pub fn normalize_ip_into_trust_cidr(ip: IpAddr) -> IpNet {
    let cidr: IpNet = match ip {
        IpAddr::V4(v4) => Ipv4Net::new_assert(v4, 24).into(),

        // XX::/56 is used as outlined in RFC 6177 in Section 1 to 2:
        // https://www.rfc-editor.org/rfc/rfc6177
        IpAddr::V6(v6) => Ipv6Net::new_assert(v6, 56).into(),
    };
    cidr.trunc()
}
