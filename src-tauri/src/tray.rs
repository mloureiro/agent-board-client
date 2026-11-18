use tauri::image::Image;
use tauri::menu::MenuBuilder;
use tauri::tray::TrayIconBuilder;
use tauri::AppHandle;

use crate::list_window;

const SHOW_SERVERS: &str = "show-servers";
const QUIT: &str = "quit";

fn icon() -> Image<'static> {
    const SIZE: u32 = 36;
    const TOP: u32 = 6;
    const COLUMN_WIDTH: u32 = 7;
    const COLUMNS: [(u32, u32); 3] = [(5, 30), (15, 20), (24, 25)];

    let mut rgba = vec![0_u8; (SIZE * SIZE * 4) as usize];
    for (left, bottom) in COLUMNS {
        for y in TOP..bottom {
            for x in left..left + COLUMN_WIDTH {
                let alpha = ((y * SIZE + x) * 4 + 3) as usize;
                rgba[alpha] = u8::MAX;
            }
        }
    }
    Image::new_owned(rgba, SIZE, SIZE)
}

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let menu = MenuBuilder::new(app)
        .text(SHOW_SERVERS, "Servers")
        .separator()
        .text(QUIT, "Quit Agent Board")
        .build()?;

    TrayIconBuilder::with_id("agent-board")
        .icon(icon())
        .icon_as_template(true)
        .tooltip("Agent Board")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            QUIT => app.exit(0),
            SHOW_SERVERS => {
                if let Err(error) = list_window::show(app) {
                    eprintln!("could not show the server list: {error}");
                }
            }
            _ => {}
        })
        .build(app)?;
    Ok(())
}
