//! Shared SMTP configuration for transactional account and moderation mail.
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

pub(crate) async fn send_message(to: &str, subject: &str, body: String) -> Result<(), String> {
    let host = std::env::var("SOW_SMTP_HOST").map_err(|_| "SMTP is not configured")?;
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
    let builder = if port == 465 {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&host)
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&host)
    }
    .map_err(|_| "Could not configure SMTP TLS")?;
    let transport = builder
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
