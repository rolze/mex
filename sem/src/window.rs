use anyhow::Result;
use gtk4::prelude::*;
use gtk4::{glib, Application};
use libadwaita::prelude::*;
use libadwaita::{ApplicationWindow, HeaderBar};
use std::path::PathBuf;

const APP_ID: &str = "io.github.rolze.sem";

pub fn run(path: PathBuf, tags: Vec<String>) -> Result<()> {
    let app = Application::builder().application_id(APP_ID).build();

    app.connect_activate(move |app| {
        build_window(app, path.clone(), tags.clone());
    });

    let exit_code = app.run_with_args::<String>(&[]);
    if exit_code != glib::ExitCode::SUCCESS {
        anyhow::bail!("sem: GTK application exited with error");
    }
    Ok(())
}

fn build_window(app: &Application, path: PathBuf, tags: Vec<String>) {
    let filename = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned());

    let header = HeaderBar::new();

    let picture = gtk4::Picture::for_filename(&path);
    picture.set_content_fit(gtk4::ContentFit::Contain);
    picture.set_vexpand(true);
    picture.set_hexpand(true);

    let caption = build_caption(&filename, &tags);
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

    // Close on Escape
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

    window.present();
}

fn build_caption(filename: &str, tags: &[String]) -> gtk4::Label {
    let text = if tags.is_empty() {
        filename.to_owned()
    } else {
        format!("{}  ·  {}", filename, tags.join(", "))
    };

    let label = gtk4::Label::new(Some(&text));
    label.set_wrap(true);
    label.set_justify(gtk4::Justification::Center);
    label.set_halign(gtk4::Align::Center);
    label.add_css_class("caption");
    label
}
