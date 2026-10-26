//! Gallery: every picture and video in the Pictures folder and its
//! subfolders, newest first, as a wall of thumbnails.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::{Rc, Weak};

use gtk::{gdk, gio, glib, prelude::*};

use crate::folderview::{ATTRIBUTES, ViewEvent};
use crate::icons;
use crate::places::{self, Special};
use crate::thumbs;
use crate::util;

const SIZE: i32 = 176;

fn is_media(info: &gio::FileInfo) -> bool {
    let mime = util::content_type(info);
    mime.starts_with("image/") || mime.starts_with("video/")
}

pub struct GalleryPage {
    pub widget: gtk::Overlay,
    store: gio::ListStore,
    sorted: gtk::SortListModel,
    pub selection: gtk::MultiSelection,
    grid: gtk::GridView,
    empty: gtk::Label,
    cancellable: RefCell<Option<gio::Cancellable>>,
    events: crate::util::Handler<ViewEvent>,
    me: RefCell<Weak<GalleryPage>>,
}

impl GalleryPage {
    pub fn new() -> Rc<GalleryPage> {
        let store = gio::ListStore::new::<gio::FileInfo>();
        let sorter = gtk::CustomSorter::new(|a, b| {
            let time = |object: &glib::Object| {
                object
                    .downcast_ref::<gio::FileInfo>()
                    .and_then(util::modified)
                    .map(|t| t.to_unix())
                    .unwrap_or(0)
            };
            time(b).cmp(&time(a)).into()
        });
        let sorted = gtk::SortListModel::new(Some(store.clone()), Some(sorter));
        let selection = gtk::MultiSelection::new(Some(sorted.clone()));
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(|_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            let picture = gtk::Picture::builder()
                .content_fit(gtk::ContentFit::Cover)
                .width_request(SIZE)
                .height_request(SIZE)
                .build();
            picture.add_css_class("gallery-photo");
            item.set_child(Some(&picture));
        });
        factory.connect_bind(|_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            let (Some(picture), Some(info)) = (
                item.child().and_downcast::<gtk::Picture>(),
                item.item().and_downcast::<gio::FileInfo>(),
            ) else {
                return;
            };
            let scale = picture.scale_factor().max(1);
            picture.set_paintable(Some(&icons::paintable(icons::for_info(&info), 96, scale)));
            picture.set_tooltip_text(Some(&info.display_name()));
            let uri = util::file_of(&info)
                .map(|file| file.uri().to_string())
                .unwrap_or_default();
            // Remember which file this widget shows, for late thumbnails.
            picture.set_widget_name(&uri);
            let weak = picture.downgrade();
            thumbs::request(&info, SIZE * scale, move |texture| {
                let (Some(picture), Some(texture)) = (weak.upgrade(), texture) else {
                    return;
                };
                if picture.widget_name() == uri {
                    picture.set_paintable(Some(&texture));
                }
            });
        });
        let grid = gtk::GridView::new(Some(selection.clone()), Some(factory));
        grid.set_max_columns(64);
        grid.set_enable_rubberband(true);
        grid.add_css_class("gallery");
        let scrolled = gtk::ScrolledWindow::builder()
            .child(&grid)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .build();
        scrolled.add_css_class("file-list");
        let empty = gtk::Label::builder()
            .label("Pictures and videos in your Pictures folder appear here.")
            .valign(gtk::Align::Start)
            .margin_top(24)
            .visible(false)
            .build();
        empty.add_css_class("empty-folder");
        let widget = gtk::Overlay::builder().child(&scrolled).build();
        widget.add_overlay(&empty);
        let page = Rc::new(GalleryPage {
            widget,
            store,
            sorted,
            selection,
            grid,
            empty,
            cancellable: RefCell::default(),
            events: RefCell::default(),
            me: RefCell::default(),
        });
        *page.me.borrow_mut() = Rc::downgrade(&page);
        page.connect_signals();
        page.reload();
        page
    }

    pub fn connect_events(&self, events: impl Fn(ViewEvent) + 'static) {
        *self.events.borrow_mut() = Some(Rc::new(events));
    }

    fn emit(&self, event: ViewEvent) {
        let events = self.events.borrow().clone();
        if let Some(events) = events {
            events(event);
        }
    }

    fn connect_signals(&self) {
        let weak = self.me.borrow().clone();
        self.grid.connect_activate(move |_, position| {
            let Some(page) = weak.upgrade() else { return };
            if let Some(info) = page
                .selection
                .item(position)
                .and_downcast::<gio::FileInfo>()
            {
                page.emit(ViewEvent::Activate(vec![info]));
            }
        });
        let weak = self.me.borrow().clone();
        self.selection.connect_selection_changed(move |_, _, _| {
            if let Some(page) = weak.upgrade() {
                page.emit(ViewEvent::SelectionChanged);
            }
        });
        let click = gtk::GestureClick::new();
        click.set_button(gdk::BUTTON_SECONDARY);
        let weak = self.me.borrow().clone();
        click.connect_pressed(move |gesture, _, x, y| {
            let Some(page) = weak.upgrade() else { return };
            let Some(widget) = gesture.widget() else {
                return;
            };
            gesture.set_state(gtk::EventSequenceState::Claimed);
            let mut on_item = false;
            let mut current = widget.pick(x, y, gtk::PickFlags::DEFAULT);
            while let Some(candidate) = current {
                if let Some(picture) = candidate.downcast_ref::<gtk::Picture>() {
                    let uri = picture.widget_name();
                    for position in 0..page.sorted.n_items() {
                        let matches = page
                            .sorted
                            .item(position)
                            .and_downcast::<gio::FileInfo>()
                            .and_then(|info| util::file_of(&info))
                            .is_some_and(|file| file.uri() == uri);
                        if matches {
                            if !page.selection.is_selected(position) {
                                page.selection.select_item(position, true);
                            }
                            on_item = true;
                            break;
                        }
                    }
                    break;
                }
                current = candidate.parent();
            }
            if !on_item {
                page.selection.unselect_all();
            }
            page.emit(ViewEvent::ContextMenu {
                on_item,
                widget,
                x,
                y,
            });
        });
        self.grid.add_controller(click);
    }

    /// Finds the pictures again.
    pub fn reload(&self) {
        if let Some(cancellable) = self.cancellable.take() {
            cancellable.cancel();
        }
        self.store.remove_all();
        let Some(root) = places::user_folder(Special::Pictures) else {
            self.empty.set_visible(true);
            return;
        };
        let cancellable = gio::Cancellable::new();
        self.cancellable.replace(Some(cancellable.clone()));
        let weak = self.me.borrow().clone();
        let store = self.store.clone();
        glib::spawn_future_local(async move {
            let mut queue = VecDeque::from([root]);
            while let Some(folder) = queue.pop_front() {
                if cancellable.is_cancelled() {
                    return;
                }
                let Ok(enumerator) = folder
                    .enumerate_children_future(
                        ATTRIBUTES,
                        gio::FileQueryInfoFlags::NONE,
                        glib::Priority::LOW,
                    )
                    .await
                else {
                    continue;
                };
                while let Ok(infos) = enumerator.next_files_future(200, glib::Priority::LOW).await {
                    if infos.is_empty() || cancellable.is_cancelled() {
                        break;
                    }
                    let mut found = Vec::new();
                    for info in infos {
                        if info.is_hidden() {
                            continue;
                        }
                        let child = folder.child(info.name());
                        if info.file_type() == gio::FileType::Directory {
                            queue.push_back(child);
                        } else if is_media(&info) {
                            info.set_attribute_object("standard::file", &child);
                            found.push(info);
                        }
                    }
                    store.extend_from_slice(&found);
                }
            }
            if let Some(page) = weak.upgrade() {
                page.empty.set_visible(page.store.n_items() == 0);
                page.emit(ViewEvent::ItemsChanged);
            }
        });
    }

    pub fn selected_infos(&self) -> Vec<gio::FileInfo> {
        let selected = self.selection.selection();
        (0..selected.size())
            .filter_map(|index| {
                self.selection
                    .item(selected.nth(index as u32))
                    .and_downcast::<gio::FileInfo>()
            })
            .collect()
    }

    pub fn n_items(&self) -> u32 {
        self.sorted.n_items()
    }

    pub fn select_all(&self) {
        self.selection.select_all();
    }

    pub fn select_none(&self) {
        self.selection.unselect_all();
    }

    pub fn focus(&self) {
        self.grid.grab_focus();
    }
}
