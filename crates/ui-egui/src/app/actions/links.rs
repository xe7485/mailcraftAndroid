//! Phone numbers and addresses handed to the platform's other apps.

use crate::*;

impl MailApp {
    pub(super) fn dial_phone(&mut self, number: &str) {
        if !self.services.can_dial_phone() {
            return;
        }
        if let Err(error) = self.services.dial_phone(number) {
            self.session.notice(error, true);
        }
    }

    pub(super) fn open_map(&mut self, address: &str) {
        if !self.services.can_open_map() {
            return;
        }
        if let Err(error) = self.services.open_map(address) {
            self.session.notice(error, true);
        }
    }
}
