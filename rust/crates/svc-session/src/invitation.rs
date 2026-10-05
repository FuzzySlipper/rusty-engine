//! Invitations: copyable text naming the host's endpoint, its relay and the
//! session secret. Anyone holding one can join until the session ends.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use iroh::{Endpoint, EndpointAddr, RelayMode, SecretKey};

const PREFIX: &str = "rusty-session-1.";

#[derive(Debug, Clone)]
pub struct Invitation {
    pub(crate) addr: EndpointAddr,
    pub(crate) secret: String,
    pub(crate) application: String,
    pub(crate) relay_token: String,
}

/// A fresh session secret: 128 random bits as hex.
pub(crate) fn secret() -> String {
    SecretKey::generate().to_bytes()[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

impl Invitation {
    /// With a relay the invitation names only the host's key and relay, so
    /// it carries no address of the host's network; the guest reaches the
    /// host through the relay and a direct path is found from there when one
    /// exists. Without a relay it carries the host's direct addresses.
    pub(crate) fn new(
        endpoint: &Endpoint,
        relayed: bool,
        relay_token: &str,
        secret: &str,
        application: &str,
    ) -> Self {
        let full = endpoint.addr();
        let addr = if relayed {
            EndpointAddr::new(full.id)
                .with_addrs(full.relay_urls().cloned().map(iroh::TransportAddr::Relay))
        } else {
            full
        };
        Self {
            addr,
            secret: secret.to_owned(),
            application: application.to_owned(),
            relay_token: relay_token.to_owned(),
        }
    }

    pub fn encode(&self) -> String {
        let value = serde_json::json!({
            "addr": self.addr,
            "secret": self.secret,
            "application": self.application,
            "relayToken": self.relay_token,
        });
        format!("{PREFIX}{}", URL_SAFE_NO_PAD.encode(value.to_string()))
    }

    /// Reads an invitation, tolerating the whitespace a chat client adds.
    pub fn decode(text: &str) -> Result<Self, String> {
        let invalid = || "this is not a session invitation".to_owned();
        let body = text.trim().strip_prefix(PREFIX).ok_or_else(invalid)?;
        let json = URL_SAFE_NO_PAD.decode(body).map_err(|_| invalid())?;
        let value: serde_json::Value = serde_json::from_slice(&json).map_err(|_| invalid())?;
        Ok(Self {
            addr: serde_json::from_value(value["addr"].clone()).map_err(|_| invalid())?,
            secret: value["secret"].as_str().ok_or_else(invalid)?.to_owned(),
            application: value["application"]
                .as_str()
                .ok_or_else(invalid)?
                .to_owned(),
            relay_token: value["relayToken"].as_str().unwrap_or_default().to_owned(),
        })
    }

    pub fn application(&self) -> &str {
        &self.application
    }

    pub fn host_key(&self) -> String {
        self.addr.id.to_string()
    }

    pub(crate) fn relay_mode(&self) -> RelayMode {
        crate::relay_mode(self.addr.relay_urls().cloned(), &self.relay_token)
    }
}
