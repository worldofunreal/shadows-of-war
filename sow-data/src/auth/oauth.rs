use super::AuthService;

pub(super) struct SocialIdentity {
    pub subject: String,
    pub verified_email: Option<String>,
}

pub(super) fn credentials(provider: &str) -> Result<(String, String), String> {
    if !matches!(provider, "google" | "discord" | "twitter" | "meta") {
        return Err("Unsupported social provider".into());
    }
    let prefix = format!("SOW_{}", provider.to_uppercase());
    let id = std::env::var(format!("{prefix}_CLIENT_ID"))
        .ok()
        .filter(|v| !v.is_empty())
        .ok_or("Social provider is not configured")?;
    let secret = std::env::var(format!("{prefix}_CLIENT_SECRET"))
        .ok()
        .filter(|v| !v.is_empty())
        .ok_or("Social provider is not configured")?;
    Ok((id, secret))
}

/// The public host this deployment serves. Each site sets SOW_PUBLIC_HOST; production is the default.
pub fn public_host() -> String {
    std::env::var("SOW_PUBLIC_HOST")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "shadowsofwar.io".to_string())
}

pub(super) fn allowed_redirect(value: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(value) else {
        return false;
    };
    if url.path() != "/auth/callback"
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return false;
    }
    (url.scheme() == "https" && url.host_str() == Some(public_host().as_str()) && url.port().is_none())
        || (url.scheme() == "http"
            && matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]")))
}

pub(super) fn authorization_url(
    provider: &str,
    client_id: &str,
    redirect: &str,
    state: &str,
    challenge: Option<&str>,
) -> Result<String, String> {
    let (base, scope) = match provider {
        "google" => (
            "https://accounts.google.com/o/oauth2/v2/auth",
            "openid profile email",
        ),
        "discord" => ("https://discord.com/api/oauth2/authorize", "identify email"),
        "twitter" => ("https://x.com/i/oauth2/authorize", "users.read tweet.read"),
        "meta" => (
            "https://www.facebook.com/dialog/oauth",
            "public_profile,email",
        ),
        _ => return Err("Unsupported social provider".into()),
    };
    let mut url = reqwest::Url::parse(base).map_err(|_| "Invalid provider endpoint")?;
    url.query_pairs_mut().extend_pairs([
        ("client_id", client_id),
        ("redirect_uri", redirect),
        ("response_type", "code"),
        ("state", state),
        ("scope", scope),
    ]);
    if provider == "twitter" {
        let challenge = challenge.ok_or("X requires S256 PKCE")?;
        url.query_pairs_mut()
            .append_pair("code_challenge", challenge)
            .append_pair("code_challenge_method", "S256");
    }
    Ok(url.into())
}

impl AuthService {
    pub(super) async fn exchange_social(
        &self,
        provider: &str,
        code: &str,
        redirect: &str,
        verifier: Option<&str>,
    ) -> Result<SocialIdentity, String> {
        let (client_id, client_secret) = credentials(provider)?;
        let token_endpoint = match provider {
            "google" => "https://oauth2.googleapis.com/token",
            "discord" => "https://discord.com/api/v10/oauth2/token",
            "twitter" => "https://api.x.com/2/oauth2/token",
            "meta" => "https://graph.facebook.com/oauth/access_token",
            _ => return Err("Unsupported social provider".into()),
        };
        let mut params = vec![
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect),
        ];
        let request = if provider == "twitter" {
            params.push((
                "code_verifier",
                verifier.ok_or("X requires a PKCE verifier")?,
            ));
            self.http
                .post(token_endpoint)
                .basic_auth(&client_id, Some(&client_secret))
                .form(&params)
        } else {
            params.extend([
                ("client_id", client_id.as_str()),
                ("client_secret", client_secret.as_str()),
            ]);
            if provider == "meta" {
                self.http.get(token_endpoint).query(&params)
            } else {
                self.http.post(token_endpoint).form(&params)
            }
        };
        let response = request
            .send()
            .await
            .map_err(|_| "Social provider is unavailable")?;
        if !response.status().is_success() {
            return Err("Social authorization code was rejected".into());
        }
        let token: serde_json::Value = response
            .json()
            .await
            .map_err(|_| "Invalid social token response")?;
        let access = token
            .get("access_token")
            .and_then(|v| v.as_str())
            .filter(|v| !v.is_empty())
            .ok_or("Social provider omitted its token")?;
        let user_endpoint = match provider {
            "google" => "https://openidconnect.googleapis.com/v1/userinfo",
            "discord" => "https://discord.com/api/v10/users/@me",
            "twitter" => "https://api.x.com/2/users/me",
            "meta" => "https://graph.facebook.com/me?fields=id,name,email",
            _ => unreachable!(),
        };
        let response = self
            .http
            .get(user_endpoint)
            .bearer_auth(access)
            .send()
            .await
            .map_err(|_| "Social profile is unavailable")?;
        if !response.status().is_success() {
            return Err("Social identity was rejected".into());
        }
        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|_| "Invalid social profile response")?;
        let profile = if provider == "twitter" {
            body.get("data").ok_or("X omitted its profile")?
        } else {
            &body
        };
        let subject = profile
            .get(if provider == "google" { "sub" } else { "id" })
            .and_then(|v| v.as_str())
            .filter(|v| !v.is_empty())
            .ok_or("Social provider omitted its subject")?;
        let email_verified = match provider {
            "google" => profile
                .get("email_verified")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            "discord" => profile
                .get("verified")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            _ => false,
        };
        let email = if email_verified {
            profile
                .get("email")
                .and_then(|v| v.as_str())
                .map(|v| v.trim().to_lowercase())
        } else {
            None
        };
        Ok(SocialIdentity {
            subject: subject.to_owned(),
            verified_email: email,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn redirects_and_provider_scopes_stay_bound_to_the_game() {
        assert!(allowed_redirect("https://shadowsofwar.io/auth/callback"));
        assert!(allowed_redirect("http://localhost:4173/auth/callback"));
        for value in [
            "https://shadowsofwar.io.evil.test/auth/callback",
            "https://shadowsofwar.io/auth/callback?next=evil",
            "http://shadowsofwar.io/auth/callback",
            "https://evil.test/auth/callback",
        ] {
            assert!(!allowed_redirect(value));
        }
        assert!(
            authorization_url(
                "twitter",
                "client",
                "https://shadowsofwar.io/auth/callback",
                "state",
                None
            )
            .is_err()
        );
        let url = reqwest::Url::parse(
            &authorization_url(
                "google",
                "client",
                "https://shadowsofwar.io/auth/callback",
                "a&b",
                None,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            url.query_pairs().find(|(k, _)| k == "state").unwrap().1,
            "a&b"
        );
        assert_eq!(
            url.query_pairs().find(|(k, _)| k == "scope").unwrap().1,
            "openid profile email"
        );
    }
}
