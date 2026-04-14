use codex_api::CoreAuthProvider;
use codex_model_provider::ProviderAuthStrategy;
use codex_model_provider::ResolvedModelProvider;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::error::CodexErr;
use codex_protocol::error::EnvVarError;

use crate::CodexAuth;

pub fn auth_provider_from_auth(
    auth: Option<CodexAuth>,
    provider: &ModelProviderInfo,
) -> codex_protocol::error::Result<CoreAuthProvider> {
    let resolved_provider = ResolvedModelProvider::resolve(provider.name.clone(), provider.clone())
        .map_err(|err| CodexErr::Fatal(err.to_string()))?;

    match resolved_provider.auth_strategy() {
        ProviderAuthStrategy::EnvBearer {
            env_key,
            env_key_instructions,
        } => {
            let token = std::env::var(env_key)
                .ok()
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    CodexErr::EnvVar(EnvVarError {
                        var: env_key.clone(),
                        instructions: env_key_instructions.clone(),
                    })
                })?;
            Ok(CoreAuthProvider {
                token: Some(token),
                account_id: None,
            })
        }
        ProviderAuthStrategy::ExperimentalBearer { token } => Ok(CoreAuthProvider {
            token: Some(token.clone()),
            account_id: None,
        }),
        ProviderAuthStrategy::OpenAi
        | ProviderAuthStrategy::ExternalBearer { .. }
        | ProviderAuthStrategy::NoProviderAuth => auth_provider_from_codex_auth(auth),
    }
}

fn auth_provider_from_codex_auth(
    auth: Option<CodexAuth>,
) -> codex_protocol::error::Result<CoreAuthProvider> {
    if let Some(auth) = auth {
        let token = auth.get_token()?;
        Ok(CoreAuthProvider {
            token: Some(token),
            account_id: auth.get_account_id(),
        })
    } else {
        Ok(CoreAuthProvider {
            token: None,
            account_id: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_model_provider_info::ModelProviderInfo;
    use codex_model_provider_info::WireApi;
    use codex_protocol::config_types::ModelProviderAuthInfo;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use std::num::NonZeroU64;

    const MISSING_ENV_KEY: &str =
        "CODEX_TEST_AUTH_PROVIDER_FROM_AUTH_MISSING_PROVIDER_KEY_9F54D778";

    fn custom_provider() -> ModelProviderInfo {
        ModelProviderInfo {
            name: "Test Provider".to_string(),
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

    #[test]
    fn openai_auth_uses_supplied_codex_auth() {
        let provider = ModelProviderInfo::create_openai_provider(/*base_url*/ None);

        let auth = auth_provider_from_auth(Some(CodexAuth::from_api_key("openai-key")), &provider)
            .expect("auth provider");

        assert_eq!(auth.token.as_deref(), Some("openai-key"));
        assert_eq!(auth.account_id, None);
    }

    #[test]
    fn custom_no_auth_preserves_supplied_codex_auth_behavior() {
        let provider = custom_provider();

        let auth = auth_provider_from_auth(Some(CodexAuth::from_api_key("custom-key")), &provider)
            .expect("auth provider");

        assert_eq!(auth.token.as_deref(), Some("custom-key"));
        assert_eq!(auth.account_id, None);
    }

    #[test]
    fn experimental_bearer_overrides_supplied_codex_auth() {
        let mut provider = custom_provider();
        provider.experimental_bearer_token = Some("provider-token".to_string());

        let auth = auth_provider_from_auth(Some(CodexAuth::from_api_key("ignored")), &provider)
            .expect("auth provider");

        assert_eq!(auth.token.as_deref(), Some("provider-token"));
        assert_eq!(auth.account_id, None);
    }

    #[test]
    fn env_bearer_reports_missing_env_key() {
        let mut provider = custom_provider();
        provider.env_key = Some(MISSING_ENV_KEY.to_string());
        provider.env_key_instructions = Some("Set the test key.".to_string());

        let err = match auth_provider_from_auth(/*auth*/ None, &provider) {
            Ok(_) => panic!("expected missing env var"),
            Err(err) => err,
        };

        let CodexErr::EnvVar(err) = err else {
            panic!("expected env var error");
        };
        assert_eq!(err.var, MISSING_ENV_KEY);
        assert_eq!(err.instructions.as_deref(), Some("Set the test key."));
    }

    #[test]
    fn external_bearer_uses_supplied_codex_auth() {
        let mut provider = custom_provider();
        provider.auth = Some(ModelProviderAuthInfo {
            command: "credential-helper".to_string(),
            args: vec!["token".to_string()],
            timeout_ms: NonZeroU64::new(10_000).unwrap(),
            refresh_interval_ms: 300_000,
            cwd: AbsolutePathBuf::from_absolute_path("/tmp").unwrap(),
        });

        let auth =
            auth_provider_from_auth(Some(CodexAuth::from_api_key("command-token")), &provider)
                .expect("auth provider");

        assert_eq!(auth.token.as_deref(), Some("command-token"));
        assert_eq!(auth.account_id, None);
    }
}
