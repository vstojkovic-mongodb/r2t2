use std::ops::RangeInclusive;

use fltk::enums::{Color, Font};

use crate::ftdc::{unix_millis_to_timestamp, Timestamp};

mod draw;
mod widget;

pub use self::draw::{
    draw_data_fill, draw_data_line, draw_time_tick_labels, draw_time_tick_lines,
    draw_value_tick_labels, draw_value_tick_lines,
};
pub use self::widget::ChartListView;

pub type DataPoint = (Timestamp, f64);

#[derive(Debug, Clone)]
pub struct ChartStyle {
    pub time_text_font: (Font, i32),
    pub time_text_color: Color,
    pub time_tick_color: Color,
    pub value_text_font: (Font, i32),
    pub value_text_color: Color,
    pub value_tick_color: Color,
    pub data_line_color: Color,
    pub data_fill_color: Color,
}

impl Default for ChartStyle {
    fn default() -> Self {
        Self {
            time_text_font: (Font::Helvetica, 12),
            time_text_color: Color::Foreground,
            time_tick_color: Color::Light1,
            value_text_font: (Font::Helvetica, 12),
            value_text_color: Color::Foreground,
            value_tick_color: Color::Light1,
            data_line_color: Color::Foreground,
            data_fill_color: Color::from_hex(0xeeeeee),
        }
    }
}

#[derive(Debug)]
pub struct TimeAxis {
    pub range: RangeInclusive<Timestamp>,
    pub ticks: Vec<Timestamp>,
}

#[derive(Debug)]
pub struct ValueAxis {
    pub range: RangeInclusive<f64>,
    pub ticks: Vec<f64>,
}

pub type ChartData = Vec<DataPoint>;

pub fn calculate_time_ticks(range: RangeInclusive<Timestamp>, max_ticks: usize) -> Vec<Timestamp> {
    let tick_delta = (*range.end() - *range.start()).num_milliseconds() / max_ticks as i64;
    let td = TIME_TICK_THRESHOLDS_MILLIS
        .into_iter()
        .find(|td| tick_delta < **td);
    let tick_delta = match td {
        Some(&td) => td,
        None => align_up_to(tick_delta, MILLIS_PER_DAY),
    };

    let start_millis = align_up_to(range.start().timestamp_millis(), tick_delta);
    let tick_delta = chrono::Duration::milliseconds(tick_delta);

    let mut ticks = Vec::with_capacity(max_ticks);
    let mut tick = unix_millis_to_timestamp(start_millis);
    while tick <= *range.end() {
        ticks.push(tick);
        tick += tick_delta;
    }
    ticks
}

pub fn calculate_value_ticks(max_value: f64, max_ticks: usize) -> Vec<f64> {
    let magnitude = 10f64.powf(max_value.log10().floor());
    let mut tick_delta = max_value / max_ticks as f64 / magnitude;
    for td in VALUE_TICK_THRESHOLDS {
        if tick_delta < *td {
            tick_delta = td * magnitude;
            break;
        }
    }

    let mut ticks = Vec::with_capacity(max_ticks);
    let mut tick = 0f64;
    while tick <= max_value {
        ticks.push(tick);
        tick += tick_delta;
    }
    ticks
}

fn align_up_to(value: i64, delta: i64) -> i64 {
    (value + delta - 1) / delta * delta
}

const MILLIS_PER_DAY: i64 = 86_400_000;
const TIME_TICK_THRESHOLDS_MILLIS: &[i64] = {
    const fn sec(s: i64) -> i64 {
        s * 1000
    }
    const fn min(m: i64) -> i64 {
        sec(m * 60)
    }
    const fn hr(h: i64) -> i64 {
        min(h * 60)
    }
    &[
        sec(1),
        sec(2),
        sec(5),
        sec(10),
        sec(15),
        sec(20),
        sec(30),
        sec(60),
        min(1),
        min(2) + sec(30),
        min(5),
        min(10),
        min(15),
        min(20),
        min(30),
        min(60),
        hr(1),
        hr(2),
        hr(3),
        hr(4),
        hr(6),
        hr(8),
        hr(12),
        hr(24),
    ]
};
const VALUE_TICK_THRESHOLDS: &[f64] = &[0.1, 0.2, 0.25, 0.5, 1.0, 2.0, 2.5, 5.0, 10.0];
