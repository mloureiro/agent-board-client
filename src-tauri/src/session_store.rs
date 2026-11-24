use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

const TOKEN_PREFIX: &str = "abs_";
const TOKEN_BODY_LENGTH: usize = 43;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CookieName {
    #[serde(rename = "__Host-board-session")]
    Host,
    #[serde(rename = "board-session")]
    Plain,
}

impl CookieName {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Host => "__Host-board-session",
            Self::Plain => "board-session",
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoredSession {
    pub name: CookieName,
    pub token: String,
}

impl fmt::Debug for StoredSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StoredSession")
            .field("name", &self.name)
            .field("token", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeError {
    NotJson,
    Name,
    Token,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreError(pub String);

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

pub fn check_token(token: &str) -> Result<(), ShapeError> {
    let valid = token.strip_prefix(TOKEN_PREFIX).is_some_and(|body| {
        body.len() == TOKEN_BODY_LENGTH
            && body
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
    });
    if valid {
        Ok(())
    } else {
        Err(ShapeError::Token)
    }
}

pub fn parse_stored(value: &str) -> Result<StoredSession, ShapeError> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Raw {
        name: String,
        token: String,
    }

    let raw: Raw = serde_json::from_str(value).map_err(|_| ShapeError::NotJson)?;
    let name = match raw.name.as_str() {
        "__Host-board-session" => CookieName::Host,
        "board-session" => CookieName::Plain,
        _ => return Err(ShapeError::Name),
    };
    check_token(&raw.token)?;
    Ok(StoredSession {
        name,
        token: raw.token,
    })
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
mod backend {
    use super::{StoreError, Uuid};

    const SERVICE: &str = "com.mloureiro.agent-board-client.session";

    fn entry(server: Uuid) -> Result<keyring::Entry, StoreError> {
        keyring::Entry::new(SERVICE, &server.to_string())
            .map_err(|error| StoreError(error.to_string()))
    }

    pub fn get(server: Uuid) -> Result<Option<String>, StoreError> {
        match entry(server)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(StoreError(error.to_string())),
        }
    }

    pub fn remove(server: Uuid) -> Result<(), StoreError> {
        match entry(server)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(StoreError(error.to_string())),
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
mod backend {
    use super::{StoreError, Uuid};

    pub fn get(_server: Uuid) -> Result<Option<String>, StoreError> {
        Ok(None)
    }

    pub fn remove(_server: Uuid) -> Result<(), StoreError> {
        Ok(())
    }
}

pub fn read(server: Uuid) -> Result<(Option<StoredSession>, bool), StoreError> {
    let Some(value) = backend::get(server)? else {
        return Ok((None, false));
    };
    match parse_stored(&value) {
        Ok(session) => Ok((Some(session), false)),
        Err(_) => {
            backend::remove(server)?;
            Ok((None, true))
        }
    }
}

pub fn delete(server: Uuid) -> Result<(), StoreError> {
    backend::remove(server)
}

pub fn delete_if_token(server: Uuid, token: &str) -> Result<(), StoreError> {
    match read(server)? {
        (Some(stored), _) if stored.token == token => backend::remove(server),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "abs_abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNO_-";

    fn stored(name: &str, token: &str) -> String {
        format!(r#"{{"name":"{name}","token":"{token}"}}"#)
    }

    #[test]
    fn checks_the_shape_of_a_token() {
        let cases: [(&str, String, Result<(), ShapeError>); 9] = [
            ("valid", TOKEN.to_owned(), Ok(())),
            (
                "42 characters",
                TOKEN[..TOKEN.len() - 1].to_owned(),
                Err(ShapeError::Token),
            ),
            ("44 characters", format!("{TOKEN}a"), Err(ShapeError::Token)),
            (
                "wrong prefix",
                TOKEN.replacen("abs_", "xyz_", 1),
                Err(ShapeError::Token),
            ),
            (
                "a semicolon",
                format!("{};", &TOKEN[..TOKEN.len() - 1]),
                Err(ShapeError::Token),
            ),
            (
                "a space",
                format!("{} ", &TOKEN[..TOKEN.len() - 1]),
                Err(ShapeError::Token),
            ),
            (
                "an equals sign",
                format!("{}=", &TOKEN[..TOKEN.len() - 1]),
                Err(ShapeError::Token),
            ),
            (
                "a set-cookie value",
                format!("{TOKEN}; Path=/; HttpOnly"),
                Err(ShapeError::Token),
            ),
            ("empty", String::new(), Err(ShapeError::Token)),
        ];

        for (name, token, expected) in cases {
            assert_eq!(check_token(&token), expected, "{name}");
        }
    }

    #[test]
    fn parses_a_stored_value() {
        let cases: Vec<(&str, String, Result<StoredSession, ShapeError>)> = vec![
            (
                "host cookie",
                stored("__Host-board-session", TOKEN),
                Ok(StoredSession {
                    name: CookieName::Host,
                    token: TOKEN.to_owned(),
                }),
            ),
            (
                "plain cookie",
                stored("board-session", TOKEN),
                Ok(StoredSession {
                    name: CookieName::Plain,
                    token: TOKEN.to_owned(),
                }),
            ),
            (
                "unknown name",
                stored("session", TOKEN),
                Err(ShapeError::Name),
            ),
            (
                "bad token",
                stored("board-session", "abs_short"),
                Err(ShapeError::Token),
            ),
            (
                "an extra field",
                format!(r#"{{"name":"board-session","token":"{TOKEN}","extra":1}}"#),
                Err(ShapeError::NotJson),
            ),
            ("not json", "abs_token".to_owned(), Err(ShapeError::NotJson)),
        ];

        for (name, value, expected) in cases {
            assert_eq!(parse_stored(&value), expected, "{name}");
        }
    }

    #[test]
    fn keeps_the_token_out_of_the_debug_output() {
        let session = StoredSession {
            name: CookieName::Plain,
            token: TOKEN.to_owned(),
        };

        assert!(!format!("{session:?}").contains(TOKEN));
    }

    #[test]
    fn names_the_cookie_as_the_board_does() {
        assert_eq!(CookieName::Host.as_str(), "__Host-board-session");
        assert_eq!(CookieName::Plain.as_str(), "board-session");
    }
}
