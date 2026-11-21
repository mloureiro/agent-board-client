use serde::Serialize;

use crate::connection::ConnectionFailure;
use crate::server_url::ServerUrlError;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ClientError {
    Name,
    Url { reason: ServerUrlError },
    NotFound,
    Storage { message: String },
    Window { message: String },
    Connection { reason: ConnectionFailure },
}

impl From<ServerUrlError> for ClientError {
    fn from(reason: ServerUrlError) -> Self {
        Self::Url { reason }
    }
}

impl From<ConnectionFailure> for ClientError {
    fn from(reason: ConnectionFailure) -> Self {
        Self::Connection { reason }
    }
}

impl From<std::io::Error> for ClientError {
    fn from(error: std::io::Error) -> Self {
        Self::Storage {
            message: error.to_string(),
        }
    }
}

impl From<tauri::Error> for ClientError {
    fn from(error: tauri::Error) -> Self {
        Self::Window {
            message: error.to_string(),
        }
    }
}
