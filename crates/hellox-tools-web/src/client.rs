use std::sync::OnceLock;
use std::time::Duration;

use reqwest::Client;

const USER_AGENT: &str = "hellox/0.1";

static CLIENT: OnceLock<Client> = OnceLock::new();

pub fn http_client() -> Client {
    CLIENT.get_or_init(build_client).clone()
}

fn build_client() -> Client {
    Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(20))
        .build()
        .expect("valid HTTP client")
}
