//! Platform-provided player identity (CrazyGames and Poki).
//! Game code reads this once at boot; portal SDKs populate via [`crate::store_portals`].

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformIdentity {
    pub provider: &'static str,
    pub display_name: String,
    pub external_id: Option<String>,
    pub avatar_url: Option<String>,
    pub auth_token: Option<String>,
}

impl PlatformIdentity {
    pub fn self_hosted(display_name: String) -> Self {
        Self {
            provider: "self",
            display_name,
            external_id: None,
            avatar_url: None,
            auth_token: None,
        }
    }
}
