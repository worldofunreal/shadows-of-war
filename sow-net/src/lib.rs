pub mod client;

/// A host in Start selects direct relay transport. Otherwise the orchestrator
/// origin proxies the local relay, with the internal port encoded in the path.
pub fn relay_url(
    orchestrator: &url::Url,
    host: Option<&str>,
    port: u16,
) -> Option<url::Url> {
    let mut url = orchestrator.clone();
    if let Some(host) = host {
        url.set_host(Some(host)).ok()?;
        url.set_port(Some(port)).ok()?;
        url.set_path("/ws/");
    } else {
        url.set_path(&format!("/relay/{port}/ws/"));
    }
    url.set_query(None);
    url.set_fragment(None);
    Some(url)
}
