//! Shared SMTP configuration for transactional account and moderation mail.
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::client::{Tls, TlsParameters};
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

pub(crate) async fn send_message(to: &str, subject: &str, body: String) -> Result<(), String> {
    let host = std::env::var("SOW_SMTP_HOST").map_err(|_| "SMTP is not configured")?;
    // The certificate is always checked against SOW_SMTP_HOST. SOW_SMTP_CONNECT_HOST only changes where the socket connects.
    let connect = std::env::var("SOW_SMTP_CONNECT_HOST").ok().filter(|value| !value.is_empty()).unwrap_or_else(|| host.clone());
    let user = std::env::var("SOW_SMTP_USER").map_err(|_| "SMTP is not configured")?;
    let password = std::env::var("SOW_SMTP_PASS").map_err(|_| "SMTP is not configured")?;
    let from = std::env::var("SOW_SMTP_FROM").unwrap_or_else(|_| user.clone());
    let port: u16 = std::env::var("SOW_SMTP_PORT")
        .unwrap_or_else(|_| "587".into())
        .parse()
        .map_err(|_| "Invalid SMTP port")?;
    let message = Message::builder()
        .from(from.parse().map_err(|_| "Invalid sender mailbox")?)
        .to(to.parse().map_err(|_| "Invalid recipient mailbox")?)
        .subject(subject)
        .body(body)
        .map_err(|_| "Could not build email")?;
    let tls = TlsParameters::new(host).map_err(|_| "Could not configure SMTP TLS")?;
    let tls = if port == 465 { Tls::Wrapper(tls) } else { Tls::Required(tls) };
    let transport = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(connect)
        .tls(tls)
        .credentials(Credentials::new(user, password))
        .port(port)
        .timeout(Some(std::time::Duration::from_secs(10)))
        .build();
    transport
        .send(message)
        .await
        .map_err(|_| "Email delivery failed")?;
    Ok(())
}
