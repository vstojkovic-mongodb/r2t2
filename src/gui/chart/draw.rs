use std::ops::Sub;

use fltk::draw;
use fltk::enums::Align;
use thousands::Separable;

use crate::ftdc::Timestamp;

use super::{ChartData, ChartStyle, DataPoint, TimeAxis, ValueAxis};

pub fn draw_time_tick_labels(
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    time_axis: &TimeAxis,
    style: &ChartStyle,
) {
    draw::set_font(style.time_text_font.0, style.time_text_font.1);
    draw::set_draw_color(style.time_text_color);

    let xform = CoordTransform::from_time_axis(time_axis, x, w);
    let mut last_tick: Option<Timestamp> = None;
    for tick in time_axis.ticks.iter() {
        let tick_x = xform.transform(*tick);

        let include_date = last_tick
            .map(|t| t.date_naive() != tick.date_naive())
            .unwrap_or(true);
        let fmt = if include_date { "%Y-%m-%d\n%H:%M:%S" } else { "\n%H:%M:%S" };

        let text = tick.format(fmt).to_string();
        let (text_w, _) = draw::measure(&text, false);
        draw::draw_text2(&text, tick_x - text_w / 2, y, text_w, h, Align::Center);

        last_tick = Some(*tick);
    }
}

pub fn draw_time_tick_lines(
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    time_axis: &TimeAxis,
    style: &ChartStyle,
) {
    draw::set_font(style.time_text_font.0, style.time_text_font.1);
    draw::set_draw_color(style.time_tick_color);

    let xform = CoordTransform::from_time_axis(time_axis, x, w);
    for tick in time_axis.ticks.iter() {
        let tick_x = xform.transform(*tick);
        draw::draw_line(tick_x, y, tick_x, y + h - 1);
    }
}

pub fn draw_value_tick_labels(
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    value_axis: &ValueAxis,
    style: &ChartStyle,
) {
    draw::set_font(style.value_text_font.0, style.value_text_font.1);
    draw::set_draw_color(style.value_text_color);

    let xform = CoordTransform::from_value_axis(value_axis, y, h);
    for tick in value_axis.ticks.iter() {
        let tick_y = xform.transform(*tick);

        let tick = (tick * 1000.0).round() / 1000.0;
        let text = format!("{} ", tick).separate_with_commas();
        let (_, text_h) = draw::measure(&text, false);
        draw::draw_text2(&text, x, tick_y - text_h / 2, w, text_h, Align::Right);
    }
}

pub fn draw_value_tick_lines(
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    value_axis: &ValueAxis,
    style: &ChartStyle,
) {
    draw::set_draw_color(style.value_tick_color);

    let xform = CoordTransform::from_value_axis(value_axis, y, h);
    for tick in value_axis.ticks.iter() {
        let tick_y = xform.transform(*tick);
        draw::draw_line(x, tick_y, x + w - 1, tick_y);
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
    if data.is_empty() {
        return;
    }

    let xform = PointTransform::new(x, y, w, h, time_axis, value_axis);

    draw::set_draw_color(style.data_line_color);
    draw::begin_line();

    for pt in data.iter() {
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
    if data.is_empty() {
        return;
    }

    let xform = PointTransform::new(x, y, w, h, time_axis, value_axis);

    draw::set_draw_color(style.data_fill_color);
    draw::begin_complex_polygon();

    let (left_bottom_x, _) = xform.transform(data.first().unwrap());
    draw::vertex(left_bottom_x as _, xform.value_xform.coord_origin as _);

    for pt in data.iter() {
        let (pt_x, pt_y) = xform.transform(pt);
        draw::vertex(pt_x as _, pt_y as _);
    }

    let (right_bottom_x, _) = xform.transform(data.last().unwrap());
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
