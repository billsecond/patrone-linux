//! The progress window for long operations, as on Windows: "45% complete",
//! what's being copied from where to where, a progress bar, pause and
//! cancel, and "More details" with the current item, time remaining, items
//! remaining and speed. Several operations stack in the one window.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::{gio, glib, prelude::*};

use crate::dialogs;
use crate::glyph::{self, Glyph};
use crate::util;

/// Operations that finish faster than this never show the window.
const SHOW_AFTER: Duration = Duration::from_millis(700);

pub struct Job {
    pub verb: String,
    pub from: String,
    pub to: String,
    pub items_total: Cell<u64>,
    pub items_done: Cell<u64>,
    pub bytes_total: Cell<u64>,
    pub bytes_done: Cell<u64>,
    pub current: RefCell<String>,
    pub cancellable: gio::Cancellable,
    paused: Cell<bool>,
    resume: RefCell<Vec<dialogs::Sender<()>>>,
    started: Instant,
    finished: Cell<bool>,
    /// While a question is on screen, time doesn't count toward the speed.
    waiting_since: Cell<Option<Instant>>,
    waited: Cell<Duration>,
}

thread_local! {
    static JOBS: RefCell<Vec<Rc<Job>>> = const { RefCell::new(Vec::new()) };
    static WINDOW: RefCell<Option<ProgressWindow>> = const { RefCell::new(None) };
    static TICKING: Cell<bool> = const { Cell::new(false) };
}

impl Job {
    /// Starts tracking an operation: "Copying" 3 items "from" Documents "to" Backup.
    pub fn start(verb: &str, from: &str, to: &str) -> Rc<Job> {
        let job = Rc::new(Job {
            verb: verb.to_owned(),
            from: from.to_owned(),
            to: to.to_owned(),
            items_total: Cell::new(0),
            items_done: Cell::new(0),
            bytes_total: Cell::new(0),
            bytes_done: Cell::new(0),
            current: RefCell::default(),
            cancellable: gio::Cancellable::new(),
            paused: Cell::new(false),
            resume: RefCell::default(),
            started: Instant::now(),
            finished: Cell::new(false),
            waiting_since: Cell::new(None),
            waited: Cell::new(Duration::ZERO),
        });
        JOBS.with(|jobs| jobs.borrow_mut().push(job.clone()));
        tick_soon();
        job
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancellable.is_cancelled()
    }

    pub fn finish(&self) {
        self.finished.set(true);
        JOBS.with(|jobs| jobs.borrow_mut().retain(|job| !job.finished.get()));
        refresh();
    }

    /// Waits here while the operation is paused.
    pub async fn checkpoint(&self) {
        while self.paused.get() && !self.is_cancelled() {
            let (sender, receiver) = dialogs::oneshot();
            self.resume.borrow_mut().push(sender);
            receiver.await;
        }
    }

    fn set_paused(&self, paused: bool) {
        self.paused.set(paused);
        if !paused {
            for sender in self.resume.take() {
                sender.send(());
            }
        }
    }

    pub fn cancel(&self) {
        self.cancellable.cancel();
        self.set_paused(false);
    }

    /// Marks time spent waiting for the user, which isn't counted.
    pub fn waiting(&self, waiting: bool) {
        if waiting {
            self.waiting_since.set(Some(Instant::now()));
        } else if let Some(since) = self.waiting_since.take() {
            self.waited.set(self.waited.get() + since.elapsed());
        }
    }

    fn fraction(&self) -> f64 {
        let bytes_total = self.bytes_total.get();
        if bytes_total > 0 {
            return (self.bytes_done.get() as f64 / bytes_total as f64).clamp(0.0, 1.0);
        }
        let items_total = self.items_total.get();
        if items_total > 0 {
            (self.items_done.get() as f64 / items_total as f64).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    fn speed(&self) -> f64 {
        let elapsed = self
            .started
            .elapsed()
            .saturating_sub(self.waited.get())
            .as_secs_f64();
        if elapsed < 0.5 {
            0.0
        } else {
            self.bytes_done.get() as f64 / elapsed
        }
    }

    fn heading(&self) -> String {
        let items = self.items_total.get();
        let what = if items == 1 {
            "1 item".to_owned()
        } else {
            format!("{} items", util::thousands(items))
        };
        match (self.from.is_empty(), self.to.is_empty()) {
            (false, false) => format!("{} {what} from {} to {}", self.verb, self.from, self.to),
            (false, true) => format!("{} {what} from {}", self.verb, self.from),
            _ => format!("{} {what}", self.verb),
        }
    }

    fn remaining(&self) -> String {
        let speed = self.speed();
        let left = self.bytes_total.get().saturating_sub(self.bytes_done.get());
        if speed < 1.0 || self.bytes_total.get() == 0 {
            return "Calculating…".into();
        }
        let seconds = (left as f64 / speed).ceil() as u64;
        match seconds {
            0..=5 => "A few seconds".into(),
            6..=59 => format!("About {seconds} seconds"),
            60..=119 => "About 1 minute".into(),
            120..=3599 => format!("About {} minutes", seconds / 60),
            _ => format!("About {} hours", seconds / 3600),
        }
    }
}

fn tick_soon() {
    if TICKING.with(|ticking| ticking.replace(true)) {
        return;
    }
    glib::timeout_add_local(Duration::from_millis(250), || {
        refresh();
        let active = JOBS.with(|jobs| !jobs.borrow().is_empty());
        if !active {
            TICKING.with(|ticking| ticking.set(false));
            glib::ControlFlow::Break
        } else {
            glib::ControlFlow::Continue
        }
    });
}

fn refresh() {
    let jobs: Vec<Rc<Job>> = JOBS.with(|jobs| jobs.borrow().clone());
    // An operation that is only waiting for an answer (a conflict, an
    // error) and hasn't moved anything yet doesn't need a progress window.
    let due: Vec<Rc<Job>> = jobs
        .into_iter()
        .filter(|job| job.started.elapsed() >= SHOW_AFTER)
        .filter(|job| {
            job.waiting_since.get().is_none()
                || job.bytes_done.get() > 0
                || job.items_done.get() > 0
        })
        .collect();
    WINDOW.with(|window| {
        let mut window = window.borrow_mut();
        if due.is_empty() {
            if let Some(open) = window.take() {
                open.window.destroy();
            }
            return;
        }
        let open = window.get_or_insert_with(ProgressWindow::new);
        open.update(&due);
    });
}

struct Row {
    job: Rc<Job>,
    widget: gtk::Box,
    heading: gtk::Label,
    percent: gtk::Label,
    bar: gtk::ProgressBar,
    pause: gtk::Button,
    name: gtk::Label,
    time: gtk::Label,
    items: gtk::Label,
    speed: gtk::Label,
}

struct ProgressWindow {
    window: gtk::Window,
    list: gtk::Box,
    rows: RefCell<Vec<Row>>,
    details: gtk::Expander,
}

impl ProgressWindow {
    fn new() -> ProgressWindow {
        let window = gtk::Window::builder()
            .title("Progress")
            .resizable(false)
            .default_width(460)
            .build();
        window.add_css_class("fluent-dialog");
        window.add_css_class("progress-window");
        let title = gtk::Label::builder().xalign(0.0).hexpand(true).build();
        title.add_css_class("dialog-title");
        let bar = util::header_box();
        bar.add_css_class("dialog-titlebar");
        bar.append(&util::window_controls(gtk::PackType::Start));
        bar.append(&title);
        bar.append(&util::window_controls(gtk::PackType::End));
        window.set_titlebar(Some(&gtk::WindowHandle::builder().child(&bar).build()));
        // Closing the window only hides it; the operations carry on, as on Windows.
        window.connect_close_request(|window| {
            window.set_visible(false);
            glib::Propagation::Stop
        });
        let list = gtk::Box::new(gtk::Orientation::Vertical, 12);
        list.add_css_class("dialog-body");
        let details = gtk::Expander::new(Some("More details"));
        details.add_css_class("progress-details");
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.append(&list);
        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        footer.add_css_class("dialog-footer");
        footer.append(&details);
        content.append(&footer);
        window.set_child(Some(&content));
        {
            let list = list.clone();
            details.connect_expanded_notify(move |expander| {
                let mut child = list.first_child();
                while let Some(row) = child {
                    if let Some(more) = row.last_child() {
                        more.set_visible(expander.is_expanded());
                    }
                    child = row.next_sibling();
                }
            });
        }
        window.present();
        ProgressWindow {
            window,
            list,
            rows: RefCell::default(),
            details,
        }
    }

    fn update(&self, jobs: &[Rc<Job>]) {
        let mut rows = self.rows.borrow_mut();
        rows.retain(|row| {
            let keep = jobs.iter().any(|job| Rc::ptr_eq(job, &row.job));
            if !keep {
                self.list.remove(&row.widget);
            }
            keep
        });
        for job in jobs {
            if !rows.iter().any(|row| Rc::ptr_eq(&row.job, job)) {
                let row = self.add_row(job);
                rows.push(row);
            }
        }
        for row in rows.iter() {
            let job = &row.job;
            let percent = (job.fraction() * 100.0).floor() as u32;
            row.heading.set_label(&job.heading());
            row.percent.set_label(&format!("{percent}% complete"));
            row.bar.set_fraction(job.fraction());
            row.name
                .set_label(&format!("Name: {}", job.current.borrow()));
            row.time
                .set_label(&format!("Time remaining: {}", job.remaining()));
            let left = job.items_total.get().saturating_sub(job.items_done.get());
            let bytes_left = job.bytes_total.get().saturating_sub(job.bytes_done.get());
            row.items.set_label(&format!(
                "Items remaining: {} ({})",
                util::thousands(left),
                util::size(bytes_left)
            ));
            row.speed
                .set_label(&format!("Speed: {}/s", util::size(job.speed() as u64)));
            if let Some(glyph) = row.pause.child().and_downcast::<Glyph>() {
                glyph.set_name(if job.paused.get() { "forward" } else { "pause" });
            }
            row.pause
                .set_tooltip_text(Some(if job.paused.get() { "Resume" } else { "Pause" }));
        }
        let title = match jobs {
            [job] => format!("{}% complete", (job.fraction() * 100.0).floor() as u32),
            _ => format!("{} operations running", jobs.len()),
        };
        self.window.set_title(Some(&title));
        if let Some(label) = self
            .window
            .titlebar()
            .and_then(|handle| handle.first_child())
            .and_then(|bar| bar.first_child())
            .and_downcast::<gtk::Label>()
        {
            label.set_label(&title);
        }
    }

    fn add_row(&self, job: &Rc<Job>) -> Row {
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 6);
        widget.add_css_class("progress-row");
        let heading = gtk::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .width_chars(40)
            .max_width_chars(56)
            .build();
        heading.add_css_class("progress-heading");
        widget.append(&heading);
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        let percent = gtk::Label::builder().xalign(0.0).hexpand(true).build();
        percent.add_css_class("progress-percent");
        line.append(&percent);
        let pause = glyph::button("pause", "Pause");
        let cancel = glyph::button("close", "Cancel");
        line.append(&pause);
        line.append(&cancel);
        widget.append(&line);
        let bar = gtk::ProgressBar::new();
        bar.add_css_class("operation-progress");
        widget.append(&bar);
        let more = gtk::Box::new(gtk::Orientation::Vertical, 2);
        more.set_visible(self.details.is_expanded());
        let label = || {
            let label = gtk::Label::builder()
                .xalign(0.0)
                .ellipsize(gtk::pango::EllipsizeMode::Middle)
                .build();
            label.add_css_class("progress-detail");
            more.append(&label);
            label
        };
        let (name, time, items, speed) = (label(), label(), label(), label());
        widget.append(&more);
        {
            let job = job.clone();
            pause.connect_clicked(move |_| {
                job.set_paused(!job.paused.get());
                refresh();
            });
        }
        {
            let job = job.clone();
            cancel.connect_clicked(move |_| job.cancel());
        }
        self.list.append(&widget);
        Row {
            job: job.clone(),
            widget,
            heading,
            percent,
            bar,
            pause,
            name,
            time,
            items,
            speed,
        }
    }
}
