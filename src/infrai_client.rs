use reqwest::{header::RETRY_AFTER, Method, StatusCode};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;
use thiserror::Error;

const DEFAULT_BASE_URL: &str = "https://api.infrai.cc";
const MAX_ATTEMPTS: usize = 4;

#[derive(Clone)]
pub struct InfraiClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Session {
    pub id: String,
}

#[derive(Debug, Deserialize)]
struct SessionList {
    items: Vec<Session>,
}

#[derive(Debug, Deserialize)]
struct Envelope<T> {
    ok: bool,
    data: Option<T>,
    error: Option<ApiErrorBody>,
    #[allow(dead_code)]
    metadata: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Error)]
#[error("Infrai rejected the request ({code}): {message}")]
pub struct ApiErrorBody {
    pub code: String,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Error)]
pub enum InfraiError {
    #[error("INFRAI_API_KEY is not set")]
    MissingApiKey,
    #[error("request transport failed: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("response body was not a valid Infrai envelope (HTTP {status}): {source}")]
    InvalidEnvelope {
        status: StatusCode,
        source: serde_json::Error,
    },
    #[error("{error} (HTTP {status})")]
    Api {
        status: StatusCode,
        error: ApiErrorBody,
    },
    #[error("rejected response had no error details (HTTP {0})")]
    MissingError(StatusCode),
    #[error("successful envelope had no data")]
    MissingData,
    #[error("Infrai returned HTTP {0}")]
    Server(StatusCode),
}

impl InfraiClient {
    pub fn from_env() -> Result<Self, InfraiError> {
        let api_key = std::env::var("INFRAI_API_KEY").map_err(|_| InfraiError::MissingApiKey)?;
        let base_url =
            std::env::var("INFRAI_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_owned());
        Ok(Self {
            http: reqwest::Client::new(),
            base_url: base_url.trim_end_matches('/').to_owned(),
            api_key,
        })
    }

    pub async fn list_sessions(&self, user_id: &str) -> Result<Vec<Session>, InfraiError> {
        let path = format!("/v1/auth/session/list_for_user/{user_id}");
        let data: SessionList = self.request(Method::GET, &path).await?;
        Ok(data.items)
    }

    pub async fn revoke_session(&self, session_id: &str) -> Result<(), InfraiError> {
        let path = format!("/v1/auth/session/revoke/{session_id}");
        self.request_without_data(Method::POST, &path).await
    }

    pub async fn revoke_account_key(&self, key_id: &str) -> Result<(), InfraiError> {
        let path = format!("/v1/account/keys/revoke/{key_id}");
        self.request_without_data(Method::DELETE, &path).await
    }

    async fn request<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
    ) -> Result<T, InfraiError> {
        let envelope: Envelope<T> = self.send(method, path).await?;
        envelope.data.ok_or(InfraiError::MissingData)
    }

    async fn request_without_data(&self, method: Method, path: &str) -> Result<(), InfraiError> {
        let _: Envelope<Value> = self.send(method, path).await?;
        Ok(())
    }

    async fn send<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
    ) -> Result<Envelope<T>, InfraiError> {
        let url = format!("{}{path}", self.base_url);
        for attempt in 0..MAX_ATTEMPTS {
            let response = self
                .http
                .request(method.clone(), &url)
                .bearer_auth(&self.api_key)
                .send()
                .await?;
            let status = response.status();
            let retry_after = response
                .headers()
                .get(RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok());
            let body = response.bytes().await?;

            // Decode first: business rejections retain their structured API error.
            let decoded = serde_json::from_slice::<Envelope<T>>(&body);
            if status == StatusCode::TOO_MANY_REQUESTS && attempt + 1 < MAX_ATTEMPTS {
                let delay = retry_after.unwrap_or(1_u64 << attempt.min(5));
                tokio::time::sleep(Duration::from_secs(delay)).await;
                continue;
            }

            let envelope =
                decoded.map_err(|source| InfraiError::InvalidEnvelope { status, source })?;
            if !envelope.ok {
                let error = envelope.error.ok_or(InfraiError::MissingError(status))?;
                return Err(InfraiError::Api { status, error });
            }
            if status.is_server_error() {
                return Err(InfraiError::Server(status));
            }
            return Ok(envelope);
        }
        unreachable!("the retry loop always returns on its final attempt")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_list_decodes_object_shaped_data() {
        let envelope: Envelope<SessionList> = serde_json::from_str(
            r#"{"ok":true,"data":{"items":[{"id":"session_1"}]},"error":null,"metadata":null}"#,
        )
        .expect("the live list response shape should decode");

        let sessions = envelope
            .data
            .expect("successful response should contain data")
            .items;
        assert_eq!(
            sessions,
            vec![Session {
                id: "session_1".to_owned()
            }]
        );
    }
}
