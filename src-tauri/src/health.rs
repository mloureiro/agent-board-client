use serde::Serialize;

use crate::sentinel::HealthReport;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Banner {
    Unreachable,
    SessionExpired,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Health {
    seen_ok: bool,
    banner: Option<Banner>,
}

impl Health {
    pub fn report(&mut self, report: HealthReport) -> bool {
        let before = self.banner;
        self.banner = match report {
            HealthReport::Ok => {
                self.seen_ok = true;
                None
            }
            HealthReport::Down => Some(Banner::Unreachable),
            HealthReport::Session if self.seen_ok => Some(Banner::SessionExpired),
            HealthReport::Session => None,
        };
        self.banner != before
    }

    pub fn banner(&self) -> Option<Banner> {
        self.banner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use HealthReport as R;

    #[test]
    fn follows_the_reports_of_a_window() {
        let cases: [(&str, &[HealthReport], Option<Banner>, bool); 8] = [
            ("session before any ok", &[R::Session], None, false),
            (
                "ok then session",
                &[R::Ok, R::Session],
                Some(Banner::SessionExpired),
                true,
            ),
            ("down then ok", &[R::Down, R::Ok], None, true),
            (
                "repeated down",
                &[R::Down, R::Down],
                Some(Banner::Unreachable),
                false,
            ),
            (
                "down before any ok",
                &[R::Down],
                Some(Banner::Unreachable),
                true,
            ),
            (
                "down then session before any ok",
                &[R::Down, R::Session],
                None,
                true,
            ),
            ("session then ok", &[R::Ok, R::Session, R::Ok], None, true),
            ("repeated ok", &[R::Ok, R::Ok], None, false),
        ];

        for (name, reports, banner, last_changed) in cases {
            let mut health = Health::default();
            let mut changed = false;
            for report in reports {
                changed = health.report(*report);
            }
            assert_eq!(health.banner(), banner, "{name}");
            assert_eq!(changed, last_changed, "{name}");
        }
    }

    #[test]
    fn serialises_a_banner_in_camel_case() {
        assert_eq!(
            serde_json::to_string(&Banner::SessionExpired).unwrap(),
            "\"sessionExpired\""
        );
        assert_eq!(
            serde_json::to_string(&Banner::Unreachable).unwrap(),
            "\"unreachable\""
        );
    }
}
