//! Bounded opaque credentials and non-recoverable access verifiers.
//!
//! Bearer values are created from the operating-system RNG and returned only
//! to the requesting connection. Durable state contains SHA-256 verifiers,
//! expiry, stable server-issued principal identity, and scope metadata. It
//! cannot reconstruct a bearer value or a private-table access code.

use std::collections::BTreeMap;
use std::fmt::{Debug, Formatter};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::authorized_table::GuestSessionId;
use crate::protocol::TableId;

pub const DEFAULT_CREDENTIAL_CAPACITY: usize = 1024;
pub const MAX_CREDENTIAL_CAPACITY: usize = 4096;
pub const TOKEN_BYTES: usize = 32;
const ACCESS_SALT_BYTES: usize = 16;
const ACCESS_DOMAIN: &[u8] = b"terminal-poker/private-access/v1\0";
const TOKEN_DOMAIN: &[u8] = b"terminal-poker/reconnect-token/v1\0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialRole {
    Guest,
    Reconnect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CredentialScope {
    pub table_id: TableId,
    pub role: CredentialRole,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BearerToken(String);

impl BearerToken {
    pub fn from_client(value: impl Into<String>) -> Result<Self, CredentialError> {
        let value = value.into();
        if value.len() != TOKEN_BYTES * 2 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(CredentialError::Malformed);
        }
        Ok(Self(value))
    }

    pub fn expose_to_wire(&self) -> &str {
        &self.0
    }
}

impl Debug for BearerToken {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("BearerToken(<redacted>)")
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconnectGrant {
    pub token: BearerToken,
    pub expires_at_unix_seconds: u64,
}

impl Debug for ReconnectGrant {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ReconnectGrant")
            .field("token", &"[REDACTED]")
            .field("expires_at_unix_seconds", &self.expires_at_unix_seconds)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct IssuedCredential {
    pub grant: ReconnectGrant,
    pub expires_at: SystemTime,
    pub scope: CredentialScope,
    pub principal: GuestSessionId,
}

impl IssuedCredential {
    pub fn expose_to_client(&self) -> &str {
        self.grant.token.expose_to_wire()
    }
}

impl Debug for IssuedCredential {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IssuedCredential")
            .field("token", &"[REDACTED]")
            .field("expires_at", &self.expires_at)
            .field("scope", &self.scope)
            .field("principal", &self.principal)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialError {
    InvalidCapacity,
    CapacityReached,
    Malformed,
    UnknownOrExpired,
    WrongScope,
    InvalidDurableRecord,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccessVerifier {
    algorithm: String,
    salt_hex: String,
    digest_hex: String,
}

impl Debug for AccessVerifier {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("AccessVerifier(REDACTED)")
    }
}

impl AccessVerifier {
    pub fn derive(access_code: &str) -> Self {
        let mut salt = [0u8; ACCESS_SALT_BYTES];
        OsRng.fill_bytes(&mut salt);
        Self {
            algorithm: "sha256-v1".to_string(),
            salt_hex: hex(&salt),
            digest_hex: hex(&access_digest(&salt, access_code.as_bytes())),
        }
    }

    pub fn derive_password(password: &str) -> Self {
        let mut salt = [0u8; ACCESS_SALT_BYTES];
        OsRng.fill_bytes(&mut salt);
        let mut digest = [0u8; 32];
        argon2::Argon2::default()
            .hash_password_into(password.as_bytes(), &salt, &mut digest)
            .expect("bounded password and fixed salt/output lengths");
        Self {
            algorithm: "argon2id-v1".into(),
            salt_hex: hex(&salt),
            digest_hex: hex(&digest),
        }
    }

    pub fn verify(&self, candidate: &str) -> bool {
        if self.algorithm == "argon2id-v1" {
            if candidate.len() > 96 {
                return false;
            }
            let Some(salt) = decode_hex(&self.salt_hex) else {
                return false;
            };
            if salt.len() != ACCESS_SALT_BYTES {
                return false;
            }
            let mut digest = [0u8; 32];
            if argon2::Argon2::default()
                .hash_password_into(candidate.as_bytes(), &salt, &mut digest)
                .is_err()
            {
                return false;
            }
            return constant_time_eq(self.digest_hex.as_bytes(), hex(&digest).as_bytes());
        }
        if self.algorithm != "sha256-v1" {
            return false;
        }
        let Some(salt) = decode_hex(&self.salt_hex) else {
            return false;
        };
        if salt.len() != ACCESS_SALT_BYTES {
            return false;
        }
        let candidate = hex(&access_digest(&salt, candidate.as_bytes()));
        constant_time_eq(self.digest_hex.as_bytes(), candidate.as_bytes())
    }

    pub fn is_valid(&self) -> bool {
        matches!(self.algorithm.as_str(), "sha256-v1" | "argon2id-v1")
            && decode_hex(&self.salt_hex).is_some_and(|value| value.len() == ACCESS_SALT_BYTES)
            && decode_hex(&self.digest_hex).is_some_and(|value| value.len() == 32)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DurableCredentialRecord {
    verifier_hex: String,
    expires_at_unix_seconds: u64,
    scope: CredentialScope,
    principal_id: String,
}

struct StoredCredential {
    expires_at: SystemTime,
    scope: CredentialScope,
    principal: GuestSessionId,
}

pub struct CredentialVault {
    capacity: usize,
    entries: BTreeMap<String, StoredCredential>,
    issued: u64,
    rejected: u64,
    expired: u64,
}

impl Debug for CredentialVault {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CredentialVault")
            .field("capacity", &self.capacity)
            .field("active", &self.entries.len())
            .field("issued", &self.issued)
            .field("rejected", &self.rejected)
            .field("expired", &self.expired)
            .finish()
    }
}

impl CredentialVault {
    pub fn new(capacity: usize) -> Result<Self, CredentialError> {
        if !(1..=MAX_CREDENTIAL_CAPACITY).contains(&capacity) {
            return Err(CredentialError::InvalidCapacity);
        }
        Ok(Self {
            capacity,
            entries: BTreeMap::new(),
            issued: 0,
            rejected: 0,
            expired: 0,
        })
    }

    pub fn issue(
        &mut self,
        principal: GuestSessionId,
        scope: CredentialScope,
        ttl: Duration,
    ) -> Result<IssuedCredential, CredentialError> {
        self.expire_at(SystemTime::now());
        if self.entries.len() >= self.capacity {
            return Err(CredentialError::CapacityReached);
        }
        let expires_at = SystemTime::now()
            .checked_add(ttl)
            .unwrap_or(SystemTime::UNIX_EPOCH);
        let (token, verifier) = loop {
            let mut bytes = [0u8; TOKEN_BYTES];
            OsRng.fill_bytes(&mut bytes);
            let token = BearerToken(hex(&bytes));
            let verifier = token_verifier(&token);
            if !self.entries.contains_key(&verifier) {
                break (token, verifier);
            }
        };
        self.entries.insert(
            verifier,
            StoredCredential {
                expires_at,
                scope,
                principal: principal.clone(),
            },
        );
        self.issued = self.issued.saturating_add(1);
        Ok(IssuedCredential {
            grant: ReconnectGrant {
                token,
                expires_at_unix_seconds: unix_seconds(expires_at),
            },
            expires_at,
            scope,
            principal,
        })
    }

    pub fn validate(
        &mut self,
        token: &BearerToken,
        expected: CredentialScope,
    ) -> Result<GuestSessionId, CredentialError> {
        self.expire_at(SystemTime::now());
        match self.entries.get(&token_verifier(token)) {
            Some(stored) if stored.scope == expected => Ok(stored.principal.clone()),
            Some(_) => {
                self.rejected = self.rejected.saturating_add(1);
                Err(CredentialError::WrongScope)
            }
            None => {
                self.rejected = self.rejected.saturating_add(1);
                Err(CredentialError::UnknownOrExpired)
            }
        }
    }

    pub fn authenticate_and_rotate(
        &mut self,
        token: &BearerToken,
        expected_role: CredentialRole,
        ttl: Duration,
    ) -> Result<IssuedCredential, CredentialError> {
        self.expire_at(SystemTime::now());
        let verifier = token_verifier(token);
        let Some(stored) = self.entries.get(&verifier) else {
            self.rejected = self.rejected.saturating_add(1);
            return Err(CredentialError::UnknownOrExpired);
        };
        if stored.scope.role != expected_role {
            self.rejected = self.rejected.saturating_add(1);
            return Err(CredentialError::WrongScope);
        }
        let principal = stored.principal.clone();
        let scope = stored.scope;
        self.entries.remove(&verifier);
        self.issue(principal, scope, ttl)
    }

    pub fn authenticate(
        &mut self,
        token: &BearerToken,
        expected_role: CredentialRole,
    ) -> Result<(GuestSessionId, CredentialScope), CredentialError> {
        self.expire_at(SystemTime::now());
        match self.entries.get(&token_verifier(token)) {
            Some(stored) if stored.scope.role == expected_role => {
                Ok((stored.principal.clone(), stored.scope))
            }
            Some(_) => {
                self.rejected = self.rejected.saturating_add(1);
                Err(CredentialError::WrongScope)
            }
            None => {
                self.rejected = self.rejected.saturating_add(1);
                Err(CredentialError::UnknownOrExpired)
            }
        }
    }

    pub fn revoke(&mut self, token: &BearerToken) -> bool {
        self.entries.remove(&token_verifier(token)).is_some()
    }

    pub fn revoke_principal(&mut self, principal: &GuestSessionId) -> usize {
        let before = self.entries.len();
        self.entries
            .retain(|_, entry| &entry.principal != principal);
        before - self.entries.len()
    }

    pub fn expire_at(&mut self, now: SystemTime) -> usize {
        let before = self.entries.len();
        self.entries.retain(|_, entry| entry.expires_at > now);
        let expired = before - self.entries.len();
        self.expired = self.expired.saturating_add(expired as u64);
        expired
    }

    pub fn active(&self) -> usize {
        self.entries.len()
    }

    pub fn durable_records(&self) -> Vec<DurableCredentialRecord> {
        self.entries
            .iter()
            .map(|(verifier_hex, stored)| DurableCredentialRecord {
                verifier_hex: verifier_hex.clone(),
                expires_at_unix_seconds: unix_seconds(stored.expires_at),
                scope: stored.scope,
                principal_id: stored.principal.stable_value().to_string(),
            })
            .collect()
    }

    pub fn restore(
        capacity: usize,
        records: Vec<DurableCredentialRecord>,
    ) -> Result<Self, CredentialError> {
        let mut vault = Self::new(capacity)?;
        if records.len() > capacity {
            return Err(CredentialError::InvalidDurableRecord);
        }
        let now = SystemTime::now();
        for record in records {
            if decode_hex(&record.verifier_hex).is_none_or(|value| value.len() != 32)
                || record.scope.table_id.0 == 0
            {
                return Err(CredentialError::InvalidDurableRecord);
            }
            let principal = GuestSessionId::new(record.principal_id)
                .map_err(|_| CredentialError::InvalidDurableRecord)?;
            let expires_at = UNIX_EPOCH
                .checked_add(Duration::from_secs(record.expires_at_unix_seconds))
                .ok_or(CredentialError::InvalidDurableRecord)?;
            if expires_at <= now {
                continue;
            }
            if vault
                .entries
                .insert(
                    record.verifier_hex,
                    StoredCredential {
                        expires_at,
                        scope: record.scope,
                        principal,
                    },
                )
                .is_some()
            {
                return Err(CredentialError::InvalidDurableRecord);
            }
        }
        Ok(vault)
    }
}

fn unix_seconds(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn token_verifier(token: &BearerToken) -> String {
    let mut hasher = Sha256::new();
    hasher.update(TOKEN_DOMAIN);
    hasher.update(token.0.as_bytes());
    hex(&hasher.finalize())
}

fn access_digest(salt: &[u8], code: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(ACCESS_DOMAIN);
    hasher.update(salt);
    hasher.update(code);
    hasher.finalize().into()
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(char::from(DIGITS[usize::from(byte >> 4)]));
        result.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    result
}

fn decode_hex(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return None;
    }
    value
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let high = (pair[0] as char).to_digit(16)?;
            let low = (pair[1] as char).to_digit(16)?;
            Some(((high << 4) | low) as u8)
        })
        .collect()
}

fn constant_time_eq(expected: &[u8], candidate: &[u8]) -> bool {
    let mut difference = expected.len() ^ candidate.len();
    let maximum = expected.len().max(candidate.len());
    for index in 0..maximum {
        difference |= usize::from(
            expected.get(index).copied().unwrap_or(0) ^ candidate.get(index).copied().unwrap_or(0),
        );
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope(table: u64, role: CredentialRole) -> CredentialScope {
        CredentialScope {
            table_id: TableId(table),
            role,
        }
    }

    #[test]
    fn random_tokens_are_scoped_expiring_rotatable_and_redacted() {
        let principal = GuestSessionId::random();
        let mut vault = CredentialVault::new(4).unwrap();
        let expected = scope(7, CredentialRole::Reconnect);
        let issued = vault
            .issue(principal.clone(), expected, Duration::from_secs(60))
            .unwrap();
        assert_eq!(issued.expose_to_client().len(), TOKEN_BYTES * 2);
        assert!(!format!("{issued:?}").contains(issued.expose_to_client()));
        assert!(!format!("{vault:?}").contains(issued.expose_to_client()));
        assert_eq!(
            vault.validate(&issued.grant.token, expected),
            Ok(principal.clone())
        );
        assert_eq!(
            vault.validate(&issued.grant.token, scope(8, CredentialRole::Reconnect)),
            Err(CredentialError::WrongScope)
        );
        let rotated = vault
            .authenticate_and_rotate(
                &issued.grant.token,
                CredentialRole::Reconnect,
                Duration::from_secs(60),
            )
            .unwrap();
        assert_ne!(rotated.expose_to_client(), issued.expose_to_client());
        assert_eq!(
            vault.validate(&issued.grant.token, expected),
            Err(CredentialError::UnknownOrExpired)
        );
        assert_eq!(
            vault.validate(&rotated.grant.token, expected),
            Ok(principal)
        );
    }

    #[test]
    fn durable_records_restore_verifiers_without_bearers() {
        let principal = GuestSessionId::random();
        let mut vault = CredentialVault::new(4).unwrap();
        let issued = vault
            .issue(
                principal.clone(),
                scope(3, CredentialRole::Reconnect),
                Duration::from_secs(60),
            )
            .unwrap();
        let records = vault.durable_records();
        let json = serde_json::to_string(&records).unwrap();
        assert!(!json.contains(issued.expose_to_client()));
        let mut restored = CredentialVault::restore(4, records).unwrap();
        assert_eq!(
            restored.validate(&issued.grant.token, scope(3, CredentialRole::Reconnect)),
            Ok(principal)
        );
    }

    #[test]
    fn access_verifier_is_salted_non_recoverable_and_fail_closed() {
        let code = "private-code-0123456789abcdef";
        let first = AccessVerifier::derive(code);
        let second = AccessVerifier::derive(code);
        assert_ne!(first, second);
        assert!(first.verify(code));
        assert!(!first.verify("private-code-wrong-0123456789"));
        let json = serde_json::to_string(&first).unwrap();
        assert!(!json.contains(code));
        assert!(first.is_valid());
    }

    #[test]
    fn capacity_expiry_and_revocation_are_failure_atomic() {
        let principal = GuestSessionId::random();
        let mut vault = CredentialVault::new(1).unwrap();
        let issued = vault
            .issue(
                principal.clone(),
                scope(1, CredentialRole::Guest),
                Duration::from_secs(60),
            )
            .unwrap();
        assert_eq!(
            vault.issue(
                principal.clone(),
                scope(1, CredentialRole::Guest),
                Duration::from_secs(60)
            ),
            Err(CredentialError::CapacityReached)
        );
        assert_eq!(vault.expire_at(issued.expires_at), 1);
        let replacement = vault
            .issue(
                principal,
                scope(1, CredentialRole::Guest),
                Duration::from_secs(60),
            )
            .unwrap();
        assert!(vault.revoke(&replacement.grant.token));
        assert_eq!(vault.active(), 0);
    }
}
