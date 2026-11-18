//! The open message and its attachments (and a note's files) to files: printed, saved where
//! asked, handed to another app; in the browser, downloaded.

use crate::*;

const MAX_SHARE_ATTACHMENTS: usize = 50;
const MAX_SHARE_ATTACHMENT_BYTES: usize = 25 << 20;

impl MailApp {
    /// The open message's original source handed to the system share sheet.
    pub(super) fn share_message(&mut self) {
        if !self.services.can_share_files() {
            return;
        }
        let name = format!("{}.eml", self.message_file_name());
        match self.session.message_source() {
            Ok(bytes) => {
                if let Err(error) = self.services.share_message(&name, "message/rfc822", &bytes) {
                    self.session.notice(error, true);
                }
            }
            Err(error) => self.session.notice(error.to_string(), true),
        }
    }

    /// One or more open message attachments handed to the system share sheet.
    pub(super) fn share_attachments(&mut self, indices: Vec<usize>) {
        if !self.services.can_share_files() || indices.is_empty() {
            return;
        }
        if indices.len() > MAX_SHARE_ATTACHMENTS {
            self.session.notice(tf!("You can share up to {n} attachments at once", n = MAX_SHARE_ATTACHMENTS), true);
            return;
        }
        let Some(parsed) = self.session.reading.as_ref().and_then(|reading| reading.parsed.as_ref()) else { return };
        let mut metadata = Vec::with_capacity(indices.len());
        let mut total_size = 0usize;
        for index in indices {
            let Some(info) = parsed.attachments.iter().find(|attachment| attachment.index == index) else { return };
            let size = usize::try_from(info.size).unwrap_or(usize::MAX);
            total_size = total_size.saturating_add(size);
            metadata.push((index, info.name.clone(), info.mime.clone()));
        }
        if total_size > MAX_SHARE_ATTACHMENT_BYTES {
            self.session.notice(tl!("The selected attachments are too large to share"), true);
            return;
        }
        let mut owned = Vec::with_capacity(metadata.len());
        let mut actual_size = 0usize;
        for (index, name, mime) in metadata {
            match self.session.attachment_bytes(index) {
                Ok(bytes) => {
                    actual_size = actual_size.saturating_add(bytes.len());
                    if actual_size > MAX_SHARE_ATTACHMENT_BYTES {
                        self.session.notice(tl!("The selected attachments are too large to share"), true);
                        return;
                    }
                    owned.push((name, mime, bytes));
                }
                Err(error) => {
                    self.session.notice(error.to_string(), true);
                    return;
                }
            }
        }
        let files: Vec<(&str, &str, &[u8])> = owned.iter().map(|(name, mime, bytes)| (name.as_str(), mime.as_str(), bytes.as_slice())).collect();
        let result = match files.as_slice() {
            [(name, mime, bytes)] => self.services.share_file(name, mime, bytes),
            _ => self.services.share_files(&files),
        };
        if let Err(error) = result {
            self.session.notice(error, true);
        }
    }

    /// The message as a PDF, saved where asked, then opened to print from the viewer (in the
    /// browser, downloaded).
    pub(super) fn print(&mut self) {
        if self.services.can_save_as() {
            let name = format!("{}.pdf", self.message_file_name());
            match self.session.message_pdf() {
                Ok(bytes) => self.services.open_file_owned_with_mime(&name, "application/pdf", bytes),
                Err(e) => self.session.notice(e.to_string(), true),
            }
            return;
        }
        if self.services.downloads() {
            self.download_message(true);
            return;
        }
        // Shown as it will print before it prints (issue #97): straight into the PDF viewer, its
        // pages there to look at and print. Not asked where to save it first (that's Save as
        // PDF): a copy of MailCraft's own, in its scratch folder.
        let name = mailcraft_engine::mime::sanitize_filename(&format!("{}.pdf", self.message_file_name()));
        let Some(dir) = crate::scratch_dir("print") else {
            self.session.notice(tl!("There's no folder to put the PDF in for the viewer"), true);
            return;
        };
        let path = dir.join(name);
        match self.session.run("message.exportPdf", &json!({"path": path.to_string_lossy()})) {
            Ok(_) => match self.services.open_path(&path) {
                Ok(()) => self.session.notice(tl!("Opened as a PDF: print it from the viewer"), false),
                Err(e) => self.session.notice(tf!("The PDF couldn't be opened: {error}", error = e), true),
            },
            Err(e) => self.session.notice(e.to_string(), true),
        }
    }

    /// The message saved where asked, as a PDF or as its `.eml` source (in the browser,
    /// downloaded).
    pub(super) fn save_message(&mut self, pdf: bool) {
        if self.services.can_save_as() {
            let (name, mime, bytes) = if pdf {
                (format!("{}.pdf", self.message_file_name()), "application/pdf", self.session.message_pdf())
            } else {
                (format!("{}.eml", self.message_file_name()), "message/rfc822", self.session.message_source())
            };
            match bytes {
                Ok(bytes) => self.services.save_as_owned(&name, mime, bytes),
                Err(e) => self.session.notice(e.to_string(), true),
            }
            return;
        }
        if self.services.downloads() {
            self.download_message(pdf);
            return;
        }
        let (ext, id) = if pdf { ("pdf", "message.exportPdf") } else { ("eml", "message.saveAs") };
        if let Some(path) = self.services.save_path(&format!("{}.{ext}", self.message_file_name())) {
            match self.session.run(id, &json!({"path": path.to_string_lossy()})) {
                Ok(_) => self.session.notice(tf!("Saved {path}", path = path.display()), false),
                Err(e) => self.session.notice(e.to_string(), true),
            }
        }
    }

    /// The message downloaded (the browser's), as a PDF or as its `.eml` source.
    fn download_message(&mut self, pdf: bool) {
        let (name, bytes) = if pdf {
            (format!("{}.pdf", self.message_file_name()), self.session.message_pdf())
        } else {
            (format!("{}.eml", self.message_file_name()), self.session.message_source())
        };
        match bytes {
            Ok(b) => self.services.download(&name, &b),
            Err(e) => self.session.notice(e.to_string(), true),
        }
    }

    pub(super) fn open_in_craft(&mut self, i: usize, cmd: &str) {
        let Some(app) = self.ui.craft_apps.iter().find(|(c, _)| *c == cmd).map(|(_, p)| p.clone()) else { return };
        if let Some(path) = self.attachment_to_temp(i, true)
            && let Err(e) = self.services.open_with(&app, &path)
        {
            self.session.notice(tf!("{app} couldn't open it: {error}", app = app.display(), error = e), true);
        }
    }

    /// The attachments `v` dragged out together (those selected, or the one dragged), each as a
    /// file for the system's drag and drop. Two of the same name keep it, the second in a folder
    /// of its own (`open/2`), rather than one written over the other.
    pub(super) fn drag_out(&mut self, v: Vec<usize>) {
        if !self.services.can_drag_out() {
            return;
        }
        let mut names: Vec<String> = Vec::new();
        let mut files = Vec::new();
        for i in v {
            let name = mailcraft_engine::mime::sanitize_filename(&self.attachment_name(i)).to_lowercase();
            let before = names.iter().filter(|n| **n == name).count();
            names.push(name);
            if let Some(path) = self.attachment_to_temp_in(i, false, (before > 0).then_some(before.saturating_add(1))) {
                files.push(path);
            }
        }
        self.ui.drag_out = files;
    }

    /// An attachment downloaded (the browser's, to save or open it).
    fn download_attachment(&mut self, i: usize) {
        let name = mailcraft_engine::mime::sanitize_filename(&self.attachment_name(i));
        match self.session.attachment_bytes(i) {
            Ok(b) => self.services.download(&name, &b),
            Err(e) => self.session.notice(e.to_string(), true),
        }
    }

    pub(super) fn save_attachment(&mut self, i: usize) {
        if self.services.can_save_as() {
            let name = mailcraft_engine::mime::sanitize_filename(&self.attachment_name(i));
            match self.session.attachment_bytes(i) {
                Ok(bytes) => {
                    let mime = mailcraft_engine::mime::mime_from_name(&name);
                    self.services.save_as_owned(&name, &mime, bytes);
                }
                Err(e) => self.session.notice(e.to_string(), true),
            }
            return;
        }
        if self.services.downloads() {
            self.download_attachment(i);
            return;
        }
        let name = self.attachment_name(i);
        if let Some(path) = self.services.save_path(&name) {
            self.run("attachment.save", json!({"index": i, "path": path.to_string_lossy()}));
        }
    }

    /// A picture of an open message saved as a file (issue #110); in the browser, downloaded.
    pub(super) fn save_picture(&mut self, key: &MessageKey, src: &str) {
        let name = self.session.picture_name(key, src);
        if self.services.can_save_as() {
            match self.session.picture_bytes(key, src) {
                Some(bytes) => {
                    let mime = mailcraft_engine::mime::mime_from_name(&name);
                    self.services.save_as_owned(&name, &mime, bytes);
                }
                None => self.session.notice(tl!("The picture couldn't be saved").to_string(), true),
            }
            return;
        }
        if self.services.downloads() {
            match self.session.picture_bytes(key, src) {
                Some(b) => self.services.download(&name, &b),
                None => self.session.notice(tl!("The picture couldn't be saved").to_string(), true),
            }
            return;
        }
        if let Some(path) = self.services.save_path(&name) {
            self.run("picture.save", json!({"key": key.to_id(), "src": src, "path": path.to_string_lossy()}));
        }
    }

    pub(super) fn open_attachment(&mut self, i: usize) {
        if self.services.downloads() && !self.services.opens_files_after_saving() {
            self.download_attachment(i);
            return;
        }
        let name = self.attachment_name(i);
        if let Some(path) = self.attachment_to_temp(i, true)
            && let Err(e) = self.services.open_path(&path)
        {
            self.session.notice(tf!("{name} could not be opened: {error}", name = name, error = e), true);
        }
    }

    /// Every attachment (`None`), or some (by index), saved into a folder picked; in the
    /// browser, every one downloaded.
    pub(super) fn save_attachments(&mut self, some: Option<Vec<usize>>) {
        if self.services.downloads() {
            let files: Vec<(usize, String)> = self
                .session
                .reading
                .as_ref()
                .and_then(|r| r.parsed.as_ref())
                .map(|p| {
                    p.attachments
                        .iter()
                        .filter(|attachment| !attachment.inline && some.as_ref().is_none_or(|indices| indices.contains(&attachment.index)))
                        .map(|attachment| (attachment.index, attachment.name.clone()))
                        .collect()
                })
                .unwrap_or_default();
            // One that can't be had is said, not skipped unseen (the rest looked like all).
            for (i, name) in files {
                match self.session.attachment_bytes(i) {
                    Ok(b) => self.services.download(&mailcraft_engine::mime::sanitize_filename(&name), &b),
                    Err(e) => self.session.notice(tf!("{name} couldn't be downloaded: {error}", name = name, error = e), true),
                }
            }
            return;
        }
        let indices = match &some {
            Some(v) => json!(v),
            None => Value::Null,
        };
        if let Some(dir) = self.services.pick_folder() {
            match self.session.run("attachment.saveAll", &json!({"dir": dir.to_string_lossy(), "indices": indices})) {
                Ok(v) => {
                    let n = v.get("saved").and_then(Value::as_array).map_or(0, Vec::len);
                    self.session.notice(tf!("Saved {n} attachments in {folder}", n = n, folder = dir.display()), false);
                }
                Err(e) => self.session.notice(e.to_string(), true),
            }
        }
    }

    /// A file name for the open message (its subject, made safe).
    pub(crate) fn message_file_name(&self) -> String {
        let subject = self.session.reading.as_ref().and_then(|r| r.parsed.as_ref()).map(|p| p.subject.clone()).unwrap_or_default();
        let name = mailcraft_engine::mime::sanitize_filename(if subject.trim().is_empty() { "message" } else { &subject });
        name.chars().take(80).collect()
    }

    /// Write an attachment to a temporary file to hand to another app: dragged out (a copy, as
    /// saving), or to be opened (never a program, script or shortcut: the engine refuses).
    pub(crate) fn attachment_to_temp(&mut self, i: usize, open: bool) -> Option<PathBuf> {
        self.attachment_to_temp_in(i, open, None)
    }

    /// [`Self::attachment_to_temp`], in the numbered folder `sub` of the temporary one when given.
    fn attachment_to_temp_in(&mut self, i: usize, open: bool, sub: Option<usize>) -> Option<PathBuf> {
        if !self.ui.has_files {
            return None;
        }
        let name = self.attachment_name(i);
        let dir = crate::scratch_dir("open").and_then(|d| match sub {
            Some(n) => std::fs::create_dir_all(d.join(n.to_string())).ok().map(|()| d.join(n.to_string())),
            None => Some(d),
        });
        let Some(dir) = dir else {
            self.session.notice(tl!("There's no folder to put the attachment in for another app"), true);
            return None;
        };
        let path = dir.join(mailcraft_engine::mime::sanitize_filename(&name));
        match self.session.run("attachment.save", &json!({"index": i, "path": path.to_string_lossy(), "open": open})) {
            Ok(_) => Some(path),
            Err(e) => {
                self.session.notice(e.to_string(), true);
                None
            }
        }
    }

    /// A note's file `index`, opened in another app (saved for it first, never when it could run
    /// a program) or saved where asked; in the browser, downloaded.
    pub(super) fn note_file(&mut self, uid: &str, index: usize, open: bool) {
        if self.services.can_save_as() && !open {
            match self.session.note_file(uid, index) {
                Ok((name, bytes)) => {
                    let mime = mailcraft_engine::mime::mime_from_name(&name);
                    self.services.save_as_owned(&name, &mime, bytes);
                }
                Err(e) => self.session.notice(e.to_string(), true),
            }
            return;
        }
        if self.services.downloads() && !self.services.opens_files_after_saving() {
            match self.session.note_file(uid, index) {
                Ok((name, b)) => self.services.download(&name, &b),
                Err(e) => self.session.notice(e.to_string(), true),
            }
            return;
        }
        let name = self
            .session
            .notes("")
            .iter()
            .find(|n| n.note.uid == uid)
            .and_then(|n| n.note.kept_files().get(index).map(|f| mailcraft_engine::mime::sanitize_filename(f.file_name())))
            .unwrap_or_else(|| "attachment".into());
        let path = if open {
            let Some(dir) = crate::scratch_dir("open") else {
                self.session.notice(tl!("There's no folder to put the attachment in for another app"), true);
                return;
            };
            dir.join(&name)
        } else {
            let Some(path) = self.services.save_path(&name) else { return };
            path
        };
        match self.session.run("note.saveFile", &json!({"uid": uid, "index": index, "path": path.to_string_lossy(), "open": open})) {
            Ok(_) if open => {
                if let Err(e) = self.services.open_path(&path) {
                    self.session.notice(tf!("{name} could not be opened: {error}", name = name, error = e), true);
                }
            }
            Ok(_) => self.session.notice(tf!("Saved {path}", path = path.display()), false),
            Err(e) => self.session.notice(e.to_string(), true),
        }
    }

    pub(crate) fn attachment_name(&self, i: usize) -> String {
        self.session
            .reading
            .as_ref()
            .and_then(|r| r.parsed.as_ref())
            .and_then(|p| p.attachments.iter().find(|a| a.index == i))
            .map(|a| a.name.clone())
            .unwrap_or_else(|| "attachment".into())
    }

    /// Every attachment of the message being read, selected (Ctrl+A in its attachments).
    pub(crate) fn select_all_attachments(&mut self) {
        let Some(r) = self.session.reading.as_ref() else { return };
        let all: Vec<usize> = r.parsed.as_ref().map(|p| p.attachments.iter().filter(|a| !a.inline).map(|a| a.index).collect()).unwrap_or_default();
        self.ui.attachment_selected = Some((r.key.clone(), all));
    }
}
