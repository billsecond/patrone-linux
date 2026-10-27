//! The lists on Task Manager's pages: a column view over rows, sortable by
//! any column, filtered by the search box, optionally a tree (apps and the
//! processes in them) with section headers (Apps, Background processes,
//! System processes).

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, gio, glib, graphene};

use crate::row::Row;

#[derive(Clone, Copy)]
pub enum Cell {
    /// An icon and a name, with an expander in a tree
    Name,
    /// Text from a string property
    Text(&'static str),
    /// Right-aligned text, and a shade from a number property that grows
    /// darker past each of four thresholds
    Number(&'static str, Option<(&'static str, [f64; 4])>),
    /// A switch showing a true/false property, and a text property beside it
    Switch(&'static str, &'static str),
}

#[derive(Clone, Copy)]
pub enum Sort {
    Text(&'static str),
    /// Biggest first on the first click
    Number(&'static str),
}

pub struct Column {
    pub title: &'static str,
    pub cell: Cell,
    pub sort: Sort,
    pub width: i32,
    pub expand: bool,
}

impl Column {
    pub fn new(title: &'static str, cell: Cell, sort: Sort, width: i32) -> Self {
        Column {
            title,
            cell,
            sort,
            width,
            expand: false,
        }
    }

    pub fn expand(mut self) -> Self {
        self.expand = true;
        self
    }
}

type MenuHandler = Rc<RefCell<Option<Box<dyn Fn(&Row, &gtk::Widget, f64, f64)>>>>;
type SwitchHandler = Rc<RefCell<Option<Box<dyn Fn(&Row, bool)>>>>;
type SectionTitle = Rc<dyn Fn(u32) -> String>;

pub struct Table {
    pub view: gtk::ColumnView,
    pub selection: gtk::SingleSelection,
    pub widget: gtk::ScrolledWindow,
    pub columns: Vec<gtk::ColumnViewColumn>,
    /// What sorts the rows, run again after each update
    sorter: gtk::Sorter,
    filter: gtk::CustomFilter,
    tree: bool,
    menu: MenuHandler,
    switched: SwitchHandler,
    headers: Rc<RefCell<Vec<glib::WeakRef<gtk::Label>>>>,
    section_title: Option<SectionTitle>,
    /// Whether the list is scrolled to the top, and stays there
    pinned: Rc<std::cell::Cell<bool>>,
}

impl Table {
    /// `matches` says whether a row matches the search box; in a tree, a
    /// group shows when it or any of its processes match.
    pub fn new(
        root: &gio::ListStore,
        tree: bool,
        section_title: Option<SectionTitle>,
        columns: Vec<Column>,
        matches: impl Fn(&Row) -> bool + 'static,
    ) -> Self {
        let model: gio::ListModel = if tree {
            gtk::TreeListModel::new(root.clone(), false, false, |item| {
                item.downcast_ref::<Row>()
                    .and_then(Row::children)
                    .map(|children| children.upcast())
            })
            .upcast()
        } else {
            root.clone().upcast()
        };
        let sort_model = gtk::SortListModel::new(Some(model), None::<gtk::Sorter>);
        if section_title.is_some() {
            sort_model.set_section_sorter(Some(&gtk::CustomSorter::new(move |a, b| {
                row_of(a, tree)
                    .map(|row| row.kind())
                    .cmp(&row_of(b, tree).map(|row| row.kind()))
                    .into()
            })));
        }
        let filter = gtk::CustomFilter::new(move |item| {
            if !tree {
                return item.downcast_ref::<Row>().is_none_or(&matches);
            }
            let Some(list_row) = item.downcast_ref::<gtk::TreeListRow>() else {
                return true;
            };
            let Some(row) = list_row.item().and_downcast::<Row>() else {
                return true;
            };
            if matches(&row) {
                return true;
            }
            if let Some(children) = row.children() {
                for index in 0..children.n_items() {
                    if children
                        .item(index)
                        .and_downcast::<Row>()
                        .is_some_and(|child| matches(&child))
                    {
                        return true;
                    }
                }
            }
            list_row
                .parent()
                .and_then(|parent| parent.item())
                .and_downcast::<Row>()
                .is_some_and(|parent| matches(&parent))
        });
        let filter_model =
            gtk::FilterListModel::new(Some(sort_model.clone()), Some(filter.clone()));
        let selection = gtk::SingleSelection::new(Some(filter_model));
        selection.set_autoselect(false);
        selection.set_can_unselect(true);

        let view = gtk::ColumnView::new(Some(selection.clone()));
        view.add_css_class("tm-table");
        view.set_show_column_separators(true);
        view.set_reorderable(false);

        let menu: MenuHandler = Rc::new(RefCell::new(None));
        let switched: SwitchHandler = Rc::new(RefCell::new(None));
        let mut view_columns = Vec::new();
        for column in columns {
            let factory = cell_factory(
                column.cell,
                tree,
                menu.clone(),
                switched.clone(),
                selection.downgrade(),
            );
            let view_column = gtk::ColumnViewColumn::new(Some(column.title), Some(factory));
            view_column.set_resizable(true);
            view_column.set_expand(column.expand);
            if column.width > 0 {
                view_column.set_fixed_width(column.width);
            }
            match column.sort {
                Sort::Text(property) => {
                    let sorter = gtk::StringSorter::new(Some(row_property(property)));
                    sorter.set_ignore_case(true);
                    view_column.set_sorter(Some(&sorter));
                }
                Sort::Number(property) => {
                    let sorter = gtk::NumericSorter::new(Some(row_property(property)));
                    sorter.set_sort_order(gtk::SortType::Descending);
                    view_column.set_sorter(Some(&sorter));
                }
            }
            view.append_column(&view_column);
            view_columns.push(view_column);
        }

        let view_sorter = view.sorter();
        let sorter: gtk::Sorter = if tree {
            gtk::TreeListRowSorter::new(view_sorter).upcast()
        } else {
            view_sorter
                .unwrap_or_else(|| gtk::CustomSorter::new(|_, _| gtk::Ordering::Equal).upcast())
        };
        sort_model.set_sorter(Some(&sorter));

        let headers: Rc<RefCell<Vec<glib::WeakRef<gtk::Label>>>> = Rc::default();
        if let Some(title) = &section_title {
            let factory = gtk::SignalListItemFactory::new();
            factory.connect_setup(|_, item| {
                let Some(header) = item.downcast_ref::<gtk::ListHeader>() else {
                    return;
                };
                header.set_child(Some(
                    &gtk::Label::builder()
                        .xalign(0.0)
                        .css_classes(["tm-section"])
                        .build(),
                ));
            });
            let title = title.clone();
            let labels = headers.clone();
            factory.connect_bind(move |_, item| {
                let Some(header) = item.downcast_ref::<gtk::ListHeader>() else {
                    return;
                };
                let Some(label) = header.child().and_downcast::<gtk::Label>() else {
                    return;
                };
                let kind = header
                    .item()
                    .as_ref()
                    .and_then(|item| row_of(item, tree))
                    .map(|row| row.kind())
                    .unwrap_or(0);
                label.set_widget_name(&kind.to_string());
                label.set_label(&title(kind));
                let mut labels = labels.borrow_mut();
                labels.retain(|label| label.upgrade().is_some());
                labels.push(label.downgrade());
            });
            view.set_header_factory(Some(&factory));
        }

        let widget = gtk::ScrolledWindow::builder()
            .child(&view)
            .vexpand(true)
            .hexpand(true)
            .build();
        let pinned = Rc::new(std::cell::Cell::new(true));
        keep_at_top(&view, &selection, &widget, &pinned);
        Table {
            view,
            selection,
            widget,
            columns: view_columns,
            sorter,
            filter,
            tree,
            menu,
            switched,
            headers,
            section_title,
            pinned,
        }
    }

    /// Sorts again, for values that changed.
    pub fn resort(&self) {
        self.sorter.changed(gtk::SorterChange::Different);
    }

    /// Filters again, for a new search.
    pub fn refilter(&self) {
        self.filter.changed(gtk::FilterChange::Different);
    }

    /// Writes the section headers again ("Apps (5)").
    pub fn refresh_headers(&self) {
        let Some(title) = &self.section_title else {
            return;
        };
        for label in self
            .headers
            .borrow()
            .iter()
            .filter_map(|label| label.upgrade())
        {
            let kind = label.widget_name().parse().unwrap_or(0);
            label.set_label(&title(kind));
        }
    }

    pub fn selected(&self) -> Option<Row> {
        self.selection
            .selected_item()
            .as_ref()
            .and_then(|item| row_of(item, self.tree))
    }

    /// Selects the first row this says yes to, and shows it.
    pub fn select(&self, wanted: impl Fn(&Row) -> bool) -> bool {
        for index in 0..self.selection.n_items() {
            let Some(item) = self.selection.item(index) else {
                continue;
            };
            if row_of(&item, self.tree).is_some_and(|row| wanted(&row)) {
                self.pinned.set(false);
                self.view.scroll_to(
                    index,
                    None,
                    gtk::ListScrollFlags::SELECT | gtk::ListScrollFlags::FOCUS,
                    None,
                );
                return true;
            }
        }
        false
    }

    /// What a right-click on a row does: given the row, the cell clicked
    /// and where.
    pub fn on_menu(&self, handler: impl Fn(&Row, &gtk::Widget, f64, f64) + 'static) {
        *self.menu.borrow_mut() = Some(Box::new(handler));
    }

    /// What flipping a row's switch does.
    pub fn on_switch(&self, handler: impl Fn(&Row, bool) + 'static) {
        *self.switched.borrow_mut() = Some(Box::new(handler));
    }
}

/// A list's right-click menu of window actions. It's made once, with the
/// list: GTK shows a section's separator only once it has looked up the
/// section's actions, so a menu made and opened at once was measured
/// without its separators, and its last items were cut off.
pub fn context_menu(menu: &gio::Menu, view: &gtk::ColumnView) -> gtk::PopoverMenu {
    let parent: gtk::Widget = view.parent().unwrap_or_else(|| view.clone().upcast());
    let popover = gtk::PopoverMenu::from_model(Some(menu));
    popover.set_parent(&parent);
    popover.set_has_arrow(false);
    popover.set_halign(gtk::Align::Start);
    popover
}

/// Opens a list's menu at a point in one of its cells.
pub fn popup(popover: &gtk::PopoverMenu, cell: &gtk::Widget, x: f64, y: f64) {
    let Some(parent) = popover.parent() else {
        return;
    };
    let point = cell
        .compute_point(&parent, &graphene::Point::new(x as f32, y as f32))
        .unwrap_or_else(|| graphene::Point::new(x as f32, y as f32));
    popover.set_pointing_to(Some(&gdk::Rectangle::new(
        point.x() as i32,
        point.y() as i32,
        1,
        1,
    )));
    popover.popup();
}

/// Keeps a list at its top while it's there, as rows come, go and change
/// places every second.
///
/// GTK keeps a list's place by a row (its anchor) and follows that row when
/// rows are added above it or the order changes (gtklistitemmanager.c), so a
/// list at the top would wander off with whichever row was first. While
/// the list is at the top, each change anchors it on the first row again
/// (scroll_to), and any scroll GTK makes while laying the rows out is put
/// back to the top, which also keeps the first section's header ("Apps")
/// in view: GTK anchors on the row under it. GTK makes those scrolls with
/// its own handler blocked (gtk_list_base_set_adjustment_values), so
/// putting them back neither moves the anchor nor makes GTK look up rows
/// that aren't laid out yet. Scrolling down or paging down lets the list
/// go; scrolling back to the top keeps it there again.
fn keep_at_top(
    view: &gtk::ColumnView,
    selection: &gtk::SingleSelection,
    scrolled: &gtk::ScrolledWindow,
    pinned: &Rc<std::cell::Cell<bool>>,
) {
    let adjustment = scrolled.vadjustment();
    adjustment.connect_value_changed(glib::clone!(
        #[strong]
        pinned,
        move |adjustment| {
            if adjustment.value() < 0.5 {
                pinned.set(true);
            } else if pinned.get() {
                adjustment.set_value(0.0);
            }
        }
    ));
    selection.connect_items_changed(glib::clone!(
        #[strong]
        pinned,
        #[weak]
        view,
        move |selection, _, _, _| {
            if pinned.get() && selection.n_items() > 0 {
                view.scroll_to(0, None, gtk::ListScrollFlags::NONE, None);
            }
        }
    ));

    // The person scrolling away from the top
    let wheel = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
    wheel.set_propagation_phase(gtk::PropagationPhase::Capture);
    wheel.connect_scroll(glib::clone!(
        #[strong]
        pinned,
        move |_, _, dy| {
            if dy > 0.0 {
                pinned.set(false);
            }
            glib::Propagation::Proceed
        }
    ));
    scrolled.add_controller(wheel);
    let bar = gtk::GestureClick::new();
    bar.set_propagation_phase(gtk::PropagationPhase::Capture);
    bar.connect_pressed(glib::clone!(
        #[strong]
        pinned,
        move |_, _, _, _| pinned.set(false)
    ));
    scrolled.vscrollbar().add_controller(bar);
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed(glib::clone!(
        #[strong]
        pinned,
        move |_, key, _, _| {
            use gdk::Key;
            if [
                Key::Down,
                Key::Page_Down,
                Key::End,
                Key::KP_Down,
                Key::KP_Page_Down,
                Key::KP_End,
            ]
            .contains(&key)
            {
                pinned.set(false);
            }
            glib::Propagation::Proceed
        }
    ));
    view.add_controller(keys);
}

/// The row behind a list item: the item itself, or in a tree its row's item.
pub fn row_of(item: &glib::Object, tree: bool) -> Option<Row> {
    if tree {
        item.downcast_ref::<gtk::TreeListRow>()?
            .item()
            .and_downcast::<Row>()
    } else {
        item.downcast_ref::<Row>().cloned()
    }
}

fn row_property(property: &str) -> gtk::PropertyExpression {
    gtk::PropertyExpression::new(Row::static_type(), None::<gtk::Expression>, property)
}

/// The row a list item shows, as an expression.
fn row_expression(item: &gtk::ListItem, tree: bool) -> gtk::Expression {
    let expression = item.property_expression_weak("item");
    if tree {
        expression
            .chain_property::<gtk::TreeListRow>("item")
            .upcast()
    } else {
        expression.upcast()
    }
}

/// The shade a value gets: 0 (none) to 4.
fn shade(value: f64, thresholds: &[f64; 4]) -> usize {
    thresholds
        .iter()
        .take_while(|&&threshold| value >= threshold)
        .count()
}

fn cell_factory(
    cell: Cell,
    tree: bool,
    menu: MenuHandler,
    switched: SwitchHandler,
    selection: glib::WeakRef<gtk::SingleSelection>,
) -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(move |_, item| {
        let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let row = row_expression(item, tree);
        let widget: gtk::Widget = match cell {
            Cell::Name => {
                let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                content.add_css_class("tm-name");
                let image = gtk::Image::builder().pixel_size(16).build();
                row.chain_property::<Row>("icon")
                    .bind(&image, "gicon", None::<&glib::Object>);
                let label = gtk::Label::builder()
                    .xalign(0.0)
                    .ellipsize(gtk::pango::EllipsizeMode::End)
                    .build();
                row.chain_property::<Row>("name")
                    .bind(&label, "label", None::<&glib::Object>);
                row.chain_property::<Row>("name").bind(
                    &label,
                    "tooltip-text",
                    None::<&glib::Object>,
                );
                content.append(&image);
                content.append(&label);
                if tree {
                    let expander = gtk::TreeExpander::builder()
                        .child(&content)
                        .indent_for_icon(true)
                        .build();
                    item.property_expression_weak("item").bind(
                        &expander,
                        "list-row",
                        None::<&glib::Object>,
                    );
                    row.chain_property::<Row>("leaf").bind(
                        &expander,
                        "hide-expander",
                        None::<&glib::Object>,
                    );
                    expander.upcast()
                } else {
                    content.upcast()
                }
            }
            Cell::Text(property) => {
                let label = gtk::Label::builder()
                    .xalign(0.0)
                    .ellipsize(gtk::pango::EllipsizeMode::End)
                    .css_classes(["tm-cell"])
                    .build();
                row.chain_property::<Row>(property)
                    .bind(&label, "label", None::<&glib::Object>);
                label.upcast()
            }
            Cell::Number(property, heat) => {
                let label = gtk::Label::builder()
                    .xalign(1.0)
                    .css_classes(["tm-cell", "tm-number"])
                    .build();
                row.chain_property::<Row>(property)
                    .bind(&label, "label", None::<&glib::Object>);
                if let Some((value, thresholds)) = heat {
                    row.chain_property::<Row>(value)
                        .chain_closure_with_callback(move |values| {
                            let value: f64 =
                                values.get(1).and_then(|v| v.get().ok()).unwrap_or(0.0);
                            vec![
                                "tm-cell".to_string(),
                                "tm-number".to_string(),
                                format!("tm-heat-{}", shade(value, &thresholds)),
                            ]
                        })
                        .bind(&label, "css-classes", None::<&glib::Object>);
                }
                label.upcast()
            }
            Cell::Switch(state, text) => {
                let content = gtk::Box::new(gtk::Orientation::Horizontal, 10);
                content.add_css_class("tm-cell");
                let switch = gtk::Switch::builder().valign(gtk::Align::Center).build();
                row.chain_property::<Row>(state)
                    .bind(&switch, "active", None::<&glib::Object>);
                let label = gtk::Label::builder().xalign(0.0).build();
                row.chain_property::<Row>(text)
                    .bind(&label, "label", None::<&glib::Object>);
                let weak_item = item.downgrade();
                let switched = switched.clone();
                switch.connect_state_set(move |_, active| {
                    let row = weak_item
                        .upgrade()
                        .and_then(|item| item.item())
                        .and_then(|item| row_of(&item, tree));
                    if let Some(row) = row {
                        // Only the user's flips: the binding sets it to the
                        // row's own state
                        if row.enabled() != active
                            && let Some(handler) = switched.borrow().as_ref()
                        {
                            handler(&row, active);
                        }
                    }
                    glib::Propagation::Proceed
                });
                content.append(&switch);
                content.append(&label);
                content.upcast()
            }
        };
        attach_menu(&widget, item, tree, menu.clone(), selection.clone());
        item.set_child(Some(&widget));
    });
    factory
}

/// A right-click selects the row and opens its menu.
fn attach_menu(
    widget: &gtk::Widget,
    item: &gtk::ListItem,
    tree: bool,
    menu: MenuHandler,
    selection: glib::WeakRef<gtk::SingleSelection>,
) {
    let click = gtk::GestureClick::builder()
        .button(gdk::BUTTON_SECONDARY)
        .build();
    let weak_item = item.downgrade();
    click.connect_pressed(move |gesture, _, x, y| {
        let Some(item) = weak_item.upgrade() else {
            return;
        };
        let Some(row) = item.item().and_then(|object| row_of(&object, tree)) else {
            return;
        };
        let Some(cell) = gesture.widget() else { return };
        if let Some(selection) = selection.upgrade() {
            selection.set_selected(item.position());
        }
        if let Some(handler) = menu.borrow().as_ref() {
            handler(&row, &cell, x, y);
        }
    });
    widget.add_controller(click);
}
