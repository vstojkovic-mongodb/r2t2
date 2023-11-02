use std::cell::RefCell;
use std::ops::RangeInclusive;
use std::rc::Rc;

use fltk::enums::{Align, Color};
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
    key_margin: i32,
    time_axis_height: i32,
    time_ticks: usize,
    value_axis_width: i32,
    value_ticks: usize,
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
        table.set_cols(3);
        table.set_rows(0);
        table.set_col_header(true);
        table.set_color(Color::Background2);

        let state = ChartListState {
            style: Default::default(),
            chart_height: 100,
            chart_spacing: 20,
            key_margin: 10,
            time_axis_height: 100,
            time_ticks: 6,
            value_axis_width: 100,
            value_ticks: 5,
            time_axis: None,
            rows: Default::default(),
        };

        table.set_col_width(0, state.value_axis_width);
        table.set_col_width(1, 400);
        table.set_col_header_height(state.time_axis_height);

        let state = Rc::new(RefCell::new(state));

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
        {
            self.state.borrow_mut().style = style;
        }
        self.table.redraw();
    }

    pub fn set_time_range<R: Into<Option<RangeInclusive<Timestamp>>>>(&mut self, time_range: R) {
        let mut state = self.state.borrow_mut();

        state.time_axis = time_range.into().map(|range| TimeAxis {
            range: range.clone(),
            ticks: calculate_time_ticks(range, state.time_ticks),
        });

        drop(state);
        self.update_rows();
    }

    pub fn set_data(&mut self, data: Vec<(MetricKey, Vec<DataPoint>)>) {
        let mut state = self.state.borrow_mut();

        state.rows = data
            .into_iter()
            .map(|(key, points)| ChartListRow::new(key, points, state.value_ticks))
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
        self.state.borrow().value_axis_width
    }

    pub fn set_value_axis_width(&mut self, width: i32) {
        let mut state = self.state.borrow_mut();
        state.value_axis_width = width;

        drop(state);
        let state = self.state.borrow();

        if state.value_ticks > 0 {
            self.table.set_col_width(0, width);
        }

        self.table.redraw();
    }

    pub fn set_value_ticks(&mut self, ticks: usize) {
        let mut state = self.state.borrow_mut();
        if state.value_ticks == ticks {
            return;
        }

        state.value_ticks = ticks;
        for row in state.rows.iter_mut() {
            row.value_axis.ticks = calculate_value_ticks(*row.value_axis.range.end(), ticks);
        }

        drop(state);
        let state = self.state.borrow();

        if ticks > 0 {
            self.table.set_col_width(0, state.value_axis_width);
        } else {
            self.table.set_col_width(0, 0);
        }

        self.table.redraw();
    }

    pub fn time_axis_height(&self) -> i32 {
        self.state.borrow().time_axis_height
    }

    pub fn set_time_axis_height(&mut self, height: i32) {
        let mut state = self.state.borrow_mut();
        state.time_axis_height = height;

        drop(state);
        let state = self.state.borrow();

        if state.time_ticks > 0 {
            self.table.set_col_header_height(height);
        }

        self.table.redraw();
    }

    pub fn set_time_ticks(&mut self, ticks: usize) {
        let mut state = self.state.borrow_mut();
        if state.time_ticks == ticks {
            return;
        }

        state.time_ticks = ticks;
        if let Some(time_axis) = state.time_axis.as_mut() {
            time_axis.ticks = calculate_time_ticks(time_axis.range.clone(), ticks);
        }

        drop(state);

        if ticks > 0 {
            self.table.set_row_header(true);
        } else {
            self.table.set_row_header(false);
        }

        self.table.redraw();
    }

    pub fn chart_width(&self) -> i32 {
        self.table.col_width(1)
    }

    pub fn set_chart_width(&mut self, width: i32) {
        self.table.set_col_width(1, width);
        self.table.redraw();
    }

    pub fn set_chart_height(&mut self, height: i32) {
        let mut state = self.state.borrow_mut();
        state.chart_height = height;

        drop(state);
        let state = self.state.borrow();

        self.table
            .set_row_height_all(state.chart_height + state.chart_spacing);
        self.table.redraw();
    }

    pub fn set_chart_spacing(&mut self, spacing: i32) {
        let mut state = self.state.borrow_mut();
        state.chart_spacing = spacing;

        drop(state);
        let state = self.state.borrow();

        self.table
            .set_row_height_all(state.chart_height + state.chart_spacing);
        self.table.redraw();
    }

    pub fn set_key_width(&mut self, width: i32) {
        self.table.set_col_width(2, width);
        self.table.redraw();
    }

    pub fn set_key_margin(&mut self, margin: i32) {
        {
            self.state.borrow_mut().key_margin = margin;
        }
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

    fltk::draw::push_clip(x, y, w, h);

    match ctx {
        TableContext::ColHeader => {
            fltk::draw::draw_rect_fill(x, y, w, h, Color::Background2);
            if col == 1 {
                if let Some(time_axis) = state.time_axis.as_ref() {
                    draw_time_tick_lines(x, y, w, h, time_axis, &state.style);
                    draw_time_tick_labels(x, y, w, h, time_axis, &state.style);
                }
            }
        }
        TableContext::Cell if col == 0 => {
            fltk::draw::draw_rect_fill(x, y, w, h, Color::Background2);
            let row = &state.rows[row as usize];
            draw_value_tick_labels(x, chart_y, w, chart_h, &row.value_axis, &state.style);
        }
        TableContext::Cell if col == 1 => {
            fltk::draw::draw_rect_fill(x, y, w, h, Color::Background2);
            if let Some(time_axis) = state.time_axis.as_ref() {
                let row = &state.rows[row as usize];
                draw_data_fill(
                    x,
                    chart_y,
                    w,
                    chart_h,
                    time_axis,
                    &row.value_axis,
                    &row.data,
                    &state.style,
                );
                draw_time_tick_lines(x, y, w, h, time_axis, &state.style);
                draw_value_tick_lines(x, chart_y, w, chart_h, &row.value_axis, &state.style);
                draw_data_line(
                    x,
                    chart_y,
                    w,
                    chart_h,
                    time_axis,
                    &row.value_axis,
                    &row.data,
                    &state.style,
                );
            }
        }
        TableContext::Cell if col == 2 => {
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
                fltk::draw::draw_text2(
                    &key,
                    x + state.key_margin,
                    y,
                    w - state.key_margin,
                    h,
                    Align::Left,
                );
            }
        }
        _ => (),
    }

    fltk::draw::pop_clip();
}
