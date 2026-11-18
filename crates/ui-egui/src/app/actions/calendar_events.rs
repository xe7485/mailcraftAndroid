//! Calendar events handed to or saved for the platform's other apps.

use crate::*;

impl MailApp {
    fn calendar_event(&self, uid: &str) -> Option<mailcraft_engine::model::ical::CalEvent> {
        self.session.find_event(uid).map(|(_, stored)| stored.event.clone())
    }

    fn event_ical(&self, event: &mailcraft_engine::model::ical::CalEvent) -> String {
        mailcraft_engine::model::ical::write(std::slice::from_ref(event), None, self.session.clock.now)
    }

    pub(super) fn share_calendar_event(&mut self, uid: &str) {
        if !self.services.can_share_calendar_events() {
            return;
        }
        let Some(event) = self.calendar_event(uid) else { return };
        if let Err(error) = self.services.share_calendar_event(&event.summary, &self.event_ical(&event)) {
            self.session.notice(error, true);
        }
    }

    pub(super) fn export_calendar_event(&mut self, uid: &str) {
        if !self.services.can_export_calendar_events() {
            return;
        }
        let Some(event) = self.calendar_event(uid) else { return };
        let name = format!("{}.ics", mailcraft_engine::mime::sanitize_filename(&event.summary));
        self.services.save_as_owned(&name, "text/calendar", self.event_ical(&event).into_bytes());
    }

    pub(super) fn add_event_to_system_calendar(&mut self, uid: &str) {
        if !self.services.can_add_to_system_calendar() {
            return;
        }
        let Some(event) = self.calendar_event(uid) else { return };
        let attendees: Vec<String> = event.attendees.iter().map(|attendee| attendee.email.clone()).collect();
        if let Err(error) = self.services.add_to_system_calendar(
            &event.summary,
            &event.location,
            &event.description,
            event.start,
            event.end,
            event.all_day,
            (!event.tzid.is_empty()).then_some(event.tzid.as_str()),
            event.rrule.as_deref(),
            &event.rdates,
            &event.exdates,
            &attendees,
        ) {
            self.session.notice(error, true);
        }
    }
}
