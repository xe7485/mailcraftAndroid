//! Address-book cards shared or exported to the platform's other apps.

use crate::*;

impl MailApp {
    fn contact_vcard(&self, uid: &str) -> Option<(String, String)> {
        let person = self.session.find_person(uid)?;
        let card = &person.card;
        Some((card.display(), mailcraft_engine::model::vcard::write(card)))
    }

    pub(super) fn share_contact(&mut self, uid: &str) {
        if !self.services.can_share_contacts() {
            return;
        }
        let Some((name, vcard)) = self.contact_vcard(uid) else { return };
        if let Err(error) = self.services.share_contact(&name, &vcard) {
            self.session.notice(error, true);
        }
    }

    pub(super) fn export_contact(&mut self, uid: &str) {
        if !self.services.can_export_contacts() {
            return;
        }
        let Some((name, vcard)) = self.contact_vcard(uid) else { return };
        let filename = format!("{}.vcf", mailcraft_engine::mime::sanitize_filename(&name));
        self.services.save_as_owned(&filename, "text/vcard", vcard.into_bytes());
    }

    pub(super) fn add_contact_to_system(&mut self, uid: &str) {
        if !self.services.can_add_to_system_contacts() {
            return;
        }
        let Some(person) = self.session.find_person(uid) else { return };
        if let Err(error) = self.services.add_to_system_contacts(&person.card) {
            self.session.notice(error, true);
        }
    }
}
