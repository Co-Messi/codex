use codex_api::AuthProvider;
use http::HeaderMap;
use http::HeaderValue;

#[derive(Clone)]
pub struct CoreAuthProvider {
    pub token: Option<String>,
    pub account_id: Option<String>,
}

impl CoreAuthProvider {
    pub fn for_test(token: Option<&str>, account_id: Option<&str>) -> Self {
        Self {
            token: token.map(str::to_string),
            account_id: account_id.map(str::to_string),
        }
    }
}

impl AuthProvider for CoreAuthProvider {
    fn add_auth_headers(&self, headers: &mut HeaderMap) {
        if let Some(token) = self.token.as_ref()
            && let Ok(header) = HeaderValue::from_str(&format!("Bearer {token}"))
        {
            let _ = headers.insert(http::header::AUTHORIZATION, header);
        }
        if let Some(account_id) = self.account_id.as_ref()
            && let Ok(header) = HeaderValue::from_str(account_id)
        {
            let _ = headers.insert("ChatGPT-Account-ID", header);
        }
    }

    fn auth_header_attached(&self) -> bool {
        self.token
            .as_ref()
            .is_some_and(|token| HeaderValue::from_str(&format!("Bearer {token}")).is_ok())
    }

    fn auth_header_name(&self) -> Option<&'static str> {
        self.auth_header_attached().then_some("authorization")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn core_auth_provider_reports_when_auth_header_will_attach() {
        let auth = CoreAuthProvider {
            token: Some("access-token".to_string()),
            account_id: None,
        };

        assert!(auth.auth_header_attached());
        assert_eq!(auth.auth_header_name(), Some("authorization"));
    }

    #[test]
    fn core_auth_provider_adds_auth_headers() {
        let auth = CoreAuthProvider::for_test(Some("access-token"), Some("workspace-123"));
        let mut headers = HeaderMap::new();

        auth.add_auth_headers(&mut headers);

        assert_eq!(
            headers
                .get(http::header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok()),
            Some("Bearer access-token")
        );
        assert_eq!(
            headers
                .get("ChatGPT-Account-ID")
                .and_then(|value| value.to_str().ok()),
            Some("workspace-123")
        );
    }
}
