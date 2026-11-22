use tauri::{LogicalPosition, LogicalSize, Rect};

pub const TAB_BAR_HEIGHT: f64 = 36.0;
const TAB_BAR_PREFIX: &str = "tabbar-";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    TabBar,
    Content,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub fn slot_of(webview_label: &str) -> Slot {
    if webview_label.starts_with(TAB_BAR_PREFIX) {
        Slot::TabBar
    } else {
        Slot::Content
    }
}

pub fn bounds(slot: Slot, width: f64, height: f64) -> Bounds {
    let bar = TAB_BAR_HEIGHT.min(height).max(0.0);
    match slot {
        Slot::TabBar => Bounds {
            x: 0.0,
            y: 0.0,
            width,
            height: bar,
        },
        Slot::Content => Bounds {
            x: 0.0,
            y: bar,
            width,
            height: (height - bar).max(0.0),
        },
    }
}

impl From<Bounds> for Rect {
    fn from(bounds: Bounds) -> Self {
        Rect {
            position: LogicalPosition::new(bounds.x, bounds.y).into(),
            size: LogicalSize::new(bounds.width, bounds.height).into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_the_bar_above_the_content() {
        let cases = [
            (
                Slot::TabBar,
                1280.0,
                860.0,
                Bounds {
                    x: 0.0,
                    y: 0.0,
                    width: 1280.0,
                    height: 36.0,
                },
            ),
            (
                Slot::Content,
                1280.0,
                860.0,
                Bounds {
                    x: 0.0,
                    y: 36.0,
                    width: 1280.0,
                    height: 824.0,
                },
            ),
            (
                Slot::TabBar,
                400.0,
                20.0,
                Bounds {
                    x: 0.0,
                    y: 0.0,
                    width: 400.0,
                    height: 20.0,
                },
            ),
            (
                Slot::Content,
                400.0,
                20.0,
                Bounds {
                    x: 0.0,
                    y: 20.0,
                    width: 400.0,
                    height: 0.0,
                },
            ),
        ];

        for (slot, width, height, expected) in cases {
            assert_eq!(
                bounds(slot, width, height),
                expected,
                "{slot:?} in {width}x{height}"
            );
        }
    }

    #[test]
    fn tells_the_bar_from_a_board_by_label() {
        assert_eq!(slot_of("tabbar-0b0c"), Slot::TabBar);
        assert_eq!(slot_of("board-0b0c-home"), Slot::Content);
        assert_eq!(slot_of("board-0b0c-3"), Slot::Content);
    }
}
