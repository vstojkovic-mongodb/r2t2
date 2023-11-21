use std::cell::RefCell;
use std::ops::{Range, RangeInclusive};
use std::rc::Rc;

use chrono::Duration;
use fltk::app::{event_coords, event_is_click};
use fltk::enums::{Align, Color, Damage, Event, Font, FrameType};
use fltk::prelude::*;
use fltk::table::{Table, TableContext};
use fltk::widget::Widget;
use thousands::Separable;

use crate::gui::ScopedClip;
use crate::metric::{Descriptor, Timestamp, TimestampFormat};

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

pub type ChartListData = Vec<ChartListSection>;

pub struct ChartListSection {
    pub name: String,
    pub state: SectionState,
    pub charts: Vec<(Rc<Descriptor>, Vec<DataPoint>)>,
}

#[derive(Debug, Clone, Copy)]
pub enum SectionState {
    Expanded,
    Collapsed,
}

struct ChartListState {
    style: ChartStyle,
    section_heading_height: i32,
    chart_height: i32,
    chart_spacing: i32,
    key_margin: i32,
    time_axis_height: i32,
    time_ticks: usize,
    value_axis_width: i32,
    value_ticks: usize,
    hover_style: HoverStyle,
    time_axis: Option<TimeAxis>,
    charts: Vec<Chart>,
    rows: Vec<ChartListRow>,
    hover: Option<Hover>,
}

#[derive(Debug, Clone)]
pub struct HoverStyle {
    pub frame: FrameType,
    pub font: (Font, i32),
    pub draw_tick: bool,
}

impl Default for HoverStyle {
    fn default() -> Self {
        Self {
            frame: FrameType::PlasticThinDownBox,
            font: (Font::Helvetica, 10),
            draw_tick: true,
        }
    }
}

struct Chart {
    desc: Rc<Descriptor>,
    value_axis: ValueAxis,
    data: ChartData,
}

enum ChartListRow {
    Section {
        name: String,
        chart_idx_range: Range<usize>,
        state: SectionState,
    },
    Chart {
        chart_idx: usize,
    },
}

impl std::ops::Not for SectionState {
    type Output = Self;
    fn not(self) -> Self::Output {
        match self {
            Self::Expanded => Self::Collapsed,
            Self::Collapsed => Self::Expanded,
        }
    }
}

struct Hover {
    extent: (i32, i32, i32, i32),
    time_text: String,
    time_extent: (i32, i32, i32, i32),
    value_text: String,
    value_extent: (i32, i32, i32, i32),
    tick_x: Option<i32>,
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

        fltk::draw::set_font(table.label_font(), table.label_size());

        let state = ChartListState {
            style: Default::default(),
            section_heading_height: fltk::draw::height(),
            chart_height: 100,
            chart_spacing: 20,
            key_margin: 10,
            time_axis_height: 100,
            time_ticks: 6,
            value_axis_width: 100,
            value_ticks: 5,
            hover_style: Default::default(),
            time_axis: None,
            charts: Vec::new(),
            rows: Vec::new(),
            hover: None,
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
        table.handle({
            let state = Rc::clone(&state);
            move |table, event| {
                match event {
                    Event::Move | Event::MouseWheel => Self::on_mouse(event, table, &state),
                    Event::Released if event_is_click() => Self::on_click(table, &state),
                    _ => (),
                };
                false
            }
        });

        Self { table, state }
    }

    pub fn widget(&self) -> Widget {
        self.table.as_base_widget()
    }

    #[allow(dead_code)]
    pub fn with_style(mut self, style: ChartStyle) -> Self {
        self.set_style(style);
        self
    }

    #[allow(dead_code)]
    pub fn with_hover_style(mut self, style: HoverStyle) -> Self {
        self.set_hover_style(style);
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

    #[allow(dead_code)]
    pub fn hover_style(&self) -> HoverStyle {
        self.state.borrow().hover_style.clone()
    }

    pub fn set_hover_style(&mut self, style: HoverStyle) {
        {
            self.state.borrow_mut().hover_style = style;
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

    pub fn set_data(&mut self, data: ChartListData) {
        let mut state = self.state.borrow_mut();

        let value_ticks = state.value_ticks;
        state.rows = Vec::new();
        state.charts = Vec::new();
        for section in data {
            let start_idx = state.charts.len();
            let end_idx = start_idx + section.charts.len();
            state.rows.push(ChartListRow::Section {
                name: section.name,
                chart_idx_range: start_idx..end_idx,
                state: section.state,
            });

            for (desc, points) in section.charts {
                let chart_idx = state.charts.len();
                state.rows.push(ChartListRow::Chart { chart_idx });
                state.charts.push(Chart::new(desc, points, value_ticks));
            }
        }

        drop(state);
        self.update_rows();
    }

    #[allow(dead_code)]
    pub fn x(&self) -> i32 {
        self.table.x()
    }

    #[allow(dead_code)]
    pub fn y(&self) -> i32 {
        self.table.y()
    }

    #[allow(dead_code)]
    pub fn w(&self) -> i32 {
        self.table.w()
    }

    #[allow(dead_code)]
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

        self.table.set_col_width(0, width);
        self.table.redraw();
    }

    pub fn set_value_ticks(&mut self, ticks: usize) {
        let mut state = self.state.borrow_mut();
        if state.value_ticks == ticks {
            return;
        }

        state.value_ticks = ticks;
        for chart in state.charts.iter_mut() {
            chart.value_axis.ticks = calculate_value_ticks(*chart.value_axis.range.end(), ticks);
        }

        drop(state);

        self.table.redraw();
    }

    #[allow(dead_code)]
    pub fn time_axis_height(&self) -> i32 {
        self.state.borrow().time_axis_height
    }

    #[allow(dead_code)]
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

    #[allow(dead_code)]
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

    #[allow(dead_code)]
    pub fn set_chart_width(&mut self, width: i32) {
        self.table.set_col_width(1, width);
        self.table.redraw();
    }

    pub fn set_chart_height(&mut self, height: i32) {
        let mut state = self.state.borrow_mut();
        state.chart_height = height;

        drop(state);

        self.update_rows();
    }

    pub fn set_chart_spacing(&mut self, spacing: i32) {
        let mut state = self.state.borrow_mut();
        state.chart_spacing = spacing;

        drop(state);

        self.update_rows();
    }

    pub fn set_key_width(&mut self, width: i32) {
        self.table.set_col_width(2, width);
        self.table.redraw();
    }

    #[allow(dead_code)]
    pub fn set_key_margin(&mut self, margin: i32) {
        {
            self.state.borrow_mut().key_margin = margin;
        }
        self.table.redraw();
    }

    fn update_rows(&mut self) {
        Self::update_table_rows(&mut self.table, &self.state.borrow());
    }

    fn on_mouse(event: Event, table: &mut Table, state: &Rc<RefCell<ChartListState>>) {
        // Due to implementation details of FLTK, a call to on_mouse can happen while executing
        // a call to on_click, when Table::set_rows(...) collapses the last section. Using
        // try_borrow_mut() here might be hacky, but right now I can't think of a better way to deal
        // with this. ¯\_(ツ)_/¯
        let mut state = match state.try_borrow_mut() {
            Ok(state) => state,
            Err(_) => return,
        };

        if let Some(hover) = state.hover.as_ref() {
            hover.apply_damage(table);
        }

        state.hover = match event {
            Event::Move => Hover::at_cursor(&table, &state),
            Event::MouseWheel => None,
            _ => unreachable!(),
        };

        if let Some(hover) = state.hover.as_ref() {
            hover.apply_damage(table);
        }
    }

    fn on_click(table: &mut Table, state: &Rc<RefCell<ChartListState>>) {
        let (ctx, row, _, _) = match table.cursor2rowcol() {
            Some(tuple) => tuple,
            None => return,
        };
        if ctx != TableContext::Cell {
            return;
        }
        let row = row as usize;

        {
            let mut state = state.borrow_mut();
            let (chart_idx_range, section_state) = match state.rows[row] {
                ChartListRow::Section { ref chart_idx_range, ref mut state, .. } => {
                    (chart_idx_range, state)
                }
                _ => return,
            };

            *section_state = !*section_state;

            match &*section_state {
                SectionState::Expanded => {
                    for (offset, chart_idx) in chart_idx_range.clone().enumerate() {
                        state
                            .rows
                            .insert(row + offset + 1, ChartListRow::Chart { chart_idx });
                    }
                }
                SectionState::Collapsed => {
                    let start = row + 1;
                    let end = start + chart_idx_range.end - chart_idx_range.start;
                    state.rows.drain(start..end);
                }
            }
        }

        Self::update_table_rows(table, &state.borrow());
    }

    fn update_table_rows(table: &mut Table, state: &ChartListState) {
        if state.time_axis.is_some() {
            table.set_rows(state.rows.len() as i32);

            let chart_row_height = state.chart_height + state.chart_spacing;
            let section_row_height = state.section_heading_height;
            for (idx, row) in state.rows.iter().enumerate() {
                let row_height = match row {
                    ChartListRow::Section { .. } => section_row_height,
                    ChartListRow::Chart { .. } => chart_row_height,
                };
                table.set_row_height(idx as i32, row_height);
            }
        } else {
            table.set_rows(0);
        }
        table.redraw();
    }
}

impl Chart {
    fn new(desc: Rc<Descriptor>, points: Vec<DataPoint>, max_ticks: usize) -> Self {
        let max_value = points
            .iter()
            .map(|p| p.1)
            .max_by(f64::total_cmp)
            .unwrap_or_default();
        let ticks = calculate_value_ticks(max_value, max_ticks);

        let value_axis = ValueAxis { range: 0f64..=max_value, ticks };
        Self { desc, value_axis, data: points }
    }
}

impl Hover {
    fn at_cursor(table: &Table, state: &ChartListState) -> Option<Self> {
        let (ctx, row, col, _) = table.cursor2rowcol()?;
        if (ctx != TableContext::Cell) || (col != 1) {
            return None;
        }
        let chart = match &state.rows[row as usize] {
            ChartListRow::Section { .. } => return None,
            ChartListRow::Chart { chart_idx } => &state.charts[*chart_idx],
        };
        let time_range = &state.time_axis.as_ref()?.range;

        let (x, _) = event_coords();
        let (cx, cy, cw, ch) = table.find_cell(TableContext::Cell, row, col).unwrap();

        let time_span = (*time_range.end() - *time_range.start()).num_milliseconds();
        let x_millis = ((x - cx) as i64) * time_span / ((cw - 1) as i64);
        let x_time = *time_range.start() + Duration::milliseconds(x_millis);
        let time_text = x_time.to_timestamp_string();

        let closest = match chart.data.binary_search_by_key(&x_time, |point| point.0) {
            Ok(idx) => Some(&chart.data[idx]),
            Err(idx) => chart.data[idx.saturating_sub(1)..]
                .iter()
                .take(2)
                .min_by_key(|&point| (point.0 - x_time).abs()),
        };
        let value_text = match closest {
            None => "".to_string(),
            Some((_, value)) => {
                let value = (value * 1000.0).round() / 1000.0;
                format!("{} ", value).separate_with_commas()
            }
        };

        fltk::draw::set_font(state.hover_style.font.0, state.hover_style.font.1);
        let (time_w, time_h) = fltk::draw::measure(&time_text, false);
        let (value_w, value_h) = fltk::draw::measure(&value_text, false);
        let spacing = fltk::draw::descent();
        let frame = FrameType::PlasticThinDownBox;

        let y = cy + ch - state.chart_spacing / 2 + spacing;
        let w = std::cmp::max(time_w, value_w) + frame.dx() + frame.dw();
        let h = time_h + value_h + frame.dy() + frame.dh();

        let time_x = x + frame.dx();
        let time_y = y + frame.dy();
        let value_x = time_x;
        let value_y = time_y + time_h;

        let tick_x = if state.hover_style.draw_tick { Some(x) } else { None };

        Some(Self {
            extent: (x, y, w, h),
            time_text,
            time_extent: (time_x, time_y, time_w, time_h),
            value_text,
            value_extent: (value_x, value_y, value_w, value_h),
            tick_x,
        })
    }

    fn apply_damage(&self, table: &mut Table) {
        let (x, y, w, h) = self.extent;
        table.set_damage_area(Damage::All, x, y, w, h);

        if let Some(tick_x) = self.tick_x {
            table.set_damage_area(Damage::All, tick_x, table.y(), 1, table.h());
        }
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
    if !fltk::draw::not_clipped(x, y, w, h) {
        return;
    }

    let state = state.borrow();
    let chart_y = y + state.chart_spacing / 2;
    let chart_h = h - state.chart_spacing;

    let _clip = ScopedClip::new(x, y, w, h);
    if let TableContext::ColHeader | TableContext::Cell = ctx {
        fltk::draw::draw_rect_fill(x, y, w, h, Color::Background2);
    }

    let time_axis = match state.time_axis.as_ref() {
        Some(axis) => axis,
        None => return,
    };

    match ctx {
        TableContext::ColHeader if col == 1 => {
            draw_time_tick_lines(x, y, w, h, time_axis, &state.style);
            if let Some(hover) = state.hover.as_ref() {
                if let Some(tick_x) = hover.tick_x {
                    fltk::draw::set_draw_color(state.style.time_tick_color);
                    fltk::draw::draw_line(tick_x, y, tick_x, y + h - 1);
                }
            }

            draw_time_tick_labels(x, y, w, h, time_axis, &state.style);
        }
        TableContext::Cell if col == 0 => match &state.rows[row as usize] {
            ChartListRow::Chart { chart_idx } => {
                let chart = &state.charts[*chart_idx];
                draw_value_tick_labels(x, chart_y, w, chart_h, &chart.value_axis, &state.style);
            }
            ChartListRow::Section { name, state: section_state, .. } => {
                draw_section_heading(table, row, name, *section_state);
            }
        },
        TableContext::Cell if col == 1 => {
            match &state.rows[row as usize] {
                ChartListRow::Chart { chart_idx } => {
                    let chart = &state.charts[*chart_idx];
                    draw_data_fill(
                        x,
                        chart_y,
                        w,
                        chart_h,
                        time_axis,
                        &chart.value_axis,
                        &chart.data,
                        &state.style,
                    );
                }
                ChartListRow::Section { .. } => (),
            };

            draw_time_tick_lines(x, y, w, h, time_axis, &state.style);
            if let Some(hover) = state.hover.as_ref() {
                if let Some(tick_x) = hover.tick_x {
                    fltk::draw::set_draw_color(state.style.time_tick_color);
                    fltk::draw::draw_line(tick_x, y, tick_x, y + h - 1);
                }
            }

            match &state.rows[row as usize] {
                ChartListRow::Chart { chart_idx } => {
                    let chart = &state.charts[*chart_idx];
                    draw_value_tick_lines(x, chart_y, w, chart_h, &chart.value_axis, &state.style);
                    draw_data_line(
                        x,
                        chart_y,
                        w,
                        chart_h,
                        time_axis,
                        &chart.value_axis,
                        &chart.data,
                        &state.style,
                    );
                }
                ChartListRow::Section { name, state: section_state, .. } => {
                    draw_section_heading(table, row, name, *section_state);
                }
            };
        }
        TableContext::Cell if col == 2 => match &state.rows[row as usize] {
            ChartListRow::Chart { chart_idx } => {
                let text = &state.charts[*chart_idx].desc.name;
                fltk::draw::set_font(table.label_font(), table.label_size());
                fltk::draw::set_draw_color(table.label_color());
                fltk::draw::draw_text2(
                    text,
                    x + state.key_margin,
                    y,
                    w - state.key_margin,
                    h,
                    Align::Left,
                );
            }
            ChartListRow::Section { name, state: section_state, .. } => {
                draw_section_heading(table, row, name, *section_state);
            }
        },
        TableContext::EndPage => {
            if let Some(hover) = state.hover.as_ref() {
                let (hx, hy, hw, hh) = hover.extent;
                let (tx, ty, tw, th) = hover.time_extent;
                let (vx, vy, vw, vh) = hover.value_extent;

                fltk::draw::draw_box(
                    FrameType::PlasticThinDownBox,
                    hx,
                    hy,
                    hw,
                    hh,
                    Color::Background2,
                );

                fltk::draw::set_draw_color(table.label_color());
                fltk::draw::set_font(state.hover_style.font.0, state.hover_style.font.1);
                fltk::draw::draw_text2(&hover.time_text, tx, ty, tw, th, Align::Left);
                fltk::draw::draw_text2(&hover.value_text, vx, vy, vw, vh, Align::Left);
            }
        }
        _ => (),
    }
}

fn draw_section_heading(table: &Table, row: i32, name: &str, state: SectionState) {
    let glyph = match state {
        SectionState::Expanded => "@2>",
        SectionState::Collapsed => "@>",
    };
    let text = format!("{} {}", glyph, name);
    let (x, y, _, _) = table.find_cell(TableContext::Cell, row, 0).unwrap();

    fltk::draw::set_font(table.label_font(), table.label_size());
    fltk::draw::set_draw_color(table.label_color());
    let (w, h) = fltk::draw::measure(&text, true);

    fltk::draw::draw_text2(&text, x, y, w, h, Align::Left);
}
