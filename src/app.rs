//! egui front-end: the whole UI and the scan/clean orchestration.

use std::sync::mpsc::{self, Receiver, TryRecvError};

use eframe::egui;

use crate::categories::{self, Category, Risk};
use crate::cleaner;
use crate::scanner::{self, Entry};
use crate::util::{format_size, short_path};

/// Hard cap on how many items we list under one expanded category, to keep the
/// UI responsive on enormous caches.
const PREVIEW_LIMIT: usize = 300;

/// What the app is currently doing.
enum Phase {
    Idle,
    Scanning,
    Results,
    /// Dry-run report: shows exactly what *would* be deleted. Deletes nothing.
    DryReport,
    Cleaning {
        done: usize,
        total: usize,
        freed: u64,
    },
    Done {
        freed: u64,
        errors: usize,
    },
}

/// One row in the category list.
struct Row {
    cat: Category,
    size: u64,
    entries: Vec<Entry>,
    selected: bool,
    scanned: bool,
}

/// Messages from the scan thread.
enum ScanMsg {
    Sized { id: &'static str, scan: scanner::CatScan },
    Done,
}

/// Messages from the clean thread.
enum CleanMsg {
    Progress { done: usize, total: usize, freed: u64 },
    Done { freed: u64, errors: usize },
}

pub struct App {
    phase: Phase,
    rows: Vec<Row>,
    scan_rx: Option<Receiver<ScanMsg>>,
    clean_rx: Option<Receiver<CleanMsg>>,
    scanned_count: usize,
    confirm_open: bool,
    dry_run: bool,
    icon: Option<egui::TextureHandle>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Decode the embedded icon once into a texture for the header.
        let icon = eframe::icon_data::from_png_bytes(crate::ICON_PNG)
            .ok()
            .map(|data| {
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [data.width as usize, data.height as usize],
                    &data.rgba,
                );
                cc.egui_ctx
                    .load_texture("app-icon", image, egui::TextureOptions::LINEAR)
            });
        Self {
            phase: Phase::Idle,
            rows: Vec::new(),
            scan_rx: None,
            clean_rx: None,
            scanned_count: 0,
            confirm_open: false,
            dry_run: false,
            icon,
        }
    }

    // --- orchestration ----------------------------------------------------

    fn start_scan(&mut self, ctx: &egui::Context) {
        let cats = categories::categories();
        self.rows = cats
            .iter()
            .cloned()
            .map(|cat| Row {
                cat,
                size: 0,
                entries: Vec::new(),
                selected: false,
                scanned: false,
            })
            .collect();
        self.scanned_count = 0;
        self.phase = Phase::Scanning;

        let (tx, rx) = mpsc::channel();
        self.scan_rx = Some(rx);
        let ctx = ctx.clone();

        std::thread::spawn(move || {
            use rayon::prelude::*;
            cats.par_iter().for_each_with(tx.clone(), |tx, cat| {
                let scan = scanner::scan_category(cat);
                let _ = tx.send(ScanMsg::Sized { id: cat.id, scan });
                ctx.request_repaint();
            });
            let _ = tx.send(ScanMsg::Done);
            ctx.request_repaint();
        });
    }

    fn poll_scan(&mut self) {
        let Some(rx) = self.scan_rx.take() else { return };
        let mut finished = false;
        loop {
            match rx.try_recv() {
                Ok(ScanMsg::Sized { id, scan }) => {
                    if let Some(row) = self.rows.iter_mut().find(|r| r.cat.id == id) {
                        row.size = scan.total;
                        row.entries = scan.entries;
                        row.scanned = true;
                        row.selected = row.cat.default_on && row.size > 0;
                    }
                    self.scanned_count += 1;
                }
                Ok(ScanMsg::Done) => {
                    finished = true;
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    finished = true;
                    break;
                }
            }
        }
        if finished {
            self.phase = Phase::Results;
        } else {
            self.scan_rx = Some(rx);
        }
    }

    fn start_clean(&mut self, ctx: &egui::Context) {
        let selected: Vec<Category> = self
            .rows
            .iter()
            .filter(|r| r.selected && r.size > 0)
            .map(|r| r.cat.clone())
            .collect();
        let total = selected.len();
        self.phase = Phase::Cleaning {
            done: 0,
            total,
            freed: 0,
        };

        let (tx, rx) = mpsc::channel();
        self.clean_rx = Some(rx);
        let ctx = ctx.clone();

        std::thread::spawn(move || {
            let mut freed = 0u64;
            let mut error_count = 0usize;
            for (i, cat) in selected.iter().enumerate() {
                let result = cleaner::clean_category(cat);
                freed += result.freed;
                error_count += result.errors.len();
                let _ = tx.send(CleanMsg::Progress {
                    done: i + 1,
                    total,
                    freed,
                });
                ctx.request_repaint();
            }
            let _ = tx.send(CleanMsg::Done {
                freed,
                errors: error_count,
            });
            ctx.request_repaint();
        });
    }

    fn poll_clean(&mut self) {
        let Some(rx) = self.clean_rx.take() else { return };
        let mut finished = false;
        loop {
            match rx.try_recv() {
                Ok(CleanMsg::Progress { done, total, freed }) => {
                    self.phase = Phase::Cleaning { done, total, freed };
                }
                Ok(CleanMsg::Done { freed, errors }) => {
                    self.phase = Phase::Done { freed, errors };
                    finished = true;
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    finished = true;
                    break;
                }
            }
        }
        if !finished {
            self.clean_rx = Some(rx);
        }
    }

    // --- derived values ---------------------------------------------------

    fn selected_total(&self) -> u64 {
        self.selected_rows().map(|r| r.size).sum()
    }

    fn selected_count(&self) -> usize {
        self.selected_rows().count()
    }

    fn selected_item_count(&self) -> usize {
        self.selected_rows().map(|r| r.entries.len()).sum()
    }

    fn selected_rows(&self) -> impl Iterator<Item = &Row> {
        self.rows.iter().filter(|r| r.selected && r.size > 0)
    }

    fn found_total(&self) -> u64 {
        self.rows.iter().map(|r| r.size).sum()
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_scan();
        self.poll_clean();

        top_bar(ctx, &self.icon);
        self.bottom_bar(ctx);

        egui::CentralPanel::default().show(ctx, |ui| match self.phase {
            Phase::Idle => self.view_idle(ui),
            Phase::Scanning => self.view_list(ui, true),
            Phase::Results => self.view_list(ui, false),
            Phase::DryReport => self.view_dry_report(ui),
            Phase::Cleaning { done, total, freed } => view_cleaning(ui, done, total, freed),
            Phase::Done { freed, errors } => self.view_done(ui, freed, errors),
        });

        self.confirm_modal(ctx);
    }
}

// ---------------------------------------------------------------------------
// Colors
// ---------------------------------------------------------------------------

const ACCENT: egui::Color32 = egui::Color32::from_rgb(0x3b, 0x82, 0xf6);
const GOOD: egui::Color32 = egui::Color32::from_rgb(0x22, 0xc5, 0x5e);
const WARN: egui::Color32 = egui::Color32::from_rgb(0xf5, 0x9e, 0x0b);
const DANGER: egui::Color32 = egui::Color32::from_rgb(0xef, 0x44, 0x44);
const HEART: egui::Color32 = egui::Color32::from_rgb(0xdb, 0x27, 0x77);
const GITHUB_COLOR: egui::Color32 = egui::Color32::from_rgb(0x30, 0x36, 0x3d);

/// Where the in-app "Support" buttons send people. Set this to your Stripe
/// Payment Link (https://buy.stripe.com/...) or your website's donate page.
const SUPPORT_URL: &str = "https://hartwellbridge.com/en/clean-my-shit#support";

/// The project's GitHub repository.
const GITHUB_URL: &str = "https://github.com/frank10gm/clean-my-shit";

/// A filled button that opens `url` in the browser.
fn link_button(ui: &mut egui::Ui, label: &str, url: &str, fill: egui::Color32, big: bool) {
    let mut text = egui::RichText::new(label).color(egui::Color32::WHITE).strong();
    if big {
        text = text.size(15.0);
    }
    let btn = egui::Button::new(text).fill(fill);
    let resp = if big {
        ui.add_sized(egui::vec2(190.0, 38.0), btn)
    } else {
        ui.add(btn)
    };
    if resp.clicked() {
        ui.ctx().open_url(egui::OpenUrl::new_tab(url));
    }
}

/// Pink "Support" button. Renders nothing until a real URL is set.
fn support_button(ui: &mut egui::Ui, label: &str, big: bool) {
    if SUPPORT_URL.contains("YOUR-WEBSITE") {
        return;
    }
    link_button(ui, label, SUPPORT_URL, HEART, big);
}

/// Dark "GitHub" button.
fn github_button(ui: &mut egui::Ui, label: &str, big: bool) {
    link_button(ui, label, GITHUB_URL, GITHUB_COLOR, big);
}

// ---------------------------------------------------------------------------
// Views
// ---------------------------------------------------------------------------

fn top_bar(ctx: &egui::Context, icon: &Option<egui::TextureHandle>) {
    egui::TopBottomPanel::top("top").show(ctx, |ui| {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if let Some(icon) = icon {
                ui.add(egui::Image::new(egui::load::SizedTexture::new(
                    icon.id(),
                    egui::vec2(34.0, 34.0),
                )));
                ui.add_space(4.0);
            }
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Clean My Shit").size(20.0).strong());
                ui.label(
                    egui::RichText::new("Free up disk space. No daemon — scan, pick, clean.")
                        .size(12.0)
                        .weak(),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                support_button(ui, "♥ Support", false);
                github_button(ui, "★ GitHub", false);
            });
        });
        ui.add_space(8.0);
    });
}

impl App {
    fn view_idle(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            ui.label(egui::RichText::new("Ready to find clutter").size(18.0).strong());
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(
                    "Scans caches, temp files, logs and other safe-to-delete clutter.\n\
                     Nothing is deleted until you review and confirm.",
                )
                .weak(),
            );
            ui.add_space(24.0);
            let btn = egui::Button::new(egui::RichText::new("  Scan  ").size(18.0).strong())
                .fill(ACCENT)
                .min_size(egui::vec2(180.0, 44.0));
            if ui.add(btn).clicked() {
                let ctx = ui.ctx().clone();
                self.start_scan(&ctx);
            }
        });
    }

    fn view_list(&mut self, ui: &mut egui::Ui, scanning: bool) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if scanning {
                ui.add(egui::Spinner::new());
                ui.label(format!(
                    "Scanning… ({}/{})",
                    self.scanned_count,
                    self.rows.len()
                ));
            } else {
                ui.label(
                    egui::RichText::new(format!("Found {} of clutter", format_size(self.found_total())))
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Deselect all").clicked() {
                        for r in &mut self.rows {
                            r.selected = false;
                        }
                    }
                    if ui.button("Select safe").clicked() {
                        for r in &mut self.rows {
                            r.selected = r.size > 0 && r.cat.risk == Risk::Safe;
                        }
                    }
                });
            }
        });
        ui.add_space(4.0);
        ui.separator();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for row in &mut self.rows {
                    row_widget(ui, row);
                }
            });
    }

    fn view_dry_report(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("🔍 Dry run").size(16.0).strong());
            ui.label(
                egui::RichText::new("preview only — nothing was deleted")
                    .color(WARN),
            );
        });
        ui.label(
            egui::RichText::new(format!(
                "Would delete {} items and free {}.",
                self.selected_item_count(),
                format_size(self.selected_total())
            ))
            .strong(),
        );
        ui.add_space(4.0);
        ui.separator();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for row in self.rows.iter().filter(|r| r.selected && r.size > 0) {
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(row.cat.name).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new(format_size(row.size)).strong());
                        });
                    });
                    for entry in row.entries.iter().take(PREVIEW_LIMIT) {
                        entry_row(ui, entry, true);
                    }
                    if row.entries.len() > PREVIEW_LIMIT {
                        ui.label(
                            egui::RichText::new(format!(
                                "    …and {} more",
                                row.entries.len() - PREVIEW_LIMIT
                            ))
                            .weak(),
                        );
                    }
                    ui.separator();
                }
            });
    }

    fn view_done(&mut self, ui: &mut egui::Ui, freed: u64, errors: usize) {
        ui.vertical_centered(|ui| {
            ui.add_space(70.0);
            ui.label(egui::RichText::new("✅").size(48.0));
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(format!("Freed {}", format_size(freed)))
                    .size(24.0)
                    .strong()
                    .color(GOOD),
            );
            ui.add_space(6.0);
            if errors > 0 {
                ui.label(
                    egui::RichText::new(format!(
                        "{errors} item(s) couldn't be removed (in use or protected) — skipped."
                    ))
                    .weak(),
                );
            }
            ui.add_space(24.0);
            if ui
                .add(
                    egui::Button::new(egui::RichText::new("  Scan again  ").size(16.0))
                        .min_size(egui::vec2(160.0, 38.0)),
                )
                .clicked()
            {
                let ctx = ui.ctx().clone();
                self.start_scan(&ctx);
            }

            ui.add_space(28.0);
            ui.label(
                egui::RichText::new("Clean My Shit is free. If it helped, chip in ♥")
                    .weak(),
            );
            ui.add_space(8.0);
            support_button(ui, "♥ Support development", true);
            ui.add_space(6.0);
            github_button(ui, "★ Star on GitHub", true);
        });
    }

    fn bottom_bar(&mut self, ctx: &egui::Context) {
        let in_results = matches!(self.phase, Phase::Results);
        let in_dry = matches!(self.phase, Phase::DryReport);
        if !in_results && !in_dry {
            return;
        }
        egui::TopBottomPanel::bottom("bottom").show(ctx, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "Selected: {}  ({} categories)",
                        format_size(self.selected_total()),
                        self.selected_count()
                    ))
                    .strong(),
                );
                ui.checkbox(&mut self.dry_run, "Dry run (preview, delete nothing)");

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let enabled = self.selected_count() > 0;
                    if in_dry {
                        if ui.button("◀ Back").clicked() {
                            self.phase = Phase::Results;
                        }
                        let btn = egui::Button::new(
                            egui::RichText::new("  Delete for real  ")
                                .strong()
                                .color(egui::Color32::WHITE),
                        )
                        .fill(DANGER)
                        .min_size(egui::vec2(150.0, 36.0));
                        if ui.add_enabled(enabled, btn).clicked() {
                            self.confirm_open = true;
                        }
                    } else {
                        let (label, fill) = if self.dry_run {
                            ("  Preview  ", ACCENT)
                        } else {
                            ("  Clean  ", DANGER)
                        };
                        let btn = egui::Button::new(
                            egui::RichText::new(label).size(16.0).strong().color(egui::Color32::WHITE),
                        )
                        .fill(fill)
                        .min_size(egui::vec2(140.0, 36.0));
                        if ui.add_enabled(enabled, btn).clicked() {
                            if self.dry_run {
                                self.phase = Phase::DryReport;
                            } else {
                                self.confirm_open = true;
                            }
                        }
                    }
                });
            });
            ui.add_space(8.0);
        });
    }

    fn confirm_modal(&mut self, ctx: &egui::Context) {
        if !self.confirm_open {
            return;
        }
        let total = format_size(self.selected_total());
        let count = self.selected_count();
        let mut start = false;
        egui::Window::new("Confirm cleanup")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!(
                        "Delete clutter from {count} categories and free about {total}?"
                    ))
                    .size(15.0),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("This permanently deletes the files. It cannot be undone.")
                        .color(WARN),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("Delete now").color(egui::Color32::WHITE),
                            )
                            .fill(DANGER)
                            .min_size(egui::vec2(120.0, 32.0)),
                        )
                        .clicked()
                    {
                        start = true;
                    }
                    if ui
                        .add(egui::Button::new("Cancel").min_size(egui::vec2(100.0, 32.0)))
                        .clicked()
                    {
                        self.confirm_open = false;
                    }
                });
                ui.add_space(4.0);
            });

        if start {
            self.confirm_open = false;
            self.start_clean(ctx);
        }
    }
}

fn view_cleaning(ui: &mut egui::Ui, done: usize, total: usize, freed: u64) {
    ui.vertical_centered(|ui| {
        ui.add_space(80.0);
        ui.add(egui::Spinner::new().size(32.0));
        ui.add_space(16.0);
        ui.label(egui::RichText::new("Cleaning…").size(18.0).strong());
        ui.add_space(12.0);
        let frac = if total == 0 {
            0.0
        } else {
            done as f32 / total as f32
        };
        ui.add(
            egui::ProgressBar::new(frac)
                .desired_width(320.0)
                .text(format!("{done}/{total}")),
        );
        ui.add_space(8.0);
        ui.label(egui::RichText::new(format!("Freed so far: {}", format_size(freed))).weak());
    });
}

/// One category row: checkbox + name/description + size + an expandable file
/// preview ("show files").
fn row_widget(ui: &mut egui::Ui, row: &mut Row) {
    let has_data = row.size > 0;
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add_enabled(has_data, egui::Checkbox::new(&mut row.selected, ""));
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(row.cat.name).strong());
                if row.cat.risk == Risk::Caution {
                    ui.label(egui::RichText::new("caution").size(10.0).color(WARN));
                }
            });
            ui.label(egui::RichText::new(row.cat.description).size(11.0).weak());
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let text = if !row.scanned {
                egui::RichText::new("…").weak()
            } else if has_data {
                egui::RichText::new(format_size(row.size)).strong()
            } else {
                egui::RichText::new("empty").weak()
            };
            ui.label(text);
        });
    });

    if has_data && !row.entries.is_empty() {
        egui::CollapsingHeader::new(format!("Show {} items", row.entries.len()))
            .id_salt(row.cat.id)
            .show(ui, |ui| {
                for entry in row.entries.iter().take(PREVIEW_LIMIT) {
                    entry_row(ui, entry, false);
                }
                if row.entries.len() > PREVIEW_LIMIT {
                    ui.label(
                        egui::RichText::new(format!(
                            "…and {} more",
                            row.entries.len() - PREVIEW_LIMIT
                        ))
                        .weak(),
                    );
                }
            });
    }

    ui.add_space(2.0);
    ui.separator();
}

/// A single file/dir line inside a preview list.
fn entry_row(ui: &mut egui::Ui, entry: &Entry, indent: bool) {
    let name = short_path(&entry.path);
    ui.horizontal(|ui| {
        if indent {
            ui.add_space(16.0);
        }
        ui.label(egui::RichText::new(name).size(12.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(format_size(entry.size)).size(12.0).weak());
        });
    });
}
