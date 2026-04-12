use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use std::{
    io::{Read, Write},
    net::{TcpStream, ToSocketAddrs},
};

use anyhow::{anyhow, Context, Result};
use hellox_config::HelloxConfig;

pub fn ensure_gateway_ready(
    config_path: &Path,
    config: &HelloxConfig,
    gateway_url: Option<&str>,
) -> Result<()> {
    let base_url = normalize_base_url(
        gateway_url
            .map(ToString::to_string)
            .unwrap_or_else(|| config.gateway.listen.clone()),
    );

    if !is_local_loopback_gateway(&base_url) || gateway_healthcheck(&base_url) {
        return Ok(());
    }

    spawn_local_gateway(config_path)?;
    wait_for_gateway(&base_url)
}

fn normalize_base_url(value: String) -> String {
    if value.starts_with("http://") || value.starts_with("https://") {
        value
    } else {
        format!("http://{value}")
    }
}

fn is_local_loopback_gateway(base_url: &str) -> bool {
    matches!(
        reqwest::Url::parse(base_url)
            .ok()
            .and_then(|url| url.host_str().map(ToString::to_string))
            .as_deref(),
        Some("127.0.0.1" | "localhost")
    )
}

fn gateway_healthcheck(base_url: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(base_url) else {
        return false;
    };
    if url.scheme() != "http" {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    let Some(port) = url.port_or_known_default() else {
        return false;
    };
    let Some(mut stream) = connect_with_timeout(host, port, Duration::from_millis(800)) else {
        return false;
    };
    let health_path = healthcheck_path(&url);
    let request =
        format!("GET {health_path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n");
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }
    let mut response = String::new();
    if stream.read_to_string(&mut response).is_err() {
        return false;
    }
    response.starts_with("HTTP/1.1 200") || response.starts_with("HTTP/1.0 200")
}

fn connect_with_timeout(host: &str, port: u16, timeout: Duration) -> Option<TcpStream> {
    let addrs = (host, port).to_socket_addrs().ok()?;
    for addr in addrs {
        if let Ok(stream) = TcpStream::connect_timeout(&addr, timeout) {
            let _ = stream.set_read_timeout(Some(timeout));
            let _ = stream.set_write_timeout(Some(timeout));
            return Some(stream);
        }
    }
    None
}

fn healthcheck_path(url: &reqwest::Url) -> String {
    let path = url.path().trim_end_matches('/');
    if path.is_empty() {
        "/health".to_string()
    } else {
        format!("{path}/health")
    }
}

fn spawn_local_gateway(config_path: &Path) -> Result<()> {
    let current_exe = std::env::current_exe().context("failed to resolve current executable")?;
    let mut command = Command::new(current_exe);
    command
        .arg("gateway")
        .arg("serve")
        .arg("--config")
        .arg(config_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command.spawn().context("failed to spawn local gateway")?;
    Ok(())
}

fn wait_for_gateway(base_url: &str) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(6);
    while Instant::now() < deadline {
        if gateway_healthcheck(base_url) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(150));
    }
    Err(anyhow!("local gateway did not become ready at {base_url}"))
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    use super::{
        gateway_healthcheck, healthcheck_path, is_local_loopback_gateway, normalize_base_url,
    };

    #[test]
    fn normalize_base_url_adds_http_scheme_for_listen_addresses() {
        assert_eq!(
            normalize_base_url("127.0.0.1:7821".to_string()),
            "http://127.0.0.1:7821"
        );
    }

    #[test]
    fn local_loopback_gateway_detection_accepts_standard_hosts() {
        assert!(is_local_loopback_gateway("http://127.0.0.1:7821"));
        assert!(is_local_loopback_gateway("http://localhost:7821"));
        assert!(!is_local_loopback_gateway("https://api.example.com"));
    }

    #[test]
    fn healthcheck_path_appends_health_suffix() {
        let url = reqwest::Url::parse("http://127.0.0.1:7821").expect("parse root url");
        assert_eq!(healthcheck_path(&url), "/health");

        let nested = reqwest::Url::parse("http://127.0.0.1:7821/api").expect("parse nested url");
        assert_eq!(healthcheck_path(&nested), "/api/health");
    }

    #[test]
    fn gateway_healthcheck_accepts_http_200_response() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let address = listener.local_addr().expect("listener addr");
        let server = thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut request = [0_u8; 512];
                let _ = stream.read(&mut request);
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
                );
            }
        });

        let result = gateway_healthcheck(&format!("http://127.0.0.1:{}", address.port()));
        server.join().expect("join server");
        assert!(result);
    }

    #[test]
    fn gateway_healthcheck_rejects_non_success_response() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let address = listener.local_addr().expect("listener addr");
        let server = thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut request = [0_u8; 512];
                let _ = stream.read(&mut request);
                thread::sleep(Duration::from_millis(50));
                let _ = stream.write_all(
                    b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                );
            }
        });

        let result = gateway_healthcheck(&format!("http://127.0.0.1:{}", address.port()));
        server.join().expect("join server");
        assert!(!result);
    }
}
