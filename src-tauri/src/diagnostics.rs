use std::fs::{self, OpenOptions};
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Manager};

const LOG_FILE: &str = "client.log";

// An app started by LaunchServices has its stderr discarded, so failures also go to a file.
pub fn record(app: &AppHandle, line: &str) {
    eprintln!("{line}");

    let Ok(directory) = app.path().app_log_dir() else {
        return;
    };
    if fs::create_dir_all(&directory).is_err() {
        return;
    }
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(directory.join(LOG_FILE))
    {
        let _ = writeln!(file, "{seconds} {line}");
    }
}
