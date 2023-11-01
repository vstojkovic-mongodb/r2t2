use std::ops::{RangeInclusive, Sub};

use fltk::draw;
use fltk::enums::{Align, Color, Font};
use thousands::Separable;

use crate::ftdc::Timestamp;

pub type DataPoint = (Timestamp, f64);

#[derive(Debug)]
pub struct TimeAxis {
    pub range: RangeInclusive<Timestamp>,
}

#[derive(Debug)]
pub struct ValueAxis {
    pub range: RangeInclusive<f64>,
    pub ticks: Vec<f64>,
    pub font: (Font, i32),
    pub color: Color,
}

#[derive(Debug)]
pub struct ChartData {
    pub points: Vec<DataPoint>,
    pub color: Color,
    pub fill: Option<Color>,
}

pub fn draw_value_axis(x: i32, y: i32, w: i32, h: i32, value_axis: &ValueAxis) {
    draw::set_draw_color(value_axis.color);
    draw::set_font(value_axis.font.0, value_axis.font.1);

    let xform = CoordTransform::from_value_axis(value_axis, y, h);
    for tick in value_axis.ticks.iter() {
        let tick_y = xform.transform(*tick);

        draw::draw_line(x, tick_y, x + w - 1, tick_y);

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
) {
    if data.points.is_empty() {
        return;
    }

    let xform = PointTransform::new(x, y, w, h, time_axis, value_axis);

    draw::set_draw_color(data.color);
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
) {
    if data.points.is_empty() {
        return;
    }

    let xform = PointTransform::new(x, y, w, h, time_axis, value_axis);
    let fill = if let Some(fill) = data.fill {
        fill
    } else {
        return;
    };

    draw::set_draw_color(fill);
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
