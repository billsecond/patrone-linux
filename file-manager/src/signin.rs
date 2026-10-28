//! Patrone's own sign-in for mounting: network credentials as Windows asks
//! for them ("Enter network credentials"), an encrypted drive's password,
//! and the questions a mount can ask, all in File Manager's dialogs. GTK's
//! mount dialog didn't follow the style sheet: in dark mode its words were
//! white on light grey, and its buttons sat in a strip of their own (the
//! user, 2026-10-28: "Connecting to a network share looks horible").

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::{gio, glib, prelude::*};

use crate::dialogs::{self, Dialog};

/// What's being mounted, for the dialog's words.
#[derive(Clone)]
pub enum Target {
    /// A network computer or folder, by name ("NAS2", "photos (\\nas)").
    Network(String),
    /// A drive on this computer: an encrypted one asks for its password.
    Drive(String),
}

impl Target {
    fn name(&self) -> &str {
        match self {
            Target::Network(name) | Target::Drive(name) => name,
        }
    }
}

/// A mount operation that asks with Patrone's dialogs.
pub fn operation(parent: Option<&gtk::Window>, target: Target) -> gio::MountOperation {
    let operation = gio::MountOperation::new();
    let parent = parent.cloned();
    // The dialog showing, so the backend giving up closes it
    let open: Rc<RefCell<Option<Rc<Dialog>>>> = Rc::default();
    let asked = Rc::new(Cell::new(0u32));
    {
        let (parent, target, open) = (parent.clone(), target.clone(), open.clone());
        operation.connect_ask_password(move |operation, message, user, domain, flags| {
            // GIO's own handler, which runs after this one, answers
            // "unhandled" at once: the mount failed behind the dialog
            operation.stop_signal_emission_by_name("ask-password");
            // The backend asks again when the password was wrong
            asked.set(asked.get() + 1);
            let request = Request {
                message: message.to_owned(),
                user: user.to_owned(),
                domain: domain.to_owned(),
                flags,
                retry: asked.get() > 1,
            };
            let (operation, parent, target, open) = (
                operation.clone(),
                parent.clone(),
                target.clone(),
                open.clone(),
            );
            glib::spawn_future_local(async move {
                match ask_password(parent.as_ref(), &target, &request, &open).await {
                    Some(answer) => {
                        operation.set_anonymous(answer.guest);
                        operation.set_username(Some(&answer.user));
                        operation.set_password(Some(&answer.password));
                        operation.set_domain(Some(&answer.domain));
                        operation.set_password_save(if answer.remember {
                            gio::PasswordSave::Permanently
                        } else {
                            gio::PasswordSave::Never
                        });
                        operation.reply(gio::MountOperationResult::Handled);
                    }
                    None => operation.reply(gio::MountOperationResult::Aborted),
                }
            });
        });
    }
    {
        let (parent, target) = (parent.clone(), target.clone());
        // The bindings have no typed connect for this one (its choices are
        // a string list)
        operation.connect_local("ask-question", false, move |values| {
            let operation = values[0].get::<gio::MountOperation>().ok()?;
            operation.stop_signal_emission_by_name("ask-question");
            let message = values[1].get::<String>().unwrap_or_default();
            let choices: Vec<String> = values[2]
                .get::<glib::StrV>()
                .map(|choices| choices.iter().map(|choice| choice.to_string()).collect())
                .unwrap_or_default();
            let (parent, target) = (parent.clone(), target.clone());
            glib::spawn_future_local(async move {
                // GIO's questions are "heading\ndetail"
                let (heading, detail) = message.split_once('\n').unwrap_or((&message, ""));
                let buttons: Vec<(&str, i32)> = choices
                    .iter()
                    .enumerate()
                    .map(|(index, choice)| (choice.as_str(), index as i32))
                    .collect();
                let title = match &target {
                    Target::Network(_) => "Network",
                    Target::Drive(_) => "Drive",
                };
                let response = dialogs::message(
                    parent.as_ref(),
                    title,
                    "question",
                    heading,
                    detail.trim(),
                    &buttons,
                    0,
                )
                .await;
                if response >= 0 {
                    operation.set_choice(response);
                    operation.reply(gio::MountOperationResult::Handled);
                } else {
                    operation.reply(gio::MountOperationResult::Aborted);
                }
            });
            None
        });
    }
    operation.connect_aborted(move |_| {
        let dialog = open.borrow().clone();
        if let Some(dialog) = dialog {
            dialog.respond(dialogs::CLOSED);
        }
    });
    operation
}

struct Request {
    message: String,
    user: String,
    domain: String,
    flags: gio::AskPasswordFlags,
    retry: bool,
}

struct Answer {
    user: String,
    password: String,
    domain: String,
    guest: bool,
    remember: bool,
}

/// Windows' "Enter network credentials", or a drive's password.
async fn ask_password(
    parent: Option<&gtk::Window>,
    target: &Target,
    request: &Request,
    open: &Rc<RefCell<Option<Rc<Dialog>>>>,
) -> Option<Answer> {
    let flags = request.flags;
    let network = matches!(target, Target::Network(_));
    let dialog = Dialog::new(
        parent,
        if network {
            "Network Credentials"
        } else {
            "Unlock Drive"
        },
    );
    dialog.window.set_default_size(420, -1);

    let text = |words: &str, class: &str| {
        let label = gtk::Label::builder()
            .label(words)
            .xalign(0.0)
            .wrap(true)
            .width_chars(36)
            .max_width_chars(52)
            .build();
        label.add_css_class(class);
        label
    };
    dialog.body.append(&text(
        if network {
            "Enter network credentials"
        } else {
            "Enter the drive's password"
        },
        "dialog-heading",
    ));
    let detail = match target {
        _ if target.name().is_empty() => request.message.clone(),
        Target::Network(name) => format!("Enter your credentials to connect to: {name}"),
        Target::Drive(name) => format!("Enter the password to unlock: {name}"),
    };
    dialog.body.append(&text(&detail, "dialog-detail"));
    if request.retry {
        dialog.body.append(&text(
            if network {
                "The user name or password is incorrect."
            } else {
                "The password is incorrect."
            },
            "dialog-detail",
        ));
    }

    let grid = gtk::Grid::builder()
        .row_spacing(8)
        .column_spacing(12)
        .margin_top(8)
        .build();
    // The labels line up, More choices' Domain too
    let labels = gtk::SizeGroup::new(gtk::SizeGroupMode::Horizontal);
    let mut row = 0;
    let mut add_row = |name: &str, field: &gtk::Widget| {
        let label = gtk::Label::builder()
            .label(name)
            .xalign(0.0)
            .mnemonic_widget(field)
            .use_underline(true)
            .build();
        labels.add_widget(&label);
        grid.attach(&label, 0, row, 1, 1);
        grid.attach(field, 1, row, 1, 1);
        row += 1;
    };
    let user = gtk::Entry::builder()
        .text(&request.user)
        .hexpand(true)
        .activates_default(true)
        .build();
    if flags.contains(gio::AskPasswordFlags::NEED_USERNAME) {
        add_row("_User name", user.upcast_ref());
    }
    let password = gtk::PasswordEntry::builder()
        .show_peek_icon(true)
        .hexpand(true)
        .activates_default(true)
        .build();
    if flags.contains(gio::AskPasswordFlags::NEED_PASSWORD) {
        add_row("_Password", password.upcast_ref());
    }
    dialog.body.append(&grid);

    let remember = gtk::CheckButton::with_mnemonic("_Remember my credentials");
    if flags.contains(gio::AskPasswordFlags::SAVING_SUPPORTED) {
        dialog.body.append(&remember);
    }

    // Domain and guest sign-in, under "More choices" as on Windows
    let domain = gtk::Entry::builder()
        .text(&request.domain)
        .hexpand(true)
        .activates_default(true)
        .build();
    let guest = gtk::CheckButton::with_mnemonic("Connect as a _guest, with no password");
    let more = gtk::Box::new(gtk::Orientation::Vertical, 8);
    more.set_margin_top(8);
    if flags.contains(gio::AskPasswordFlags::NEED_DOMAIN) {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let label = gtk::Label::builder()
            .label("_Domain")
            .xalign(0.0)
            .use_underline(true)
            .mnemonic_widget(&domain)
            .build();
        labels.add_widget(&label);
        row.append(&label);
        row.append(&domain);
        more.append(&row);
    }
    if flags.contains(gio::AskPasswordFlags::ANONYMOUS_SUPPORTED) {
        more.append(&guest);
        let (user, password, remember) = (user.clone(), password.clone(), remember.clone());
        guest.connect_toggled(move |guest| {
            let signed_in = !guest.is_active();
            user.set_sensitive(signed_in);
            password.set_sensitive(signed_in);
            remember.set_sensitive(signed_in);
        });
    }
    if more.first_child().is_some() {
        let expander = gtk::Expander::builder()
            .label("More choices")
            .child(&more)
            .build();
        dialog.body.append(&expander);
    }

    dialog.add_button(if network { "_Connect" } else { "_Unlock" }, 0, true);
    dialog.add_button("Cancel", 1, false);
    // Straight to the password when the user name is known
    if user.text().is_empty() && flags.contains(gio::AskPasswordFlags::NEED_USERNAME) {
        user.grab_focus();
    } else {
        password.grab_focus();
    }

    open.replace(Some(dialog.clone()));
    let response = dialog.run().await;
    open.replace(None);
    (response == 0).then(|| Answer {
        user: user.text().to_string(),
        password: password.text().to_string(),
        domain: domain.text().to_string(),
        guest: guest.is_active(),
        remember: remember.is_active(),
    })
}
