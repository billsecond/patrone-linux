//! The Performance page: a list of devices on the left (processors,
//! memory, each disk, each network adapter, graphics where the card says
//! how busy it is), each with a small graph, and the chosen device's graph
//! and figures on the right.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{cairo, glib};

use crate::format;
use crate::graph::{self, Graph};
use crate::model::Totals;
use crate::sample::{CpuInfo, Disk, Gpu, Memory, Net, Sample};

/// A device's history: kept when the list of devices changes.
struct Graphs {
    main: Graph,
    /// Each processor, or a disk's transfer rate
    more: Vec<Graph>,
}

enum Detail {
    Cpu,
    Memory {
        composition: Rc<RefCell<Memory>>,
        bar: gtk::DrawingArea,
    },
    Disk {
        rate_scale: gtk::Label,
    },
    Net {
        scale: gtk::Label,
    },
    Gpu,
}

struct Device {
    key: String,
    graphs: Rc<Graphs>,
    summary: gtk::Label,
    detail: Detail,
    /// The figures on its page, by caption
    values: HashMap<&'static str, gtk::Label>,
}

pub struct Page {
    pub widget: gtk::Paned,
    list: gtk::ListBox,
    stack: gtk::Stack,
    devices: RefCell<Vec<Device>>,
    info: CpuInfo,
}

impl Page {
    pub fn new() -> Rc<Self> {
        let list = gtk::ListBox::builder().css_classes(["tm-devices"]).build();
        let stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .hexpand(true)
            .build();
        let side = gtk::ScrolledWindow::builder()
            .child(&list)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .width_request(240)
            .css_classes(["tm-device-side"])
            .build();
        let detail = gtk::ScrolledWindow::builder()
            .child(&stack)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();
        let widget = gtk::Paned::builder()
            .start_child(&side)
            .end_child(&detail)
            .shrink_start_child(false)
            .resize_start_child(false)
            .position(250)
            .build();
        let page = Rc::new(Page {
            widget,
            list,
            stack,
            devices: RefCell::new(Vec::new()),
            info: CpuInfo::read(),
        });
        let stack = page.stack.clone();
        page.list.connect_row_selected(move |_, row| {
            if let Some(row) = row {
                stack.set_visible_child_name(&row.widget_name());
            }
        });
        page
    }

    pub fn update(&self, sample: &Sample, totals: &Totals) {
        let keys = device_keys(sample);
        let current: Vec<String> = self
            .devices
            .borrow()
            .iter()
            .map(|device| device.key.clone())
            .collect();
        if keys != current {
            self.rebuild(sample, &keys);
        }
        for device in self.devices.borrow().iter() {
            self.update_device(device, sample, totals);
        }
    }

    /// The devices changed (a disk or adapter came or went): lists them
    /// again, keeping each one's history and the one shown.
    fn rebuild(&self, sample: &Sample, keys: &[String]) {
        let shown = self
            .list
            .selected_row()
            .map(|row| row.widget_name().to_string());
        while let Some(row) = self.list.row_at_index(0) {
            self.list.remove(&row);
        }
        while let Some(child) = self.stack.first_child() {
            self.stack.remove(&child);
        }
        let mut old: HashMap<String, Rc<Graphs>> = self
            .devices
            .borrow_mut()
            .drain(..)
            .map(|device| (device.key, device.graphs))
            .collect();
        let mut devices = Vec::new();
        for key in keys {
            let graphs = old
                .remove(key)
                .unwrap_or_else(|| Rc::new(new_graphs(key, sample)));
            let mut device = Device {
                key: key.clone(),
                graphs,
                summary: gtk::Label::builder()
                    .xalign(0.0)
                    .css_classes(["tm-device-summary"])
                    .build(),
                detail: match kind_of(key) {
                    "cpu" => Detail::Cpu,
                    "memory" => Detail::Memory {
                        composition: Rc::new(RefCell::new(sample.memory.clone())),
                        bar: gtk::DrawingArea::new(),
                    },
                    "disk" => Detail::Disk {
                        rate_scale: gtk::Label::new(None),
                    },
                    "net" => Detail::Net {
                        scale: gtk::Label::new(None),
                    },
                    _ => Detail::Gpu,
                },
                values: HashMap::new(),
            };

            let row_box = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            row_box.append(&device.graphs.main.view(false, 64, 44));
            let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
            text.set_valign(gtk::Align::Center);
            text.append(
                &gtk::Label::builder()
                    .label(title_of(key, sample))
                    .xalign(0.0)
                    .css_classes(["tm-device-title"])
                    .build(),
            );
            text.append(&device.summary);
            row_box.append(&text);
            let row = gtk::ListBoxRow::builder().child(&row_box).build();
            row.set_widget_name(key);
            self.list.append(&row);

            let page = self.detail_page(&mut device, sample);
            self.stack.add_named(&page, Some(key));
            devices.push(device);
        }
        *self.devices.borrow_mut() = devices;
        let wanted = shown
            .filter(|key| keys.contains(key))
            .unwrap_or_else(|| keys[0].clone());
        let mut index = 0;
        while let Some(row) = self.list.row_at_index(index) {
            if row.widget_name() == wanted {
                self.list.select_row(Some(&row));
            }
            index += 1;
        }
    }

    /// A device's page on the right: its name, its graphs, its figures.
    fn detail_page(&self, device: &mut Device, sample: &Sample) -> gtk::Box {
        let key = device.key.clone();
        let page = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .css_classes(["tm-device-page"])
            .build();
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let title = title_of(&key, sample);
        let heading = title.split(" (").next().unwrap_or(&title).to_string();
        header.append(
            &gtk::Label::builder()
                .label(&heading)
                .xalign(0.0)
                .hexpand(true)
                .css_classes(["tm-device-heading"])
                .build(),
        );
        let subtitle = match kind_of(&key) {
            "cpu" => self.info.model.clone(),
            "memory" => format::bytes(sample.memory.total as f64),
            "disk" => disk_of(&key, sample)
                .map(|disk| disk.model.clone())
                .unwrap_or_default(),
            "net" => net_of(&key, sample)
                .map(|net| net.name.clone())
                .unwrap_or_default(),
            _ => gpu_of(&key, sample)
                .map(|gpu| gpu.name.clone())
                .unwrap_or_default(),
        };
        header.append(
            &gtk::Label::builder()
                .label(&subtitle)
                .xalign(1.0)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .css_classes(["tm-device-model"])
                .build(),
        );
        page.append(&header);

        // The figures, and the facts beside them, or below them in a
        // narrow window
        let stats = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .min_children_per_line(1)
            .max_children_per_line(2)
            .column_spacing(48)
            .row_spacing(16)
            .margin_top(12)
            .css_classes(["tm-stats"])
            .build();
        let big = gtk::Grid::builder()
            .column_spacing(32)
            .row_spacing(12)
            .valign(gtk::Align::Start)
            .build();
        let info = gtk::Grid::builder()
            .column_spacing(16)
            .row_spacing(4)
            .valign(gtk::Align::Start)
            .build();
        stats.append(&big);
        stats.append(&info);
        let values = &mut device.values;
        let graphs = device.graphs.clone();

        match &device.detail {
            Detail::Cpu => {
                let overall = graph_block(
                    &graphs.main,
                    "% Utilization",
                    &label("100%"),
                    "60 seconds",
                    "0",
                );
                let each = gtk::Grid::builder()
                    .row_spacing(4)
                    .column_spacing(4)
                    .hexpand(true)
                    .vexpand(true)
                    .row_homogeneous(true)
                    .column_homogeneous(true)
                    .build();
                let columns = (graphs.more.len() as f64).sqrt().ceil().max(1.0) as i32;
                for (index, core) in graphs.more.iter().enumerate() {
                    let view = core.view(true, -1, -1);
                    view.set_size_request(60, 40);
                    view.set_tooltip_text(Some(&format!("Processor {index}")));
                    each.attach(&view, index as i32 % columns, index as i32 / columns, 1, 1);
                }
                let switcher = gtk::Stack::builder().vexpand(true).build();
                switcher.add_named(&overall, Some("overall"));
                switcher.add_named(&each, Some("each"));
                let toggle = gtk::ToggleButton::builder()
                    .label("Each processor")
                    .valign(gtk::Align::Center)
                    .css_classes(["tm-small-toggle"])
                    .build();
                toggle.connect_toggled(glib::clone!(
                    #[weak]
                    switcher,
                    move |toggle| switcher.set_visible_child_name(if toggle.is_active() {
                        "each"
                    } else {
                        "overall"
                    })
                ));
                header.append(&toggle);
                page.append(&switcher);
                for (column, row, name) in [
                    (0, 0, "Utilization"),
                    (1, 0, "Speed"),
                    (0, 1, "Processes"),
                    (1, 1, "Threads"),
                    (2, 1, "Open files"),
                    (0, 2, "Up time"),
                ] {
                    values.insert(name, big_stat(&big, column, row, name));
                }
                let mut lines: Vec<(&str, String)> = vec![
                    (
                        "Base speed",
                        self.info
                            .base_mhz
                            .map(format::frequency)
                            .unwrap_or_else(|| "Unknown".into()),
                    ),
                    ("Sockets", self.info.sockets.to_string()),
                    ("Cores", self.info.cores.to_string()),
                    ("Logical processors", self.info.logical.to_string()),
                    (
                        "Virtualization",
                        if self.info.virtualization {
                            "Available"
                        } else {
                            "Not available"
                        }
                        .into(),
                    ),
                    (
                        "Virtual machine",
                        if self.info.virtual_machine {
                            "Yes"
                        } else {
                            "No"
                        }
                        .into(),
                    ),
                ];
                for (level, size) in ["L1 cache", "L2 cache", "L3 cache"]
                    .into_iter()
                    .zip(self.info.caches)
                {
                    if let Some(size) = size {
                        lines.push((level, format::bytes(size as f64)));
                    }
                }
                for (row, (caption, value)) in lines.into_iter().enumerate() {
                    info_line(&info, row as i32, caption, &value);
                }
            }
            Detail::Memory { composition, bar } => {
                let total = format::bytes(sample.memory.total as f64);
                page.append(&graph_block(
                    &graphs.main,
                    "Memory usage",
                    &label(&total),
                    "60 seconds",
                    "0",
                ));
                page.append(
                    &gtk::Label::builder()
                        .label("Memory composition")
                        .xalign(0.0)
                        .margin_top(8)
                        .css_classes(["tm-graph-caption"])
                        .build(),
                );
                bar.set_content_height(32);
                bar.set_hexpand(true);
                let composition = composition.clone();
                bar.set_draw_func(move |_, cr, width, height| {
                    draw_composition(&composition.borrow(), cr, width as f64, height as f64);
                });
                bar.set_tooltip_text(Some(
                    "In use; modified, waiting to be written to disk; cached, kept for later and given up when needed; and free",
                ));
                page.append(bar);
                for (column, row, name) in [
                    (0, 0, "In use"),
                    (1, 0, "Available"),
                    (0, 1, "Committed"),
                    (1, 1, "Cached"),
                    (0, 2, "Kernel"),
                    (1, 2, "Swap"),
                ] {
                    values.insert(name, big_stat(&big, column, row, name));
                }
                info_line(&info, 0, "Total", &total);
                info_line(
                    &info,
                    1,
                    "Swap size",
                    &format::bytes(sample.memory.swap_total as f64),
                );
                values.insert("Shared", info_line(&info, 2, "Shared", ""));
            }
            Detail::Disk { rate_scale } => {
                page.append(&graph_block(
                    &graphs.main,
                    "Active time",
                    &label("100%"),
                    "60 seconds",
                    "0",
                ));
                rate_scale.add_css_class("tm-graph-caption");
                let block = graph_block(
                    &graphs.more[0],
                    "Disk transfer rate (solid: read, dashed: write)",
                    rate_scale,
                    "60 seconds",
                    "0",
                );
                block.set_margin_top(8);
                page.append(&block);
                for (column, row, name) in [
                    (0, 0, "Active time"),
                    (1, 0, "Average response time"),
                    (0, 1, "Read speed"),
                    (1, 1, "Write speed"),
                ] {
                    values.insert(name, big_stat(&big, column, row, name));
                }
                if let Some(disk) = disk_of(&key, sample) {
                    info_line(&info, 0, "Capacity", &format::bytes(disk.capacity as f64));
                    info_line(
                        &info,
                        1,
                        "System disk",
                        if disk.system { "Yes" } else { "No" },
                    );
                    info_line(&info, 2, "Type", disk.kind);
                    info_line(&info, 3, "Device", &format!("/dev/{}", disk.name));
                }
            }
            Detail::Net { scale } => {
                scale.add_css_class("tm-graph-caption");
                page.append(&graph_block(
                    &graphs.main,
                    "Throughput (solid: receive, dashed: send)",
                    scale,
                    "60 seconds",
                    "0",
                ));
                for (column, row, name) in [(0, 0, "Send"), (1, 0, "Receive")] {
                    values.insert(name, big_stat(&big, column, row, name));
                }
                if let Some(net) = net_of(&key, sample) {
                    let speed = match net.speed {
                        Some(speed) if speed >= 1000 => {
                            format!("{:.1} Gbps", speed as f64 / 1000.0)
                        }
                        Some(speed) => format!("{speed} Mbps"),
                        None => "Unknown".into(),
                    };
                    info_line(&info, 0, "Adapter name", &net.name);
                    info_line(&info, 1, "Connection type", net.kind);
                    info_line(&info, 2, "Link speed", &speed);
                    values.insert("IPv4 address", info_line(&info, 3, "IPv4 address", ""));
                    values.insert("IPv6 address", info_line(&info, 4, "IPv6 address", ""));
                }
            }
            Detail::Gpu => {
                page.append(&graph_block(
                    &graphs.main,
                    "Utilization",
                    &label("100%"),
                    "60 seconds",
                    "0",
                ));
                for (column, row, name) in [(0, 0, "Utilization"), (1, 0, "Dedicated memory")] {
                    values.insert(name, big_stat(&big, column, row, name));
                }
            }
        }
        page.append(&stats);
        page
    }

    fn update_device(&self, device: &Device, sample: &Sample, totals: &Totals) {
        let set = |name: &str, text: &str| {
            if let Some(label) = device.values.get(name)
                && label.label() != text
            {
                label.set_label(text);
            }
        };
        let set_summary = |text: &str| {
            if device.summary.label() != text {
                device.summary.set_label(text);
            }
        };
        let graphs = &device.graphs;
        match &device.detail {
            Detail::Cpu => {
                graphs.main.push(&[sample.cpu]);
                for (graph, &value) in graphs.more.iter().zip(&sample.cores) {
                    graph.push(&[value]);
                }
                let speed = sample.mhz.map(format::frequency).unwrap_or_default();
                set_summary(&format!("{:.0}%  {speed}", sample.cpu * 100.0));
                set("Utilization", &format!("{:.0}%", sample.cpu * 100.0));
                set("Speed", &speed);
                set("Processes", &format::grouped(totals.processes as u64));
                set("Threads", &format::grouped(totals.threads));
                set("Open files", &format::grouped(sample.open_files));
                set("Up time", &format::uptime(sample.uptime));
            }
            Detail::Memory { composition, bar } => {
                let memory = &sample.memory;
                graphs.main.push(&[memory.used() as f64]);
                *composition.borrow_mut() = memory.clone();
                if bar.is_drawable() {
                    bar.queue_draw();
                }
                let percent = memory.used() as f64 / memory.total.max(1) as f64 * 100.0;
                set_summary(&format!(
                    "{}/{} ({percent:.0}%)",
                    format::bytes(memory.used() as f64).trim_end_matches(" GB"),
                    format::bytes(memory.total as f64)
                ));
                set("In use", &format::bytes(memory.used() as f64));
                set("Available", &format::bytes(memory.available as f64));
                set(
                    "Committed",
                    &format!(
                        "{}/{}",
                        format::bytes(memory.committed as f64),
                        format::bytes(memory.commit_limit as f64)
                    ),
                );
                set("Cached", &format::bytes(memory.cached as f64));
                set("Kernel", &format::bytes(memory.kernel as f64));
                set(
                    "Swap",
                    &if memory.swap_total == 0 {
                        "None".to_string()
                    } else {
                        format!(
                            "{} used",
                            format::bytes(memory.swap_total.saturating_sub(memory.swap_free) as f64)
                        )
                    },
                );
                set("Shared", &format::bytes(memory.shared as f64));
            }
            Detail::Disk { rate_scale } => {
                let Some(disk) = disk_of(&device.key, sample) else {
                    return;
                };
                graphs.main.push(&[disk.active]);
                graphs.more[0].push(&[disk.read, disk.write]);
                let scale = format::bytes_rate(graphs.more[0].scale());
                if rate_scale.label() != scale {
                    rate_scale.set_label(&scale);
                }
                set_summary(&format!("{}\n{:.0}%", disk.kind, disk.active * 100.0));
                set("Active time", &format!("{:.0}%", disk.active * 100.0));
                set("Average response time", &format!("{:.1} ms", disk.response));
                set("Read speed", &format::bytes_rate(disk.read));
                set("Write speed", &format::bytes_rate(disk.write));
            }
            Detail::Net { scale } => {
                let Some(net) = net_of(&device.key, sample) else {
                    return;
                };
                graphs.main.push(&[net.receive, net.send]);
                let top = format::bits_rate(graphs.main.scale());
                if scale.label() != top {
                    scale.set_label(&top);
                }
                set_summary(&format!(
                    "{}\nS: {}  R: {}",
                    net.kind,
                    format::bits_rate(net.send),
                    format::bits_rate(net.receive)
                ));
                set("Send", &format::bits_rate(net.send));
                set("Receive", &format::bits_rate(net.receive));
                set("IPv4 address", &net.ipv4.join("\n"));
                set("IPv6 address", &net.ipv6.join("\n"));
            }
            Detail::Gpu => {
                let Some(gpu) = gpu_of(&device.key, sample) else {
                    return;
                };
                graphs.main.push(&[gpu.busy]);
                set_summary(&format!("{:.0}%", gpu.busy * 100.0));
                set("Utilization", &format!("{:.0}%", gpu.busy * 100.0));
                set(
                    "Dedicated memory",
                    &match (gpu.memory_used, gpu.memory_total) {
                        (Some(used), Some(total)) => format!(
                            "{}/{}",
                            format::bytes(used as f64),
                            format::bytes(total as f64)
                        ),
                        _ => "Unknown".into(),
                    },
                );
            }
        }
    }
}

fn new_graphs(key: &str, sample: &Sample) -> Graphs {
    match kind_of(key) {
        "cpu" => Graphs {
            main: Graph::new(graph::CPU, 1, Some(1.0), 0.0),
            more: (0..sample.cores.len())
                .map(|_| Graph::new(graph::CPU, 1, Some(1.0), 0.0))
                .collect(),
        },
        "memory" => Graphs {
            main: Graph::new(
                graph::MEMORY,
                1,
                Some(sample.memory.total.max(1) as f64),
                0.0,
            ),
            more: Vec::new(),
        },
        "disk" => Graphs {
            main: Graph::new(graph::DISK, 1, Some(1.0), 0.0),
            // Read and write, fitted to at least 100 KB/s
            more: vec![Graph::new(graph::DISK, 2, None, 100.0 * 1024.0)],
        },
        // Receive and send, fitted to at least 100 Kbps
        "net" => Graphs {
            main: Graph::new(graph::NETWORK, 2, None, 12_500.0),
            more: Vec::new(),
        },
        _ => Graphs {
            main: Graph::new(graph::GPU, 1, Some(1.0), 0.0),
            more: Vec::new(),
        },
    }
}

fn device_keys(sample: &Sample) -> Vec<String> {
    let mut keys = vec!["cpu".to_string(), "memory".to_string()];
    keys.extend(
        sample
            .disks
            .iter()
            .map(|disk| format!("disk:{}", disk.name)),
    );
    keys.extend(sample.nets.iter().map(|net| format!("net:{}", net.name)));
    keys.extend((0..sample.gpus.len()).map(|index| format!("gpu:{index}")));
    keys
}

fn kind_of(key: &str) -> &str {
    key.split(':').next().unwrap_or(key)
}

fn disk_of<'a>(key: &str, sample: &'a Sample) -> Option<&'a Disk> {
    let name = key.strip_prefix("disk:")?;
    sample.disks.iter().find(|disk| disk.name == name)
}

fn net_of<'a>(key: &str, sample: &'a Sample) -> Option<&'a Net> {
    let name = key.strip_prefix("net:")?;
    sample.nets.iter().find(|net| net.name == name)
}

fn gpu_of<'a>(key: &str, sample: &'a Sample) -> Option<&'a Gpu> {
    sample
        .gpus
        .get(key.strip_prefix("gpu:")?.parse::<usize>().ok()?)
}

/// A device's name in the list: "CPU", "Disk 0 (sda)", "Ethernet (eth0)".
fn title_of(key: &str, sample: &Sample) -> String {
    match kind_of(key) {
        "cpu" => "CPU".into(),
        "memory" => "Memory".into(),
        "disk" => {
            let name = key.trim_start_matches("disk:");
            let index = sample
                .disks
                .iter()
                .position(|disk| disk.name == name)
                .unwrap_or(0);
            format!("Disk {index} ({name})")
        }
        "net" => {
            let kind = net_of(key, sample).map(|net| net.kind).unwrap_or("Network");
            format!("{kind} ({})", key.trim_start_matches("net:"))
        }
        _ => format!("GPU {}", key.trim_start_matches("gpu:")),
    }
}

fn label(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .css_classes(["tm-graph-caption"])
        .build()
}

/// A graph with its captions: what it shows and its top above, the minute
/// and zero below.
fn graph_block(
    graph: &Graph,
    caption: &str,
    top: &gtk::Label,
    left: &str,
    right: &str,
) -> gtk::Box {
    let block = gtk::Box::new(gtk::Orientation::Vertical, 2);
    block.set_vexpand(true);
    let above = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    above.append(
        &gtk::Label::builder()
            .label(caption)
            .xalign(0.0)
            .hexpand(true)
            .css_classes(["tm-graph-caption"])
            .build(),
    );
    top.set_xalign(1.0);
    above.append(top);
    block.append(&above);
    let view = graph.view(true, -1, -1);
    view.set_size_request(-1, 150);
    block.append(&view);
    let below = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    below.append(
        &gtk::Label::builder()
            .label(left)
            .xalign(0.0)
            .hexpand(true)
            .css_classes(["tm-graph-caption"])
            .build(),
    );
    below.append(
        &gtk::Label::builder()
            .label(right)
            .css_classes(["tm-graph-caption"])
            .build(),
    );
    block.append(&below);
    block
}

/// A figure in large type under its caption.
fn big_stat(grid: &gtk::Grid, column: i32, row: i32, caption: &str) -> gtk::Label {
    let block = gtk::Box::new(gtk::Orientation::Vertical, 0);
    block.append(
        &gtk::Label::builder()
            .label(caption)
            .xalign(0.0)
            .css_classes(["tm-stat-caption"])
            .build(),
    );
    let value = gtk::Label::builder()
        .xalign(0.0)
        .selectable(true)
        .css_classes(["tm-stat-value"])
        .build();
    block.append(&value);
    grid.attach(&block, column, row, 1, 1);
    value
}

/// A fact in small type beside its caption; returns the value's label.
fn info_line(grid: &gtk::Grid, row: i32, caption: &str, value: &str) -> gtk::Label {
    grid.attach(
        &gtk::Label::builder()
            .label(format!("{caption}:"))
            .xalign(0.0)
            .yalign(0.0)
            .css_classes(["tm-info-caption"])
            .build(),
        0,
        row,
        1,
        1,
    );
    let label = gtk::Label::builder()
        .label(value)
        .xalign(0.0)
        .selectable(true)
        .css_classes(["tm-info-value"])
        .build();
    grid.attach(&label, 1, row, 1, 1);
    label
}

/// The memory composition bar: in use, modified, cached, free.
fn draw_composition(memory: &Memory, cr: &cairo::Context, width: f64, height: f64) {
    let (r, g, b) = graph::MEMORY;
    let total = memory.total.max(1) as f64;
    let free = memory.free as f64;
    let cached = memory.cached as f64;
    let modified = memory.modified as f64;
    let in_use = (total - free - cached - modified).max(0.0);
    let mut x = 0.5;
    for (value, alpha) in [
        (in_use, 0.45),
        (modified, 0.75),
        (cached, 0.16),
        (free, 0.0),
    ] {
        let w = (width - 1.0) * value / total;
        cr.set_source_rgba(r, g, b, alpha);
        cr.rectangle(x, 0.5, w, height - 1.0);
        let _ = cr.fill();
        if x > 1.0 {
            cr.set_source_rgba(r, g, b, 0.8);
            cr.move_to(x.round() + 0.5, 0.0);
            cr.line_to(x.round() + 0.5, height);
            let _ = cr.stroke();
        }
        x += w;
    }
    cr.set_source_rgba(r, g, b, 0.8);
    cr.rectangle(0.5, 0.5, width - 1.0, height - 1.0);
    let _ = cr.stroke();
}
