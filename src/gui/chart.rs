use std::ops::RangeInclusive;

use fltk::draw;
use fltk::enums::Color;

use crate::ftdc::Timestamp;

pub type DataPoint = (Timestamp, i64);

#[derive(Debug)]
pub struct TimeAxis {
    pub range: RangeInclusive<Timestamp>,
}

#[derive(Debug)]
pub struct ValueAxis {
    pub range: RangeInclusive<i64>,
}

#[derive(Debug)]
pub struct ChartData {
    pub points: Vec<DataPoint>,
    pub color: Color,
}

pub fn draw_data(
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    time_axis: &TimeAxis,
    value_axis: &ValueAxis,
    data: &ChartData,
) {
    let t_min = time_axis.range.start().timestamp_millis();
    let t_diff = time_axis.range.end().timestamp_millis() - t_min;
    let v_min = value_axis.range.start();
    let v_diff = value_axis.range.end() - v_min;
    let x_diff = (w - 1) as i64;
    let y_diff = (h - 1) as i64;
    let y_max = y + h - 1;

    draw::set_draw_color(data.color);
    draw::begin_line();

    for (pt_time, pt_value) in data.points.iter() {
        let t_offset = pt_time.timestamp_millis() - t_min;
        let pt_x = x + (t_offset * x_diff / t_diff) as i32;

        let v_offset = pt_value - v_min;
        let pt_y = y_max - (v_offset * y_diff / v_diff) as i32;

        draw::vertex(pt_x as _, pt_y as _);
    }

    draw::end_line();
}
