//! Live clan and attendance checks against Hercules TEST. Same networking
//! calls as the client match arms (`ToggleAttendanceWindow`, clan chat).

use std::time::{Duration, Instant};

use super::LogicSession;

/// What TEST answered to `CZ_OPEN_UI` type 5.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttendanceReply {
    pub days_attended: u32,
    pub claimed_today: bool,
}

impl LogicSession {
    /// Log in, enter the map, ask for the attendance calendar and wait for
    /// `ZC_UI_OPEN` type 7. `Ok(None)` when TEST stays silent (attendance off
    /// or outside its period).
    pub fn request_attendance_live(&mut self) -> Result<Option<AttendanceReply>, String> {
        self.login_to_character_select()?;
        self.enter_world()?;
        {
            let Some(net) = self.net.as_mut() else {
                return Err("logic mode has no network".to_owned());
            };
            net.request_attendance_ui().map_err(|_| "map server is not connected".to_owned())?;
        }
        self.note("RequestAttendanceUi".to_owned());
        let deadline = Instant::now() + Duration::from_secs(6);
        while Instant::now() < deadline {
            let drained = {
                let (Some(net), Some(events)) = (self.net.as_mut(), self.events.as_mut()) else {
                    return Err("logic mode has no network".to_owned());
                };
                net.get_events(events);
                events.drain().collect::<Vec<_>>()
            };
            for event in drained {
                if let korangar_networking::NetworkEvent::AttendanceOpened {
                    days_attended,
                    claimed_today,
                } = event
                {
                    return Ok(Some(AttendanceReply {
                        days_attended,
                        claimed_today,
                    }));
                }
                self.on_event(event)?;
            }
            self.take_failure()?;
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(None)
    }
}
