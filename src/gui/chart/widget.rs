use std::cell::RefCell;
use std::ops::RangeInclusive;
use std::rc::Rc;

use fltk::enums::{Align, Color, Font};
use fltk::prelude::*;
use fltk::table::{Table, TableContext};
use fltk::widget::Widget;

use crate::ftdc::{MetricKey, Timestamp};

use super::{
    calculate_time_ticks, calculate_value_ticks, draw_data_fill, draw_data_line,
    draw_time_tick_labels, draw_time_tick_lines, draw_value_tick_labels, draw_value_tick_lines,
    ChartData, ChartStyle, DataPoint, TimeAxis, ValueAxis,
};

#[derive(Clone)]
pub struct ChartListView {
    table: Table,
    state: Rc<RefCell<ChartListState>>,
}

struct ChartListState {
    style: ChartStyle,
    chart_height: i32,
    chart_spacing: i32,
    chart_margin: i32,
    max_time_ticks: usize,  // TODO: Add setter
    max_value_ticks: usize, // TODO: Add setter
    time_axis: Option<TimeAxis>,
    rows: Vec<ChartListRow>,
}

struct ChartListRow {
    key: MetricKey,
    value_axis: ValueAxis,
    data: ChartData,
}

impl Default for ChartListView {
    fn default() -> Self {
        Self::new(0, 0, 0, 0)
    }
}

impl ChartListView {
    pub fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        let mut table = Table::new(x, y, w, h, "");
        table.set_cols(2);
        table.set_rows(0);
        table.set_col_header(true);
        table.set_row_header(true);
        table.set_color(Color::Background2);

        table.set_col_width(0, 410);
        table.set_row_header_width(100);
        table.set_col_header_height(100);

        let state = Rc::new(RefCell::new(ChartListState {
            style: Default::default(),
            chart_height: 120,
            chart_spacing: 20,
            chart_margin: 10,
            max_time_ticks: 6,
            max_value_ticks: 5,
            time_axis: None,
            rows: Default::default(),
        }));

        table.draw_cell({
            let state = Rc::clone(&state);
            move |table, ctx, row, col, x, y, w, h| {
                draw_cell(table, &state, ctx, row, col, x, y, w, h)
            }
        });

        Self { table, state }
    }

    pub fn widget(&self) -> Widget {
        self.table.as_base_widget()
    }

    pub fn with_style(mut self, style: ChartStyle) -> Self {
        self.set_style(style);
        self
    }

    pub fn style(&self) -> ChartStyle {
        self.state.borrow().style.clone()
    }

    pub fn set_style(&mut self, style: ChartStyle) {
        self.state.borrow_mut().style = style;
        self.table.redraw();
    }

    pub fn set_time_range<R: Into<Option<RangeInclusive<Timestamp>>>>(&mut self, time_range: R) {
        let mut state = self.state.borrow_mut();
        state.time_axis = time_range.into().map(|range| TimeAxis {
            range: range.clone(),
            ticks: calculate_time_ticks(range, state.max_time_ticks),
        });
        drop(state);

        self.update_rows();
    }

    pub fn set_data(&mut self, data: Vec<(MetricKey, Vec<DataPoint>)>) {
        let mut state = self.state.borrow_mut();
        state.rows = data
            .into_iter()
            .map(|(key, points)| ChartListRow::new(key, points, state.max_value_ticks))
            .collect();
        drop(state);

        self.update_rows();
    }

    pub fn x(&self) -> i32 {
        self.table.x()
    }

    pub fn y(&self) -> i32 {
        self.table.y()
    }

    pub fn w(&self) -> i32 {
        self.table.w()
    }

    pub fn h(&self) -> i32 {
        self.table.h()
    }

    pub fn value_axis_width(&self) -> i32 {
        self.table.row_header_width()
    }

    pub fn set_value_axis_width(&mut self, width: i32) {
        self.table.set_row_header_width(width);
        self.table.redraw();
    }

    pub fn time_axis_height(&self) -> i32 {
        self.table.col_header_height()
    }

    pub fn set_time_axis_height(&mut self, height: i32) {
        self.table.set_col_header_height(height);
        self.table.redraw();
    }

    pub fn chart_width(&self) -> i32 {
        self.table.col_width(0)
    }

    pub fn set_chart_width(&mut self, width: i32) {
        self.table.set_col_width(0, width);
        self.table.redraw();
    }

    pub fn set_chart_height(&mut self, height: i32) {
        self.state.borrow_mut().chart_height = height;
        self.table.set_row_height_all(height);
        self.table.redraw();
    }

    pub fn set_chart_gap(&mut self, gap: i32) {
        self.state.borrow_mut().chart_spacing = gap;
        self.table.redraw();
    }

    pub fn set_key_width(&mut self, width: i32) {
        self.table.set_col_width(1, width);
        self.table.redraw();
    }

    fn update_rows(&mut self) {
        let state = self.state.borrow();
        if state.time_axis.is_some() {
            self.table.set_rows(state.rows.len() as i32);
        } else {
            self.table.set_rows(0);
        }
        self.table
            .set_row_height_all(state.chart_height + state.chart_spacing);
        self.table.redraw();
    }
}

impl ChartListRow {
    fn new(key: MetricKey, points: Vec<DataPoint>, max_ticks: usize) -> Self {
        let max_value = points
            .iter()
            .map(|p| p.1)
            .max_by(f64::total_cmp)
            .unwrap_or_default();
        let ticks = calculate_value_ticks(max_value, max_ticks);

        let value_axis = ValueAxis { range: 0f64..=max_value, ticks };
        Self { key, value_axis, data: points }
    }
}

fn draw_cell(
    table: &Table,
    state: &Rc<RefCell<ChartListState>>,
    ctx: TableContext,
    row: i32,
    col: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) {
    let state = state.borrow();
    let chart_y = y + state.chart_spacing / 2;
    let chart_h = h - state.chart_spacing;
    let chart_w = w - state.chart_margin;

    fltk::draw::push_clip(x, y, w, h);

    match ctx {
        TableContext::ColHeader => {
            fltk::draw::draw_rect_fill(x, y, w, h, Color::Background2);
            if col == 0 {
                if let Some(time_axis) = state.time_axis.as_ref() {
                    draw_time_tick_lines(x, y, chart_w, h, time_axis, &state.style);
                    draw_time_tick_labels(x, y, chart_w, h, time_axis, &state.style);
                }
            }
        }
        TableContext::RowHeader => {
            fltk::draw::draw_rect_fill(x, y, w, h, Color::Background2);
            let row = &state.rows[row as usize];
            draw_value_tick_labels(x, chart_y, chart_w, chart_h, &row.value_axis, &state.style);
        }
        TableContext::Cell if col == 0 => {
            fltk::draw::draw_rect_fill(x, y, w, h, Color::Background2);
            if let Some(time_axis) = state.time_axis.as_ref() {
                let row = &state.rows[row as usize];
                draw_data_fill(
                    x,
                    chart_y,
                    chart_w,
                    chart_h,
                    time_axis,
                    &row.value_axis,
                    &row.data,
                    &state.style,
                );
                draw_time_tick_lines(x, y, chart_w, h, time_axis, &state.style);
                draw_value_tick_lines(x, chart_y, chart_w, chart_h, &row.value_axis, &state.style);
                draw_data_line(
                    x,
                    chart_y,
                    chart_w,
                    chart_h,
                    time_axis,
                    &row.value_axis,
                    &row.data,
                    &state.style,
                );
            }
        }
        TableContext::Cell if col == 1 => {
            fltk::draw::draw_rect_fill(x, y, w, h, Color::Background2);
            fltk::draw::set_font(table.label_font(), table.label_size());
            fltk::draw::set_draw_color(table.label_color());
            if state.time_axis.is_some() {
                let row = &state.rows[row as usize];
                let mut key = String::new();
                let mut first = true;
                for elem in row.key.iter() {
                    if first {
                        first = false;
                    } else {
                        key.push('\t');
                    }
                    key.push_str(elem);
                }
                fltk::draw::draw_text2(&key, x, y, w, h, Align::Left);
            }
        }
        _ => (),
    }

    fltk::draw::pop_clip();
}