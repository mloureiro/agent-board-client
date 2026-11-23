use serde::{Deserialize, Serialize};

const LEVELS: [u16; 17] = [
    25, 33, 50, 67, 75, 80, 90, 100, 110, 125, 150, 175, 200, 250, 300, 400, 500,
];
const ACTUAL_SIZE: u16 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomStep {
    In,
    Out,
    Reset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Zoom(u16);

impl Default for Zoom {
    fn default() -> Self {
        Self(ACTUAL_SIZE)
    }
}

impl Zoom {
    pub fn restored(self) -> Self {
        if LEVELS.contains(&self.0) {
            self
        } else {
            Self::default()
        }
    }

    pub fn step(self, step: ZoomStep) -> Self {
        let level = match step {
            ZoomStep::In => LEVELS.iter().copied().find(|level| *level > self.0),
            ZoomStep::Out => LEVELS.iter().rev().copied().find(|level| *level < self.0),
            ZoomStep::Reset => Some(ACTUAL_SIZE),
        };
        level.map_or(self, Self)
    }

    pub fn factor(self) -> f64 {
        f64::from(self.0) / 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_through_the_levels_and_stops_at_each_end() {
        let cases = [
            (100, ZoomStep::In, 110),
            (100, ZoomStep::Out, 90),
            (33, ZoomStep::Out, 25),
            (25, ZoomStep::Out, 25),
            (400, ZoomStep::In, 500),
            (500, ZoomStep::In, 500),
            (250, ZoomStep::Reset, 100),
            (25, ZoomStep::Reset, 100),
        ];
        for (from, step, expected) in cases {
            assert_eq!(Zoom(from).step(step), Zoom(expected), "{from} {step:?}");
        }
    }

    #[test]
    fn restores_only_a_known_level() {
        assert_eq!(Zoom(150).restored(), Zoom(150));
        assert_eq!(Zoom(0).restored(), Zoom(100));
        assert_eq!(Zoom(101).restored(), Zoom(100));
        assert_eq!(Zoom(u16::MAX).restored(), Zoom(100));
    }

    #[test]
    fn reads_a_missing_level_as_actual_size() {
        #[derive(Deserialize)]
        struct Saved {
            #[serde(default)]
            zoom: Zoom,
        }
        let saved: Saved = serde_json::from_str("{}").unwrap();
        assert_eq!(saved.zoom, Zoom(100));
        assert_eq!(saved.zoom.factor(), 1.0);
    }
}
