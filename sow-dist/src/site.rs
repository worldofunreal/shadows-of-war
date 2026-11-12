//! Deployment site identity. `./sow p` (production) and `./sow ptr` (PTR on the
//! same IONOS host) run one pipeline in prod.rs and prod_relay.rs. This module
//! is the only owner of the values that differ between them: public host,
//! loopback address, jails, and host-side paths. Production values are the
//! original literals, so production output does not change.
//!
//! Jails share the host network stack, so PTR listens on its own loopback
//! address with the same ports. Paths inside a jail do not change: PTR mounts
//! its own release tree at the same `/srv/sow` inside its jail.

use anyhow::{Result, bail};
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Site {
    Production,
    Ptr,
}

static SELECTED: OnceLock<Site> = OnceLock::new();

/// Select the site once, before any command runs.
pub(crate) fn select(site: Site) -> Result<()> {
    match SELECTED.set(site) {
        Ok(()) => Ok(()),
        Err(_) if current() == site => Ok(()),
        Err(_) => bail!("deployment site was already selected"),
    }
}

/// The selected site. Commands that never select one (tests, local preview) run as production.
pub(crate) fn current() -> Site {
    SELECTED.get().copied().unwrap_or(Site::Production)
}

impl Site {
    /// Public host served by nginx and named by the TLS certificate.
    pub(crate) fn host(self) -> &'static str {
        match self {
            Self::Production => "shadowsofwar.io",
            Self::Ptr => "ptr.shadowsofwar.io",
        }
    }

    pub(crate) fn origin(self) -> String {
        format!("https://{}", self.host())
    }

    /// Name in deploy messages, so a PTR run never reports "Production".
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Production => "Production",
            Self::Ptr => "PTR",
        }
    }

    /// Loopback address for the jails, relay management, and nginx upstreams.
    pub(crate) fn loopback(self) -> &'static str {
        match self {
            Self::Production => "127.0.0.1",
            Self::Ptr => "127.0.0.2",
        }
    }

    /// The server's HTTP listener: maps, admin, and `/internal/match/progress`, which the database calls
    /// to verify turn streams. The database falls back to production's address when this is unset.
    pub(crate) fn server_http_url(self) -> String {
        format!("http://{}:25566", self.loopback())
    }

    pub(crate) fn server_jail(self) -> &'static str {
        match self {
            Self::Production => "sow-server",
            Self::Ptr => "ptr-sow-server",
        }
    }

    pub(crate) fn database_jail(self) -> &'static str {
        match self {
            Self::Production => "sow-database",
            Self::Ptr => "ptr-sow-database",
        }
    }

    /// Host directory that holds `current` and `releases`, mounted read-only into the jails as `/srv/sow`.
    pub(crate) fn release_root(self) -> &'static str {
        match self {
            Self::Production => "/srv/sow",
            Self::Ptr => "/srv/ptr",
        }
    }

    /// Host copy of the service environment files.
    pub(crate) fn host_etc(self) -> &'static str {
        match self {
            Self::Production => "/usr/local/etc/sow",
            Self::Ptr => "/usr/local/etc/sow-ptr",
        }
    }

    /// Host directory mounted into the jails as `/var/log/sow`.
    pub(crate) fn log_dir(self) -> &'static str {
        match self {
            Self::Production => "/var/log/sow",
            Self::Ptr => "/var/log/sow-ptr",
        }
    }

    /// Staging directory on the control host.
    pub(crate) fn remote_stage(self) -> &'static str {
        match self {
            Self::Production => "/home/bizkit/.sow-deploy",
            Self::Ptr => "/home/bizkit/.sow-deploy-ptr",
        }
    }

    /// Ignored environment file for this site's own secrets. Loaded after sow-dist/.env.
    pub(crate) fn env_file(self) -> &'static str {
        match self {
            Self::Production => "sow-dist/.env",
            Self::Ptr => "sow-dist/.env.ptr",
        }
    }

    /// nginx site file. nginx reads conf.d in name order and its first server
    /// block answers unknown hosts, so PTR's file must sort after production's.
    pub(crate) fn nginx_conf(self) -> &'static str {
        match self {
            Self::Production => "shadowsofwar.io.conf",
            Self::Ptr => "shadowsofwar.ptr.conf",
        }
    }

    pub(crate) fn is_production(self) -> bool {
        self == Self::Production
    }

    /// The other site on this host.
    pub(crate) fn peer(self) -> Self {
        match self {
            Self::Production => Self::Ptr,
            Self::Ptr => Self::Production,
        }
    }

    /// Jail root on the host. Inside a jail the same files appear under /.
    pub(crate) fn server_root(self) -> String {
        format!("/zroot/jails/{}", self.server_jail())
    }

    pub(crate) fn database_root(self) -> String {
        format!("/zroot/jails/{}", self.database_jail())
    }

    /// Names the TLS certificate must cover.
    fn tls_names(self) -> &'static str {
        match self {
            Self::Production => "shadowsofwar.io ws.shadowsofwar.io",
            Self::Ptr => "ptr.shadowsofwar.io",
        }
    }

    /// Port 80 names. Production also answers on www and the raw IP.
    fn http_names(self) -> &'static str {
        match self {
            Self::Production => "shadowsofwar.io www.shadowsofwar.io ws.shadowsofwar.io 74.208.246.177",
            Self::Ptr => "ptr.shadowsofwar.io",
        }
    }

    /// Port 443 game names.
    fn tls_server_names(self) -> &'static str {
        match self {
            Self::Production => "shadowsofwar.io ws.shadowsofwar.io",
            Self::Ptr => "ptr.shadowsofwar.io",
        }
    }

    /// The identity import (player accounts from production) is never sent to PTR.
    pub(crate) fn owns_identity_import(self) -> bool {
        self == Self::Production
    }

    /// Portal packages (CrazyGames, Poki, Jest) are owner upload artifacts in
    /// dist/. Only production writes them, so PTR cannot put its origin into them.
    pub(crate) fn builds_portal_packages(self) -> bool {
        self == Self::Production
    }

    /// Prefix of this site's build cache directories under dist/.sow-state.
    pub(crate) fn state_prefix(self) -> &'static str {
        match self {
            Self::Production => "prod",
            Self::Ptr => "ptr",
        }
    }

    /// Reward audit accounts are rows in this site's own database. Each site keeps its own file, so
    /// PTR never reads production accounts, which PTR's database does not contain.
    pub(crate) fn reward_audit_accounts_file(self) -> &'static str {
        match self {
            Self::Production => "reward-audit-accounts.json",
            Self::Ptr => "ptr-reward-audit-accounts.json",
        }
    }

    /// Prefix for this site's http-level nginx map variables. nginx shares one http
    /// context, so each site's maps carry its own prefix and never collide.
    pub(crate) fn map_prefix(self) -> &'static str {
        match self {
            Self::Production => "",
            Self::Ptr => "ptr_",
        }
    }

    /// Every `__SITE_*__` placeholder with its value for this site.
    fn tokens(self) -> Vec<(&'static str, String)> {
        vec![
            ("__SITE_HOST__", self.host().to_string()),
            ("__SITE_LOOPBACK__", self.loopback().to_string()),
            ("__SITE_SERVER_JAIL__", self.server_jail().to_string()),
            ("__SITE_DATABASE_JAIL__", self.database_jail().to_string()),
            ("__SITE_SERVER_ROOT__", self.server_root()),
            ("__SITE_DATABASE_ROOT__", self.database_root()),
            ("__SITE_RELEASE_ROOT__", self.release_root().to_string()),
            ("__SITE_RELEASE_REL__", self.release_root().trim_start_matches('/').to_string()),
            ("__SITE_HOST_ETC__", self.host_etc().to_string()),
            ("__SITE_HOST_ETC_REL__", self.host_etc().trim_start_matches('/').to_string()),
            ("__SITE_NGINX_CONF__", self.nginx_conf().to_string()),
            ("__SITE_LOG_DIR__", self.log_dir().to_string()),
            ("__SITE_TLS_NAMES__", self.tls_names().to_string()),
            ("__SITE_HTTP_NAMES__", self.http_names().to_string()),
            ("__SITE_TLS_SERVER_NAMES__", self.tls_server_names().to_string()),
            ("__SITE_MAP_PREFIX__", self.map_prefix().to_string()),
        ]
    }
}

/// Replace every `__SITE_*__` placeholder in `text` with the selected site's value.
/// A placeholder this module does not know is an error, never a silent pass-through.
pub(crate) fn render(text: &str) -> Result<String> {
    if !text.contains("__SITE_") {
        return Ok(text.to_string());
    }
    let mut rendered = text.to_string();
    for (token, value) in current().tokens() {
        rendered = rendered.replace(token, &value);
    }
    if rendered.contains("__SITE_") {
        bail!("unknown deployment site placeholder in generated text");
    }
    Ok(rendered)
}

/// Render command arguments before they reach ssh, rsync, curl, or any other tool.
pub(crate) fn render_args(args: &[&str]) -> Result<Vec<String>> {
    args.iter().map(|arg| render(arg)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_values_are_the_original_literals() {
        let production = Site::Production;
        assert_eq!(production.host(), "shadowsofwar.io");
        assert_eq!(production.loopback(), "127.0.0.1");
        assert_eq!(production.server_jail(), "sow-server");
        assert_eq!(production.release_root(), "/srv/sow");
        assert_eq!(production.remote_stage(), "/home/bizkit/.sow-deploy");
        assert_eq!(production.origin(), "https://shadowsofwar.io");
        assert_eq!(production.map_prefix(), "");
        assert_eq!(production.label(), "Production");
    }

    #[test]
    fn ptr_never_shares_production_listen_addresses_or_secrets_file() {
        assert_ne!(Site::Ptr.loopback(), Site::Production.loopback());
        assert_ne!(Site::Ptr.server_jail(), Site::Production.server_jail());
        assert_ne!(Site::Ptr.database_jail(), Site::Production.database_jail());
        assert_ne!(Site::Ptr.release_root(), Site::Production.release_root());
        assert_ne!(Site::Ptr.env_file(), Site::Production.env_file());
        assert!(!Site::Ptr.owns_identity_import());
        assert!(!Site::Ptr.builds_portal_packages());
        assert_eq!(Site::Ptr.label(), "PTR");
        assert_ne!(
            Site::Ptr.reward_audit_accounts_file(),
            Site::Production.reward_audit_accounts_file()
        );
        assert_ne!(Site::Ptr.server_http_url(), Site::Production.server_http_url());
    }

    #[test]
    fn ptr_nginx_file_loads_after_production_and_sites_are_peers() {
        assert!(Site::Production.nginx_conf() < Site::Ptr.nginx_conf());
        assert_eq!(Site::Ptr.peer(), Site::Production);
        assert_eq!(Site::Production.peer(), Site::Ptr);
        assert!(!Site::Ptr.is_production());
        assert_ne!(Site::Ptr.map_prefix(), Site::Production.map_prefix());
    }

    #[test]
    fn nginx_maps_never_share_a_variable_between_sites() {
        // Both site files load into one nginx http context, where a duplicate map name fails `nginx -t`.
        let template = include_str!("../deploy/freebsd/conf.d/shadowsofwar.io.conf");
        let map_targets = |site: Site| -> Vec<String> {
            let mut text = template.to_string();
            for (token, value) in site.tokens() {
                text = text.replace(token, &value);
            }
            assert!(!text.contains("__SITE_"), "unrendered placeholder for {site:?}");
            text.lines()
                .filter(|line| line.starts_with("map "))
                .filter_map(|line| line.split_whitespace().nth(2).map(str::to_string))
                .collect()
        };
        let production = map_targets(Site::Production);
        let ptr = map_targets(Site::Ptr);
        assert_eq!(production.len(), 3);
        assert_eq!(ptr.len(), 3);
        assert!(production.iter().all(|name| !ptr.contains(name)));
    }
}
