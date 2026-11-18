//! Entry points shared with mobile hosts and Android system integrations.

use super::*;

impl MailApp {
    /// Let the network backend flush recent offline edits before the platform suspends us.
    pub fn flush_changes(&mut self, timeout: std::time::Duration) -> bool {
        self.backend.flush_changes(timeout)
    }

    /// Save open drafts to the local cache before the app is suspended.
    pub fn checkpoint_drafts(&mut self) -> usize {
        let kept = self.session.keep_drafts();
        self.flush_jobs();
        kept
    }

    /// Save an edited note before the app is suspended or closed.
    pub fn checkpoint_note_edit(&mut self) -> bool {
        let Some((uid, text)) = self.ui.note_edit.take() else { return false };
        match self.session.run("note.update", &json!({"uid": uid, "text": text})) {
            Ok(_) => {
                self.flush_jobs();
                true
            }
            Err(error) => {
                self.ui.note_edit = Some((uid, text));
                self.session.notice(error.to_string(), true);
                false
            }
        }
    }

    /// Queue a full sync for every account.
    pub fn sync_now(&mut self) -> usize {
        let accounts = self.session.sync_all(None);
        self.flush_jobs();
        accounts
    }

    /// Shut down network workers and seal the local cache.
    pub fn shutdown(&mut self) -> Result<(), String> {
        self.backend.shutdown();
        if let Some(store) = &self.session.store {
            store.seal_all().map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    /// Follow a notification to the cached message, calendar event or task.
    pub fn open_notification_target(&mut self, target: NotificationTarget) -> bool {
        match target {
            NotificationTarget::Inbox => {
                self.run("view.module", json!({"name": "mail"}));
                self.run("folder.unified", json!({"role": "inbox"}));
                true
            }
            NotificationTarget::Message { key } => {
                let Some(key) = MessageKey::from_id(&key) else {
                    self.run("view.module", json!({"name": "mail"}));
                    self.run("folder.unified", json!({"role": "inbox"}));
                    return false;
                };
                self.run("folder.open", json!({"account": key.account, "folder": key.folder}));
                if self.session.summary(&key).is_none() {
                    self.run("view.module", json!({"name": "mail"}));
                    self.run("folder.unified", json!({"role": "inbox"}));
                    return false;
                }
                self.run("list.open", json!({"key": key.to_id()}));
                true
            }
            NotificationTarget::Reminder { account: _, uid, occurrence, due: _, task } => {
                let dialog = if task { Dialog::open_task(&self.session, &uid) } else { Dialog::open_event(&self.session, &uid, occurrence) };
                if let Some(dialog) = dialog {
                    self.ui.dialog = Some(dialog);
                    true
                } else {
                    self.run("view.module", json!({"name": if task { "tasks" } else { "calendar" }}));
                    false
                }
            }
            NotificationTarget::Calendar => {
                self.run("view.calendar", json!({}));
                true
            }
            NotificationTarget::Tasks => {
                self.run("view.module", json!({"name": "tasks"}));
                true
            }
        }
    }

    /// A document selected by Android after a picker request or share-sheet fallback.
    pub fn open_handed_from_picker(&mut self, what: &str) {
        let path = std::path::Path::new(what);
        let extension = path.extension().and_then(std::ffi::OsStr::to_str).unwrap_or_default().to_ascii_lowercase();
        let data_file = matches!(extension.as_str(), "pst" | "mailcraft");
        let outlook_message_file = extension == "msg";
        if outlook_message_file {
            self.services.track_picked_file(path);
        }
        if data_file {
            self.services.track_picked_file(path);
            let path = path.to_string_lossy().trim().to_string();
            match self.session.run("account.inspectDataFile", &json!({"path": path, "again": true})) {
                Ok(_) => {
                    self.ui.backstage = None;
                    self.ui.dialog = Some(Dialog::OpenDataFile { path });
                }
                Err(error) => self.session.notice(tf!("Couldn't open {file}: {error}", file = path, error = error.to_string()), true),
            }
        } else {
            self.open_handed(what);
        }
        if !data_file && !outlook_message_file {
            self.services.discard_picked_file(path);
        }
    }

    /// Route a shared document to an Android picker action that is waiting for a result.
    pub fn receive_shared_picker_file(&mut self, path: &std::path::Path, last_in_batch: bool) -> Result<bool, String> {
        self.services.receive_shared_picker_file(path, last_in_batch)
    }

    /// Attach a private staged copy shared from another app.
    pub fn open_shared_attachment_from_picker(&mut self, path: &str) {
        let file = std::path::Path::new(path);
        if self.session.compose.is_none() {
            self.run("mail.new", json!({}));
        }
        if self.session.compose.is_none() {
            self.session.notice(tl!("Add an account first: a new message needs an account to send from.").to_string(), true);
        } else {
            match self.session.run("compose.attach", &json!({"path": path})) {
                Ok(_) => self.ui.backstage = None,
                Err(error) => self.session.notice(error.to_string(), true),
            }
        }
        self.services.discard_picked_file(file);
    }

    /// Turn text shared from another app into a new draft.
    pub fn open_shared_text(&mut self, subject: &str, body: &str, to: &str, cc: &str, bcc: &str) {
        let subject: String = subject.chars().map(|c| if c.is_control() { ' ' } else { c }).take(998).collect();
        let body: String = body.chars().take(1 << 20).collect();
        let bound = |text: &str| text.chars().take(16_384).collect::<String>();
        self.run("mail.new", json!({}));
        if self.session.compose.is_some() {
            if let Err(error) =
                self.session.run("compose.update", &json!({"subject": subject, "body": body, "to": bound(to), "cc": bound(cc), "bcc": bound(bcc)}))
            {
                self.session.notice(error.to_string(), true);
            } else {
                self.ui.backstage = None;
            }
        }
    }
}
