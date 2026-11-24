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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Zoom(u16);

impl Default for Zoom {
    fn default() -> Self {
        Self(ACTUAL_SIZE)
    }
}

impl Zoom {
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
}
