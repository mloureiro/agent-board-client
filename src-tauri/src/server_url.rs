use std::net::{Ipv4Addr, Ipv6Addr};

use serde::Serialize;
use url::{Host, Url};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ServerUrlError {
    Invalid,
    Insecure,
    Credentials,
}

fn accepts_plain_http(host: &Host<&str>) -> bool {
    match host {
        Host::Domain(name) => *name == "localhost",
        Host::Ipv4(address) => *address == Ipv4Addr::LOCALHOST,
        Host::Ipv6(address) => *address == Ipv6Addr::LOCALHOST,
    }
}

pub fn check_server_url(input: &str) -> Result<Url, ServerUrlError> {
    let trimmed = input.trim();
    if trimmed.chars().any(char::is_whitespace) {
        return Err(ServerUrlError::Invalid);
    }

    let url = Url::parse(trimmed).map_err(|_| ServerUrlError::Invalid)?;
    let is_http = url.scheme() == "http";
    let is_https = url.scheme() == "https";
    let Some(host) = url.host().filter(|_| is_http || is_https) else {
        return Err(ServerUrlError::Invalid);
    };

    if !url.username().is_empty() || url.password().is_some() {
        return Err(ServerUrlError::Credentials);
    }

    if is_http && !accepts_plain_http(&host) {
        return Err(ServerUrlError::Insecure);
    }

    Ok(url)
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Deserialize)]
    struct Case {
        input: String,
        url: Option<String>,
        error: Option<String>,
    }

    #[test]
    fn obeys_the_case_table_shared_with_the_frontend() {
        let cases: Vec<Case> =
            serde_json::from_str(include_str!("../../fixtures/server-url-cases.json")).unwrap();

        for case in cases {
            let actual = check_server_url(&case.input);

            let actual_url = actual.as_ref().ok().map(|url| url.as_str().to_owned());
            let actual_error = actual
                .as_ref()
                .err()
                .map(|error| serde_json::to_value(error).unwrap());
            assert_eq!(actual_url, case.url, "url for {:?}", case.input);
            assert_eq!(
                actual_error,
                case.error.map(serde_json::Value::String),
                "error for {:?}",
                case.input
            );
        }
    }
}
