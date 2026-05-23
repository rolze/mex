use crate::cache;
use anyhow::Result;
use gtk4::prelude::*;
use gtk4::{glib, Application};
use libadwaita::{ApplicationWindow, HeaderBar};
use std::path::PathBuf;
use std::sync::Arc;

const APP_ID: &str = "io.github.rolze.sem";

// ── Single-image mode ────────────────────────────────────────────────────────

pub fn run_single(path: PathBuf, tags: Vec<String>) -> Result<()> {
    let app = Application::builder().application_id(APP_ID).build();

    app.connect_activate(move |app| {
        build_single_window(app, path.clone(), tags.clone());
    });

    let exit_code = app.run_with_args::<String>(&[]);
    if exit_code != glib::ExitCode::SUCCESS {
        anyhow::bail!("sem: GTK application exited with error");
    }
    Ok(())
}

fn build_single_window(app: &Application, path: PathBuf, tags: Vec<String>) {
    let filename = basename(&path);

    let header = HeaderBar::new();

    let picture = gtk4::Picture::for_filename(&path);
    picture.set_content_fit(gtk4::ContentFit::Contain);
    picture.set_vexpand(true);
    picture.set_hexpand(true);

    let caption = make_caption(&filename, &tags);
    caption.set_margin_top(6);
    caption.set_margin_bottom(8);
    caption.set_margin_start(12);
    caption.set_margin_end(12);

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    content.append(&picture);
    content.append(&caption);

    let toolbar_view = libadwaita::ToolbarView::new();
    toolbar_view.add_top_bar(&header);
    toolbar_view.set_content(Some(&content));

    let window = ApplicationWindow::builder()
        .application(app)
        .title(&filename)
        .default_width(800)
        .default_height(600)
        .content(&toolbar_view)
        .build();

    add_escape_to_close(&window);
    window.present();
}

// ── Grid mode ────────────────────────────────────────────────────────────────

pub fn run_grid(entries: Vec<(PathBuf, Vec<String>)>, cache_dir: PathBuf) -> Result<()> {
    let app = Application::builder().application_id(APP_ID).build();

    app.connect_activate(move |app| {
        build_grid_window(app, entries.clone(), cache_dir.clone());
    });

    let exit_code = app.run_with_args::<String>(&[]);
    if exit_code != glib::ExitCode::SUCCESS {
        anyhow::bail!("sem: GTK application exited with error");
    }
    Ok(())
}

fn build_grid_window(app: &Application, entries: Vec<(PathBuf, Vec<String>)>, cache_dir: PathBuf) {
    let entries = Arc::new(entries);
    let cache_dir = Arc::new(cache_dir);
    let n = entries.len();
    let grid_title = format!("{n} images");

    // ── Single-image view (pre-built, swapped in on demand) ──────────────────
    let single_picture = gtk4::Picture::new();
    single_picture.set_content_fit(gtk4::ContentFit::Contain);
    single_picture.set_vexpand(true);
    single_picture.set_hexpand(true);

    let single_caption = gtk4::Label::new(None);
    single_caption.set_wrap(true);
    single_caption.set_justify(gtk4::Justification::Center);
    single_caption.set_halign(gtk4::Align::Center);
    single_caption.add_css_class("caption");
    single_caption.set_margin_top(6);
    single_caption.set_margin_bottom(8);

    let single_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    single_box.append(&single_picture);
    single_box.append(&single_caption);

    // ── Grid ─────────────────────────────────────────────────────────────────
    let flow_box = gtk4::FlowBox::new();
    flow_box.set_selection_mode(gtk4::SelectionMode::Single);
    flow_box.set_homogeneous(true);
    flow_box.set_column_spacing(6);
    flow_box.set_row_spacing(6);
    flow_box.set_min_children_per_line(2);
    flow_box.set_max_children_per_line(8);
    flow_box.set_margin_start(8);
    flow_box.set_margin_end(8);
    flow_box.set_margin_top(8);
    flow_box.set_margin_bottom(8);

    // One Picture per entry; populated progressively by worker thread.
    let mut cell_pictures: Vec<gtk4::Picture> = Vec::with_capacity(n);

    for (path, _) in entries.iter() {
        let pic = gtk4::Picture::new();
        pic.set_size_request(256, 256);
        pic.set_content_fit(gtk4::ContentFit::Contain);

        let label = gtk4::Label::new(Some(&basename(path)));
        label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        label.set_max_width_chars(18);
        label.set_halign(gtk4::Align::Center);

        let cell = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        cell.set_margin_top(4);
        cell.set_margin_bottom(4);
        cell.set_margin_start(4);
        cell.set_margin_end(4);
        cell.append(&pic);
        cell.append(&label);

        flow_box.append(&cell);
        cell_pictures.push(pic);
    }

    let scrolled = gtk4::ScrolledWindow::new();
    scrolled.set_child(Some(&flow_box));
    scrolled.set_vexpand(true);

    // ── Stack: "grid" / "single" pages ───────────────────────────────────────
    let stack = gtk4::Stack::new();
    stack.add_named(&scrolled, Some("grid"));
    stack.add_named(&single_box, Some("single"));

    // ── Header bar + back button ──────────────────────────────────────────────
    let header = HeaderBar::new();
    let title_label = gtk4::Label::new(Some(&grid_title));
    title_label.add_css_class("title");
    header.set_title_widget(Some(&title_label));

    let back_btn = gtk4::Button::from_icon_name("go-previous-symbolic");
    back_btn.set_visible(false);
    header.pack_start(&back_btn);

    // ── ToolbarView + Window ──────────────────────────────────────────────────
    let toolbar_view = libadwaita::ToolbarView::new();
    toolbar_view.add_top_bar(&header);
    toolbar_view.set_content(Some(&stack));

    let window = ApplicationWindow::builder()
        .application(app)
        .title(&grid_title)
        .default_width(960)
        .default_height(720)
        .content(&toolbar_view)
        .build();

    // ── Click: grid → single ─────────────────────────────────────────────────
    {
        let entries = Arc::clone(&entries);
        let single_picture = single_picture.clone();
        let single_caption = single_caption.clone();
        let stack = stack.clone();
        let back_btn = back_btn.clone();
        let title_label = title_label.clone();
        let window = window.clone();

        flow_box.connect_child_activated(move |_, child| {
            let idx = child.index() as usize;
            if let Some((path, tags)) = entries.get(idx) {
                let name = basename(path);
                let caption_text = caption_text(&name, tags);

                single_picture.set_file(Some(&gtk4::gio::File::for_path(path)));
                single_caption.set_text(&caption_text);
                title_label.set_label(&name);
                window.set_title(Some(&name));
                back_btn.set_visible(true);
                stack.set_visible_child_name("single");
            }
        });
    }

    // ── Back button: single → grid ────────────────────────────────────────────
    {
        let stack = stack.clone();
        let back_btn_ref = back_btn.clone();
        let title_label = title_label.clone();
        let window = window.clone();
        let grid_title = grid_title.clone();

        back_btn.connect_clicked(move |_| {
            stack.set_visible_child_name("grid");
            back_btn_ref.set_visible(false);
            title_label.set_label(&grid_title);
            window.set_title(Some(&grid_title));
        });
    }

    // ── Escape key ────────────────────────────────────────────────────────────
    {
        let stack = stack.clone();
        let back_btn = back_btn.clone();
        let title_label = title_label.clone();
        let grid_title = grid_title.clone();
        let window_weak = window.downgrade();

        let key_ctrl = gtk4::EventControllerKey::new();
        key_ctrl.connect_key_pressed(move |_, key, _, _| {
            if key == gtk4::gdk::Key::Escape {
                let visible = stack
                    .visible_child_name()
                    .map(|s| s.to_string())
                    .unwrap_or_default();

                if visible == "single" {
                    stack.set_visible_child_name("grid");
                    back_btn.set_visible(false);
                    title_label.set_label(&grid_title);
                    if let Some(w) = window_weak.upgrade() {
                        w.set_title(Some(&grid_title));
                    }
                } else if let Some(w) = window_weak.upgrade() {
                    w.close();
                }
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        window.add_controller(key_ctrl);
    }

    // ── Progressive thumbnail loading ─────────────────────────────────────────
    let (sender, receiver) = std::sync::mpsc::channel::<(usize, Option<PathBuf>)>();

    {
        let entries = Arc::clone(&entries);
        let cache_dir = Arc::clone(&cache_dir);

        std::thread::spawn(move || {
            for (i, (path, _)) in entries.iter().enumerate() {
                let result = cache::ensure_thumbnail(path, &cache_dir).ok();
                if sender.send((i, result)).is_err() {
                    break; // window closed
                }
            }
        });
    }

    glib::idle_add_local(move || match receiver.try_recv() {
        Ok((idx, thumb_opt)) => {
            if let (Some(pic), Some(thumb)) = (cell_pictures.get(idx), thumb_opt) {
                pic.set_file(Some(&gtk4::gio::File::for_path(thumb)));
            }
            glib::ControlFlow::Continue
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
    });

    window.present();
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn basename(path: &PathBuf) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

fn caption_text(filename: &str, tags: &[String]) -> String {
    if tags.is_empty() {
        filename.to_owned()
    } else {
        format!("{}  ·  {}", filename, tags.join(", "))
    }
}

fn make_caption(filename: &str, tags: &[String]) -> gtk4::Label {
    let label = gtk4::Label::new(Some(&caption_text(filename, tags)));
    label.set_wrap(true);
    label.set_justify(gtk4::Justification::Center);
    label.set_halign(gtk4::Align::Center);
    label.add_css_class("caption");
    label
}

fn add_escape_to_close(window: &ApplicationWindow) {
    let key_ctrl = gtk4::EventControllerKey::new();
    let win_ref = window.downgrade();
    key_ctrl.connect_key_pressed(move |_, key, _, _| {
        if key == gtk4::gdk::Key::Escape {
            if let Some(w) = win_ref.upgrade() {
                w.close();
            }
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    window.add_controller(key_ctrl);
}
