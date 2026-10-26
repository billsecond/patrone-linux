//! Windows-style dialogs: a title bar with the title and GTK's own close
//! button (in the corner and style the account chose), a white body, and
//! the buttons on the right of a gray footer.

use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};

use gtk::{gdk, gio, glib, prelude::*};

use crate::glyph::Glyph;
use crate::util;

/// A value that arrives later, on the main thread.
pub struct Receiver<T>(Rc<RefCell<(Option<T>, Option<Waker>)>>);

#[derive(Clone)]
pub struct Sender<T>(Rc<RefCell<(Option<T>, Option<Waker>)>>);

pub fn oneshot<T>() -> (Sender<T>, Receiver<T>) {
    let shared = Rc::new(RefCell::new((None, None)));
    (Sender(shared.clone()), Receiver(shared))
}

impl<T> Sender<T> {
    /// Delivers the value; only the first one counts.
    pub fn send(&self, value: T) {
        let mut shared = self.0.borrow_mut();
        if shared.0.is_none() {
            shared.0 = Some(value);
            if let Some(waker) = shared.1.take() {
                waker.wake();
            }
        }
    }
}

impl<T> Future for Receiver<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        let mut shared = self.0.borrow_mut();
        match shared.0.take() {
            Some(value) => Poll::Ready(value),
            None => {
                shared.1 = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

/// Closing a dialog without choosing.
pub const CLOSED: i32 = -1;

pub struct Dialog {
    pub window: gtk::Window,
    pub body: gtk::Box,
    pub footer: gtk::Box,
    sender: Sender<i32>,
    receiver: RefCell<Option<Receiver<i32>>>,
}

impl Dialog {
    pub fn new(parent: Option<&gtk::Window>, title: &str) -> Rc<Dialog> {
        let window = gtk::Window::builder()
            .title(title)
            .modal(parent.is_some())
            .resizable(false)
            .destroy_with_parent(true)
            .build();
        window.set_transient_for(parent);
        window.add_css_class("fluent-dialog");

        let title_label = gtk::Label::builder()
            .label(title)
            .xalign(0.0)
            .hexpand(true)
            .build();
        title_label.add_css_class("dialog-title");
        let bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        bar.add_css_class("dialog-titlebar");
        bar.append(&util::window_controls(gtk::PackType::Start));
        bar.append(&title_label);
        bar.append(&util::window_controls(gtk::PackType::End));
        window.set_titlebar(Some(&gtk::WindowHandle::builder().child(&bar).build()));

        let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
        body.add_css_class("dialog-body");
        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        footer.add_css_class("dialog-footer");
        footer.set_halign(gtk::Align::Fill);
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        footer.append(&spacer);
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.append(&body);
        content.append(&footer);
        window.set_child(Some(&content));

        let (sender, receiver) = oneshot();
        let dialog = Rc::new(Dialog {
            window,
            body,
            footer,
            sender,
            receiver: RefCell::new(Some(receiver)),
        });
        // The window's own handlers hold it weakly: a strong reference from
        // its children kept every closed dialog alive, and Folder Options
        // then showed the old, dead one again.
        {
            let sender = dialog.sender.clone();
            dialog.window.connect_close_request(move |_| {
                sender.send(CLOSED);
                glib::Propagation::Proceed
            });
        }
        let keys = gtk::EventControllerKey::new();
        {
            let window = dialog.window.downgrade();
            keys.connect_key_pressed(move |_, key, _, _| {
                if key == gdk::Key::Escape {
                    if let Some(window) = window.upgrade() {
                        window.close();
                    }
                    glib::Propagation::Stop
                } else {
                    glib::Propagation::Proceed
                }
            });
        }
        dialog.window.add_controller(keys);
        dialog
    }

    /// Adds a button that ends the dialog with `response`.
    pub fn add_button(self: &Rc<Self>, label: &str, response: i32, primary: bool) -> gtk::Button {
        let button = gtk::Button::with_mnemonic(label);
        button.add_css_class("dialog-button");
        if primary {
            button.add_css_class("suggested-action");
            self.window.set_default_widget(Some(&button));
        }
        let weak = Rc::downgrade(self);
        button.connect_clicked(move |_| {
            if let Some(dialog) = weak.upgrade() {
                dialog.respond(response);
            }
        });
        self.footer.append(&button);
        button
    }

    pub fn respond(&self, response: i32) {
        self.sender.send(response);
        self.window.destroy();
    }

    /// Shows the dialog and waits for a button (or `CLOSED`).
    pub async fn run(self: &Rc<Self>) -> i32 {
        self.window.present();
        let receiver = self.receiver.borrow_mut().take();
        match receiver {
            Some(receiver) => receiver.await,
            None => CLOSED,
        }
    }
}

/// A message with an icon, a heading, a detail and buttons. Returns the
/// chosen button's response.
pub async fn message(
    parent: Option<&gtk::Window>,
    title: &str,
    icon: &str,
    heading: &str,
    detail: &str,
    buttons: &[(&str, i32)],
    default: i32,
) -> i32 {
    let dialog = Dialog::new(parent, title);
    dialog.window.set_default_size(440, -1);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    let glyph = Glyph::sized(icon, 32);
    glyph.set_valign(gtk::Align::Start);
    row.append(&glyph);
    let text = gtk::Box::new(gtk::Orientation::Vertical, 6);
    text.set_hexpand(true);
    let heading_label = gtk::Label::builder()
        .label(heading)
        .xalign(0.0)
        .wrap(true)
        .width_chars(36)
        .max_width_chars(52)
        .build();
    heading_label.add_css_class("dialog-heading");
    text.append(&heading_label);
    if !detail.is_empty() {
        let detail_label = gtk::Label::builder()
            .label(detail)
            .xalign(0.0)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .width_chars(36)
            .max_width_chars(52)
            .selectable(true)
            .build();
        detail_label.add_css_class("dialog-detail");
        text.append(&detail_label);
    }
    row.append(&text);
    dialog.body.append(&row);
    for (label, response) in buttons {
        let button = dialog.add_button(label, *response, *response == default);
        if *response == default {
            button.grab_focus();
        }
    }
    dialog.run().await
}

/// Shows an error without waiting for it.
pub fn error(parent: Option<&gtk::Window>, title: &str, heading: &str, detail: &str) {
    let parent = parent.cloned();
    let (title, heading, detail) = (title.to_owned(), heading.to_owned(), detail.to_owned());
    glib::spawn_future_local(async move {
        message(
            parent.as_ref(),
            &title,
            "error",
            &heading,
            &detail,
            &[("OK", 0)],
            0,
        )
        .await;
    });
}

/// The words of a GIO error, without GIO's own prefixes.
pub fn describe(error: &glib::Error) -> String {
    if error.matches(gio::IOErrorEnum::PermissionDenied) {
        "You need permission to perform this action.".into()
    } else if error.matches(gio::IOErrorEnum::NoSpace) {
        "There is not enough space on the disk.".into()
    } else if error.matches(gio::IOErrorEnum::NotFound) {
        "Could not find this item. It is no longer located where it was.".into()
    } else if error.matches(gio::IOErrorEnum::ReadOnly) {
        "The disk is write-protected.".into()
    } else if error.matches(gio::IOErrorEnum::FilenameTooLong) {
        "The file name is too long.".into()
    } else if error.matches(gio::IOErrorEnum::InvalidFilename) {
        "The file name is not valid.".into()
    } else {
        error.message().to_owned()
    }
}
