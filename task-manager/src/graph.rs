//! The Performance page's graphs: the last minute of a reading, drawn as a
//! filled line over a grid, and a second, dashed line where a device has
//! two readings (a network's sending and receiving).

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{cairo, gdk};

/// One reading a second for a minute.
pub const POINTS: usize = 60;

pub type Color = (f64, f64, f64);

pub const CPU: Color = (0.157, 0.545, 0.980);
pub const MEMORY: Color = (0.604, 0.302, 0.792);
pub const DISK: Color = (0.302, 0.651, 0.047);
pub const NETWORK: Color = (0.824, 0.478, 0.122);
pub const GPU: Color = (0.078, 0.639, 0.659);

struct Data {
    series: Vec<VecDeque<f64>>,
    /// The top of the scale, or None to fit the readings
    fixed: Option<f64>,
    /// The smallest top for a fitted scale
    floor: f64,
    color: Color,
    views: Vec<gtk::DrawingArea>,
}

#[derive(Clone)]
pub struct Graph {
    data: Rc<RefCell<Data>>,
}

impl Graph {
    pub fn new(color: Color, series: usize, fixed: Option<f64>, floor: f64) -> Self {
        Graph {
            data: Rc::new(RefCell::new(Data {
                series: (0..series)
                    .map(|_| VecDeque::from(vec![0.0; POINTS]))
                    .collect(),
                fixed,
                floor,
                color,
                views: Vec::new(),
            })),
        }
    }

    /// A drawing of the graph; a graph can have several (the small one in
    /// the device list and the large one). A width or height below zero
    /// fills the space it's given.
    pub fn view(&self, grid: bool, width: i32, height: i32) -> gtk::DrawingArea {
        let area = gtk::DrawingArea::builder()
            .hexpand(width < 0)
            .vexpand(height < 0)
            .build();
        if width > 0 {
            area.set_content_width(width);
        }
        if height > 0 {
            area.set_content_height(height);
        }
        let data = self.data.clone();
        area.set_draw_func(move |area, cr, width, height| {
            draw(
                &data.borrow(),
                cr,
                width as f64,
                height as f64,
                grid,
                area.color(),
            );
        });
        self.data.borrow_mut().views.push(area.clone());
        area
    }

    pub fn push(&self, values: &[f64]) {
        let mut data = self.data.borrow_mut();
        for (series, &value) in data.series.iter_mut().zip(values) {
            series.push_back(value);
            while series.len() > POINTS {
                series.pop_front();
            }
        }
        // Drawings of a device that was listed again have left the window
        data.views.retain(|view| view.parent().is_some());
        for view in &data.views {
            if view.is_drawable() {
                view.queue_draw();
            }
        }
    }

    /// The top of the scale now.
    pub fn scale(&self) -> f64 {
        top(&self.data.borrow())
    }
}

fn top(data: &Data) -> f64 {
    if let Some(fixed) = data.fixed {
        return fixed;
    }
    let highest = data
        .series
        .iter()
        .flatten()
        .copied()
        .fold(0.0, f64::max)
        .max(data.floor);
    // Round up to 1, 2 or 5 times a power of ten
    let power = 10f64.powf(highest.log10().floor());
    [1.0, 2.0, 5.0, 10.0]
        .iter()
        .map(|step| step * power)
        .find(|&value| value >= highest)
        .unwrap_or(highest)
}

fn draw(data: &Data, cr: &cairo::Context, width: f64, height: f64, grid: bool, text: gdk::RGBA) {
    let (r, g, b) = data.color;
    cr.set_line_width(1.0);

    cr.set_source_rgba(r, g, b, 0.05);
    cr.rectangle(0.0, 0.0, width, height);
    let _ = cr.fill();

    if grid {
        cr.set_source_rgba(
            text.red() as f64,
            text.green() as f64,
            text.blue() as f64,
            0.09,
        );
        for step in 1..10 {
            let x = (width * step as f64 / 10.0).round() + 0.5;
            let y = (height * step as f64 / 10.0).round() + 0.5;
            cr.move_to(x, 0.0);
            cr.line_to(x, height);
            cr.move_to(0.0, y);
            cr.line_to(width, y);
        }
        let _ = cr.stroke();
    }

    let scale = top(data);
    for (index, series) in data.series.iter().enumerate() {
        let count = series.len();
        if count < 2 {
            continue;
        }
        let point = |i: usize, value: f64| -> (f64, f64) {
            let x = width * i as f64 / (count - 1) as f64;
            let y = height - (value / scale).clamp(0.0, 1.0) * (height - 1.0);
            (x, y)
        };
        cr.new_path();
        for (i, &value) in series.iter().enumerate() {
            let (x, y) = point(i, value);
            if i == 0 {
                cr.move_to(x, y);
            } else {
                cr.line_to(x, y);
            }
        }
        if index == 0 {
            let path = cr.copy_path();
            cr.line_to(width, height);
            cr.line_to(0.0, height);
            cr.close_path();
            cr.set_source_rgba(r, g, b, 0.18);
            let _ = cr.fill();
            if let Ok(path) = path {
                cr.append_path(&path);
            }
            cr.set_dash(&[], 0.0);
        } else {
            cr.set_dash(&[4.0, 3.0], 0.0);
        }
        cr.set_source_rgba(r, g, b, 1.0);
        cr.set_line_width(1.5);
        let _ = cr.stroke();
        cr.set_line_width(1.0);
    }
    cr.set_dash(&[], 0.0);

    cr.set_source_rgba(r, g, b, 0.7);
    cr.rectangle(0.5, 0.5, width - 1.0, height - 1.0);
    let _ = cr.stroke();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scales() {
        let graph = Graph::new(CPU, 1, None, 1.0);
        graph.push(&[3.2]);
        assert_eq!(graph.scale(), 5.0);
        graph.push(&[42.0]);
        assert_eq!(graph.scale(), 50.0);
        let fixed = Graph::new(CPU, 1, Some(1.0), 0.0);
        fixed.push(&[0.4]);
        assert_eq!(fixed.scale(), 1.0);
    }
}
