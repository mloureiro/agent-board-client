use std::fs::{self, OpenOptions};
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Manager};

const LOG_FILE: &str = "client.log";

fn append(app: &AppHandle, line: &str) {
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

// An app started by LaunchServices has its stderr discarded, so failures also go to a file.
pub fn record(app: &AppHandle, line: &str) {
    eprintln!("{line}");
    append(app, line);
}

// The release profile aborts on panic, so the message must be written before the default hook returns.
pub fn record_panics(app: AppHandle) {
    let print = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        print(info);
        append(&app, &format!("panic: {info}"));
    }));
}
