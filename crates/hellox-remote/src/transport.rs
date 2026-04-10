use anyhow::{anyhow, Context, Result};
use hellox_server::{
    DirectConnectConfig, DirectConnectRequest, ServerSessionDetail, ServerSessionSummary,
};
use reqwest::blocking::{Client, RequestBuilder};
use reqwest::header::AUTHORIZATION;
use reqwest::Method;

use crate::environment::ResolvedRemoteEnvironment;

pub trait RemoteSessionTransport {
    fn create_direct_connect_session(
        &self,
        request: DirectConnectRequest,
    ) -> Result<DirectConnectConfig>;
    fn list_sessions(&self) -> Result<Vec<ServerSessionSummary>>;
    fn load_session_detail(&self, session_id: &str) -> Result<ServerSessionDetail>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRemoteSessionTransport {
    access: ResolvedRemoteEnvironment,
}

impl HttpRemoteSessionTransport {
    pub fn new(access: ResolvedRemoteEnvironment) -> Self {
        Self { access }
    }

    pub fn access(&self) -> &ResolvedRemoteEnvironment {
        &self.access
    }

    fn request_json(&self, method: Method, path: &str) -> RequestBuilder {
        let url = format!("{}{}", self.access.server_url.trim_end_matches('/'), path);
        let mut request = Client::new().request(method, url).header(
            AUTHORIZATION,
            format!("Bearer {}", self.access.access_token),
        );
        if let Some(device_token) = self.access.device_token.as_deref() {
            request = request.header("x-hellox-device-token", device_token);
        }
        request
    }
}

impl RemoteSessionTransport for HttpRemoteSessionTransport {
    fn create_direct_connect_session(
        &self,
        request: DirectConnectRequest,
    ) -> Result<DirectConnectConfig> {
        self.request_json(Method::POST, "/sessions")
            .json(&request)
            .send()
            .context("failed to create remote direct-connect session")
            .and_then(read_json_response)
    }

    fn list_sessions(&self) -> Result<Vec<ServerSessionSummary>> {
        self.request_json(Method::GET, "/sessions")
            .send()
            .context("failed to list remote sessions")
            .and_then(read_json_response)
    }

    fn load_session_detail(&self, session_id: &str) -> Result<ServerSessionDetail> {
        self.request_json(Method::GET, &format!("/sessions/{session_id}"))
            .send()
            .context("failed to load remote session")
            .and_then(read_json_response)
    }
}

fn read_json_response<T>(response: reqwest::blocking::Response) -> Result<T>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let status = response.status();
    if !status.is_success() {
        let body = response
            .text()
            .unwrap_or_else(|_| "unable to read response body".to_string());
        return Err(anyhow!(
            "remote request failed with status {}: {}",
            status,
            extract_remote_error(&body)
        ));
    }
    response
        .json()
        .context("failed to parse JSON response from remote server")
}

fn extract_remote_error(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("error")
                .and_then(|item| item.get("message"))
                .and_then(|item| item.as_str())
                .map(ToString::to_string)
        })
        .filter(|message| !message.is_empty())
        .unwrap_or_else(|| body.trim().to_string())
}
