use std::sync::Arc;

use codex_model_provider::ProviderAuthStrategy;
use codex_model_provider::ResolvedModelProvider;
use codex_model_provider_info::ModelProviderInfo;

use crate::AuthManager;

/// Returns the provider-scoped auth manager when this provider uses command-backed auth.
///
/// Providers without custom auth continue using the caller-supplied base manager.
pub fn auth_manager_for_provider(
    auth_manager: Option<Arc<AuthManager>>,
    provider: &ModelProviderInfo,
) -> Option<Arc<AuthManager>> {
    external_bearer_auth_manager(provider).or(auth_manager)
}

/// Whether this provider uses command-backed bearer-token auth.
pub fn provider_uses_external_bearer_auth(provider: &ModelProviderInfo) -> bool {
    matches!(
        resolved_provider_auth(provider),
        Some(ProviderAuthStrategy::ExternalBearer { .. })
    )
}

/// Returns an auth manager for request paths that always require authentication.
///
/// Providers with command-backed auth get a bearer-only manager; otherwise the caller's manager
/// is reused unchanged.
pub fn required_auth_manager_for_provider(
    auth_manager: Arc<AuthManager>,
    provider: &ModelProviderInfo,
) -> Arc<AuthManager> {
    external_bearer_auth_manager(provider).unwrap_or(auth_manager)
}

fn external_bearer_auth_manager(provider: &ModelProviderInfo) -> Option<Arc<AuthManager>> {
    match resolved_provider_auth(provider)? {
        ProviderAuthStrategy::ExternalBearer { config } => {
            Some(AuthManager::external_bearer_only(config))
        }
        ProviderAuthStrategy::OpenAi
        | ProviderAuthStrategy::EnvBearer { .. }
        | ProviderAuthStrategy::ExperimentalBearer { .. }
        | ProviderAuthStrategy::NoProviderAuth => None,
    }
}

fn resolved_provider_auth(provider: &ModelProviderInfo) -> Option<ProviderAuthStrategy> {
    ResolvedModelProvider::resolve(provider.name.clone(), provider.clone())
        .ok()
        .map(|resolved_provider| resolved_provider.auth_strategy().clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_app_server_protocol::AuthMode;
    use codex_model_provider_info::WireApi;
    use codex_protocol::config_types::ModelProviderAuthInfo;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use std::num::NonZeroU64;

    use crate::CodexAuth;

    fn provider() -> ModelProviderInfo {
        ModelProviderInfo {
            name: "test".to_string(),
            base_url: Some("https://example.com/v1".to_string()),
            env_key: None,
            env_key_instructions: None,
            experimental_bearer_token: None,
            auth: None,
            wire_api: WireApi::Responses,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: None,
            stream_max_retries: None,
            stream_idle_timeout_ms: None,
            websocket_connect_timeout_ms: None,
            requires_openai_auth: false,
            supports_websockets: false,
        }
    }

    fn external_auth_config() -> ModelProviderAuthInfo {
        let cwd = std::env::current_dir().expect("current dir");
        ModelProviderAuthInfo {
            command: "echo".to_string(),
            args: vec!["provider-token".to_string()],
            timeout_ms: NonZeroU64::new(1_000).unwrap(),
            refresh_interval_ms: 60_000,
            cwd: AbsolutePathBuf::try_from(cwd).expect("cwd should be absolute"),
        }
    }

    #[test]
    fn auth_manager_for_provider_uses_external_bearer_auth() {
        let provider = ModelProviderInfo {
            auth: Some(external_auth_config()),
            ..provider()
        };

        assert!(provider_uses_external_bearer_auth(&provider));

        let auth_manager = auth_manager_for_provider(/*auth_manager*/ None, &provider)
            .expect("external bearer auth manager");

        assert_eq!(auth_manager.auth_mode(), Some(AuthMode::ApiKey));
        assert_eq!(auth_manager.auth_cached(), None);
    }

    #[test]
    fn auth_manager_for_provider_ignores_non_external_provider_auth() {
        let provider = ModelProviderInfo {
            env_key: Some("TEST_PROVIDER_API_KEY".to_string()),
            ..provider()
        };

        let auth_manager = auth_manager_for_provider(/*auth_manager*/ None, &provider);

        assert!(!provider_uses_external_bearer_auth(&provider));
        assert!(auth_manager.is_none());
    }

    #[test]
    fn required_auth_manager_for_provider_reuses_base_manager_without_external_auth() {
        let auth_manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("base"));

        let scoped_auth_manager =
            required_auth_manager_for_provider(auth_manager.clone(), &provider());

        assert!(Arc::ptr_eq(&scoped_auth_manager, &auth_manager));
    }

    #[test]
    fn required_auth_manager_for_provider_uses_external_bearer_auth() {
        let auth_manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("base"));
        let provider = ModelProviderInfo {
            auth: Some(external_auth_config()),
            ..provider()
        };

        let scoped_auth_manager = required_auth_manager_for_provider(auth_manager, &provider);

        assert_eq!(scoped_auth_manager.auth_mode(), Some(AuthMode::ApiKey));
        assert_eq!(scoped_auth_manager.auth_cached(), None);
    }
}
