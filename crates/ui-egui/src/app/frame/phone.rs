//! Single-column phone navigation: a compact app bar, bottom module bar and folder drawer.

use crate::*;

impl MailApp {
    pub(super) fn phone_chrome(&mut self, ui: &mut Ui, acts: &mut Vec<Action>) {
        let t = self.ui.tokens;
        let search = self.ui.phone_search_open && !self.session.accounts.is_empty();
        egui::Panel::top("phone-topbar")
            .exact_size(if search { 86.0 } else { 50.0 })
            .frame(egui::Frame::new().fill(t.bar).inner_margin(Margin::symmetric(8, 5)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if !self.session.accounts.is_empty() && widgets::bar_icon_button(ui, Icon::Folder, tl!("Folder pane"), &t).clicked() {
                        self.ui.phone_drawer_open = true;
                    }
                    let title = module_name(self.session.module);
                    ui.label(RichText::new(title).font(theme::bold(16.0)).color(t.on_bar));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if widgets::bar_icon_button(ui, Icon::Gear, "Accounts and settings", &t).clicked() {
                            acts.push(Action::Backstage(backstage::Page::Info));
                        }
                        if !self.session.accounts.is_empty() {
                            if self.session.registry().get("mail.sync").is_some_and(|command| (command.enabled)(&self.session).is_none())
                                && widgets::bar_icon_button(ui, Icon::Sync, "Send / Receive all folders (F9)", &t).clicked()
                            {
                                acts.push(Action::Run("mail.sync", json!({})));
                            }
                            if widgets::bar_icon_button(ui, Icon::Search, "Search", &t).clicked() {
                                self.ui.phone_search_open = !self.ui.phone_search_open;
                                self.ui.focus_search = self.ui.phone_search_open;
                            }
                        }
                    });
                });
                if search {
                    ui.add_space(3.0);
                    self.top_search(ui, ui.available_width().max(80.0), acts);
                }
            });

        if !self.session.accounts.is_empty() && self.ui.backstage.is_none() {
            egui::Panel::bottom("phone-modules")
                .exact_size(58.0)
                .frame(egui::Frame::new().fill(t.chrome).inner_margin(Margin::symmetric(4, 2)).stroke(egui::Stroke::new(1.0, t.border)))
                .show(ui, |ui| phone_modules(ui, &t, self.session.module, acts));
        }
    }

    pub(super) fn phone_panes(&mut self, ui: &mut Ui, acts: &mut Vec<Action>) {
        let t = self.ui.tokens;
        let module = self.session.module;
        let body = ui.max_rect();
        let detail = self.session.reading.is_some() || self.session.opened_file.is_some() || self.session.compose.is_some();

        match module {
            Module::Mail => {
                egui::CentralPanel::default().frame(egui::Frame::new().fill(t.surface)).show(ui, |ui| {
                    if detail {
                        reading::show(&mut self.session, &mut self.ui, ui, acts);
                    } else {
                        list::show(&self.session, &mut self.ui, ui, acts);
                    }
                });
            }
            Module::Calendar => {
                egui::CentralPanel::default().frame(egui::Frame::new().fill(t.surface)).show(ui, |ui| {
                    calendar::show(&self.session, &mut self.ui, ui, acts);
                });
            }
            Module::People => {
                egui::CentralPanel::default().frame(egui::Frame::new().fill(t.surface)).show(ui, |ui| {
                    people::show(&self.session, &mut self.ui, ui, acts);
                });
            }
            Module::Tasks => {
                egui::CentralPanel::default().frame(egui::Frame::new().fill(t.surface)).show(ui, |ui| {
                    tasks::show(&self.session, &mut self.ui, ui, acts);
                });
            }
            Module::Notes => {
                egui::CentralPanel::default().frame(egui::Frame::new().fill(t.surface)).show(ui, |ui| {
                    notes::show(&self.session, &mut self.ui, ui, acts);
                });
            }
        }

        if self.ui.phone_drawer_open {
            let mut close = false;
            egui::Area::new(egui::Id::unique("phone-navigation-drawer")).fixed_pos(body.min).order(egui::Order::Foreground).show(ui.ctx(), |ui| {
                let (full, response) = ui.allocate_exact_size(body.size(), Sense::click());
                ui.painter().rect_filled(full, 0.0, Color32::from_black_alpha(112));
                close |= response.clicked();
                let drawer_width = full.width().min(320.0);
                let drawer = egui::Rect::from_min_size(full.min, vec2(drawer_width, full.height()));
                ui.scope_builder(egui::UiBuilder::new().max_rect(drawer), |ui| {
                    egui::Frame::new().fill(t.surface).stroke(egui::Stroke::new(1.0, t.border)).inner_margin(Margin::symmetric(12, 8)).show(
                        ui,
                        |ui| {
                            ui.set_min_height(drawer.height());
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(module_name(module)).font(theme::bold(16.0)).color(t.text));
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    close |= widgets::tiny_icon_button(ui, Icon::Close, tl!("Close"), &t).clicked();
                                });
                            });
                            ui.separator();
                            egui::ScrollArea::vertical().show(ui, |ui| match module {
                                Module::Mail => nav::show(&self.session, &t, ui, acts),
                                Module::Calendar => calendar::nav(&self.session, &mut self.ui, ui, acts),
                                Module::People => people::nav(&self.session, &mut self.ui, ui, acts),
                                Module::Tasks => tasks::nav(&self.session, &mut self.ui, ui, acts),
                                Module::Notes => notes::nav(&self.session, &mut self.ui, ui, acts),
                            });
                        },
                    );
                });
            });
            let changed_page = acts.iter().any(
                |action| matches!(action, Action::Run(id, _) if matches!(*id, "folder.open" | "folder.unified" | "folder.outbox" | "view.module")),
            );
            if close || changed_page {
                self.ui.phone_drawer_open = false;
            }
        }
    }

    pub(in crate::app) fn phone_back(&mut self, ctx: &egui::Context) -> bool {
        if !self.ui.phone_layout || !ctx.input(|input| input.key_pressed(egui::Key::BrowserBack)) {
            return false;
        }
        if self.ui.dialog.is_some() {
            ctx.input_mut(|input| {
                input.events.push(egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                });
            });
            return false;
        }
        if self.ui.phone_drawer_open {
            self.ui.phone_drawer_open = false;
        } else if self.ui.backstage.is_some() {
            self.ui.backstage = None;
        } else if self.ui.phone_search_open {
            self.ui.phone_search_open = false;
            self.ui.focus_search = false;
        } else if self.session.compose.is_some() {
            self.run("compose.close", json!({}));
        } else if self.session.reading.is_some() {
            self.run("list.close", json!({}));
        } else {
            return false;
        }
        ctx.request_repaint();
        true
    }
}

fn module_name(module: Module) -> &'static str {
    match module {
        Module::Mail => tl!("Mail"),
        Module::Calendar => tl!("Calendar"),
        Module::People => tl!("People"),
        Module::Tasks => tl!("Tasks"),
        Module::Notes => tl!("Notes"),
    }
}

fn phone_modules(ui: &mut Ui, t: &Tokens, current: Module, acts: &mut Vec<Action>) {
    const MODULES: [(Module, Icon, &str, &str); 5] = [
        (Module::Mail, Icon::Unread, "Mail", "mail"),
        (Module::Calendar, Icon::Calendar, "Calendar", "calendar"),
        (Module::People, Icon::People, "People", "people"),
        (Module::Tasks, Icon::Task, "Tasks", "tasks"),
        (Module::Notes, Icon::Note, "Notes", "notes"),
    ];
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let width = ui.available_width() / MODULES.len() as f32;
        for (module, icon, label, code) in MODULES {
            let (rect, response) = ui.allocate_exact_size(vec2(width, 52.0), Sense::click());
            let selected = module == current;
            if selected || response.hovered() {
                ui.painter().rect_filled(rect.shrink2(vec2(2.0, 1.0)), 8.0, if selected { t.accent_soft } else { t.hover });
            }
            let color = if selected { t.accent } else { t.dim };
            icons::paint(ui.painter(), egui::Rect::from_center_size(egui::pos2(rect.center().x, rect.top() + 16.0), vec2(20.0, 20.0)), icon, color);
            let text = ui.painter().layout_no_wrap(tl!(label).to_owned(), theme::regular(10.5), color);
            ui.painter().galley(egui::pos2(rect.center().x - text.size().x / 2.0, rect.top() + 29.0), text, color);
            widgets::named(ui, &response, tl!(label), true, Some(selected));
            if response.clicked() && !selected {
                acts.push(Action::Run("view.module", json!({"name": code})));
            }
        }
    });
}
