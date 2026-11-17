use serde::Serialize;

use crate::server_url::ServerUrlError;

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ClientError {
    Name,
    Url { reason: ServerUrlError },
    NotFound,
    Storage { message: String },
    Window { message: String },
}

impl From<ServerUrlError> for ClientError {
    fn from(reason: ServerUrlError) -> Self {
        Self::Url { reason }
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
