//! Test-account loading. The password stays off `Debug` and out of errors.

use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

/// Hercules TEST login. Live scenarios refuse every other endpoint.
pub const TEST_HOST: &str = "192.168.11.110";
pub const TEST_PORT: u16 = 6910;

const POOL_ACCOUNT: &str = "/root/Work/private-gap-accounts/kgap29.json";
const M1_ACCOUNT: &str = "/root/Work/private-korangar-test-account.json";

/// Why a live session did not start, or the value it produced.
pub enum LiveConnect<T> {
    /// Missing opt-in, file, or TEST. Callers print this and return.
    Skip(String),
    /// The account file was opted in but cannot be used. Callers fail.
    Invalid(String),
    Ready(T),
}

pub struct Credentials {
    pub username: String,
    password: String,
    pub host: String,
    pub port: u16,
}

impl Credentials {
    pub fn password(&self) -> &str {
        &self.password
    }

    pub fn redact(&self, text: &str) -> String {
        if self.password.is_empty() {
            text.to_owned()
        } else {
            text.replace(&self.password, "[redacted]")
        }
    }

    /// Live tests stay opt-in so a normal `cargo test` never dials TEST.
    pub fn resolve() -> LiveConnect<Credentials> {
        if !opted_in() {
            return LiveConnect::Skip("set KORANGAR_E2E=1 or KORANGAR_E2E_ACCOUNT to run live TEST scenarios".to_owned());
        }
        let Some(path) = account_path() else {
            return LiveConnect::Skip("no account file (KORANGAR_E2E_ACCOUNT, KORANGAR_AUTOPILOT, or the verify pool)".to_owned());
        };
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(_) => return LiveConnect::Invalid("cannot read the e2e account file".to_owned()),
        };
        let credentials = match parse_account(&text) {
            Ok(credentials) => credentials,
            Err(error) => return LiveConnect::Invalid(error),
        };
        if credentials.host != TEST_HOST || credentials.port != TEST_PORT {
            return LiveConnect::Skip(format!("refusing login that is not {TEST_HOST}:{TEST_PORT}"));
        }
        if !tcp_open(&credentials) {
            return LiveConnect::Skip(format!("{TEST_HOST}:{TEST_PORT} unreachable"));
        }
        LiveConnect::Ready(credentials)
    }
}

impl Drop for Credentials {
    fn drop(&mut self) {
        self.password.clear();
    }
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Credentials")
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .field("host", &self.host)
            .field("port", &self.port)
            .finish()
    }
}

#[derive(Deserialize)]
struct AccountFile {
    username: String,
    password: String,
    login_address: String,
}

pub(crate) fn parse_account(text: &str) -> Result<Credentials, String> {
    let account: AccountFile =
        serde_json::from_str(text).map_err(|_| "account file must be JSON with username, password, and login_address".to_owned())?;
    if account.username.is_empty() || account.password.is_empty() || account.login_address.is_empty() {
        return Err("account file is missing username, password, or login_address".to_owned());
    }
    let (host, port) = account
        .login_address
        .trim()
        .rsplit_once(':')
        .ok_or_else(|| "login_address must be host:port".to_owned())?;
    let port = port.parse::<u16>().map_err(|_| "login_address has a bad port".to_owned())?;
    Ok(Credentials {
        username: account.username,
        password: account.password,
        host: host.to_owned(),
        port,
    })
}

fn opted_in() -> bool {
    flag("KORANGAR_E2E") || nonempty("KORANGAR_E2E_ACCOUNT").is_some() || nonempty("KORANGAR_AUTOPILOT").is_some()
}

fn account_path() -> Option<PathBuf> {
    if let Some(path) = nonempty("KORANGAR_E2E_ACCOUNT") {
        return Some(PathBuf::from(path));
    }
    if let Some(path) = nonempty("KORANGAR_AUTOPILOT") {
        return Some(PathBuf::from(path));
    }
    if Path::new(POOL_ACCOUNT).is_file() {
        return Some(PathBuf::from(POOL_ACCOUNT));
    }
    if Path::new(M1_ACCOUNT).is_file() {
        return Some(PathBuf::from(M1_ACCOUNT));
    }
    None
}

fn nonempty(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn flag(name: &str) -> bool {
    match std::env::var(name) {
        Ok(value) => {
            let value = value.trim();
            !(value.is_empty() || value == "0" || value.eq_ignore_ascii_case("false"))
        }
        Err(_) => false,
    }
}

fn tcp_open(credentials: &Credentials) -> bool {
    let address = format!("{}:{}", credentials.host, credentials.port);
    let Ok(address) = address.parse::<SocketAddr>() else {
        return false;
    };
    TcpStream::connect_timeout(&address, Duration::from_secs(2)).is_ok()
}

#[cfg(test)]
mod tests {
    use super::parse_account;

    #[test]
    fn debug_redacts_the_password() {
        let credentials =
            parse_account(r#"{"username":"kgap29","password":"s3cret-value","login_address":"192.168.11.110:6910"}"#).unwrap();
        let rendered = format!("{credentials:?}");
        assert!(!rendered.contains("s3cret-value"));
        assert!(rendered.contains("<redacted>"));
        assert!(credentials.redact("bad s3cret-value").contains("[redacted]"));
        assert!(!credentials.redact("bad s3cret-value").contains("s3cret-value"));
    }

    #[test]
    fn rejects_a_non_test_endpoint_string() {
        let credentials = parse_account(r#"{"username":"kgap29","password":"s3cret-value","login_address":"203.0.113.5:6900"}"#).unwrap();
        assert_ne!(credentials.host, super::TEST_HOST);
    }
}
