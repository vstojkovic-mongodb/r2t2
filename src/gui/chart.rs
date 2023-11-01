use std::ops::{RangeInclusive, Sub};

use fltk::draw;
use fltk::enums::{Align, Color, Font};
use thousands::Separable;

use crate::ftdc::{unix_millis_to_timestamp, Timestamp};

pub type DataPoint = (Timestamp, f64);

#[derive(Debug)]
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

#[derive(Debug)]
pub struct ChartData {
    pub points: Vec<DataPoint>,
}

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

pub fn draw_time_axis(
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    time_axis: &TimeAxis,
    style: &ChartStyle,
    draw_header: bool,
) {
    draw::set_font(style.time_text_font.0, style.time_text_font.1);
    let text_h = draw::height();

    let xform = CoordTransform::from_time_axis(time_axis, x, w);
    let mut last_tick: Option<Timestamp> = None;
    for tick in time_axis.ticks.iter() {
        let tick_x = xform.transform(*tick);

        draw::set_draw_color(style.time_tick_color);
        draw::draw_line(tick_x, y, tick_x, y + h - 1);

        if draw_header {
            draw::set_draw_color(style.time_text_color);
            if last_tick
                .map(|t| t.date_naive() != tick.date_naive())
                .unwrap_or(true)
            {
                let text = tick.format("%Y-%m-%d").to_string();
                let (text_w, _) = draw::measure(&text, false);
                draw::draw_text2(
                    &text,
                    tick_x - text_w / 2,
                    y + text_h,
                    text_w,
                    text_h,
                    Align::Center,
                );
            }

            let text = tick.format("%H:%M:%S").to_string();
            let (text_w, _) = draw::measure(&text, false);
            draw::draw_text2(
                &text,
                tick_x - text_w / 2,
                y + text_h * 2,
                text_w,
                text_h,
                Align::Center,
            );

            last_tick = Some(*tick);
        }
    }
}

pub fn draw_value_axis(x: i32, y: i32, w: i32, h: i32, value_axis: &ValueAxis, style: &ChartStyle) {
    draw::set_font(style.value_text_font.0, style.value_text_font.1);

    let xform = CoordTransform::from_value_axis(value_axis, y, h);
    for tick in value_axis.ticks.iter() {
        let tick_y = xform.transform(*tick);

        draw::set_draw_color(style.value_tick_color);
        draw::draw_line(x, tick_y, x + w - 1, tick_y);

        draw::set_draw_color(style.value_text_color);
        let tick = (tick * 1000.0).round() / 1000.0;
        let text = format!("{} ", tick).separate_with_commas();
        let (text_w, text_h) = draw::measure(&text, false);
        draw::draw_text2(
            &text,
            x - text_w,
            tick_y - text_h / 2,
            text_w,
            text_h,
            Align::Right,
        );
    }
}

pub fn draw_data_line(
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    time_axis: &TimeAxis,
    value_axis: &ValueAxis,
    data: &ChartData,
    style: &ChartStyle,
) {
    if data.points.is_empty() {
        return;
    }

    let xform = PointTransform::new(x, y, w, h, time_axis, value_axis);

    draw::set_draw_color(style.data_line_color);
    draw::begin_line();

    for pt in data.points.iter() {
        let (pt_x, pt_y) = xform.transform(pt);
        draw::vertex(pt_x as _, pt_y as _);
    }

    draw::end_line();
}

pub fn draw_data_fill(
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    time_axis: &TimeAxis,
    value_axis: &ValueAxis,
    data: &ChartData,
    style: &ChartStyle,
) {
    if data.points.is_empty() {
        return;
    }

    let xform = PointTransform::new(x, y, w, h, time_axis, value_axis);

    draw::set_draw_color(style.data_fill_color);
    draw::begin_complex_polygon();

    let (left_bottom_x, _) = xform.transform(data.points.first().unwrap());
    draw::vertex(left_bottom_x as _, xform.value_xform.coord_origin as _);

    for pt in data.points.iter() {
        let (pt_x, pt_y) = xform.transform(pt);
        draw::vertex(pt_x as _, pt_y as _);
    }

    let (right_bottom_x, _) = xform.transform(data.points.last().unwrap());
    draw::vertex(right_bottom_x as _, xform.value_xform.coord_origin as _);

    draw::end_complex_polygon();
}

trait CoordInterpolate: Sub + Copy {
    fn interpolate(self, min: Self, span: Self::Output, coord_origin: i32, coord_span: i32) -> i32;
}

impl CoordInterpolate for f64 {
    fn interpolate(self, min: Self, span: Self::Output, coord_origin: i32, coord_span: i32) -> i32 {
        coord_origin + ((self - min) * coord_span as Self / span) as i32
    }
}

impl CoordInterpolate for i64 {
    fn interpolate(self, min: Self, span: Self::Output, coord_origin: i32, coord_span: i32) -> i32 {
        coord_origin + ((self - min) * coord_span as Self / span) as i32
    }
}

impl CoordInterpolate for Timestamp {
    fn interpolate(self, min: Self, span: Self::Output, coord_origin: i32, coord_span: i32) -> i32 {
        self.timestamp_millis().interpolate(
            min.timestamp_millis(),
            span.num_milliseconds(),
            coord_origin,
            coord_span,
        )
    }
}

struct CoordTransform<D: CoordInterpolate>
where
    D::Output: Copy,
{
    domain_min: D,
    domain_span: D::Output,
    coord_origin: i32,
    coord_span: i32,
}

impl<D: CoordInterpolate> CoordTransform<D>
where
    D::Output: Copy,
{
    fn transform(&self, domain_value: D) -> i32 {
        domain_value.interpolate(
            self.domain_min,
            self.domain_span,
            self.coord_origin,
            self.coord_span,
        )
    }
}

impl CoordTransform<Timestamp> {
    fn from_time_axis(time_axis: &TimeAxis, x: i32, w: i32) -> Self {
        let domain_min = *time_axis.range.start();
        let domain_span = *time_axis.range.end() - domain_min;
        let coord_origin = x;
        let coord_span = w - 1;
        Self { domain_min, domain_span, coord_origin, coord_span }
    }
}

impl CoordTransform<f64> {
    fn from_value_axis(value_axis: &ValueAxis, y: i32, h: i32) -> Self {
        let domain_min = *value_axis.range.start();
        let domain_span = *value_axis.range.end() - domain_min;
        let coord_origin = y + h - 1;
        let coord_span = -(h - 1);
        Self { domain_min, domain_span, coord_origin, coord_span }
    }
}

struct PointTransform {
    time_xform: CoordTransform<Timestamp>,
    value_xform: CoordTransform<f64>,
}

impl PointTransform {
    fn new(x: i32, y: i32, w: i32, h: i32, time_axis: &TimeAxis, value_axis: &ValueAxis) -> Self {
        Self {
            time_xform: CoordTransform::from_time_axis(time_axis, x, w),
            value_xform: CoordTransform::from_value_axis(value_axis, y, h),
        }
    }

    fn transform(&self, point: &DataPoint) -> (i32, i32) {
        (
            self.time_xform.transform(point.0),
            self.value_xform.transform(point.1),
        )
    }
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
