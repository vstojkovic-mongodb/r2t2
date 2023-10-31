use std::cell::RefCell;
use std::ops::RangeInclusive;
use std::rc::Rc;

use anyhow::{bail, Context};
use chrono::DateTime;
use fltk::app::{self, Sender};
use fltk::button::Button;
use fltk::dialog::{FileDialogType, NativeFileChooser};
use fltk::enums::{Color, Shortcut};
use fltk::frame::Frame;
use fltk::input::Input;
use fltk::menu::MenuBar;
use fltk::prelude::*;
use fltk::window::Window;
use fltk_float::grid::{CellAlign, Grid};

use crate::ftdc::{MetricKey, Timestamp};
use crate::Message;

use super::chart::{
    draw_data_fill, draw_data_line, draw_value_axis, ChartData, TimeAxis, ValueAxis,
};
use super::layout::wrapper_factory;
use super::menu::MenuExt;
use super::weak_cb;

pub struct MainWindow {
    window: Window,
    tx: Sender<Message>,
    start_input: Input,
    end_input: Input,
    key_input: Input,
    draw_button: Button,
    chart: Frame,
    data_margins: (i32, i32, i32, i32),
    state: RefCell<State>,
}

pub enum Update {
    DataSetLoaded {
        start: Timestamp,
        end: Timestamp,
        keys: Vec<MetricKey>,
    },
    MetricSampled(Vec<(Timestamp, i64)>),
}

enum State {
    Empty,
    Loaded(DataState),
    Selected {
        data: DataState,
        selector: SelectorState,
    },
    Charted {
        data: DataState,
        chart: ChartState,
    },
}

impl State {
    fn take(&mut self) -> Self {
        std::mem::replace(self, State::Empty)
    }
}

struct DataState {
    keys: Vec<MetricKey>,
    time_range: RangeInclusive<Timestamp>,
}

struct SelectorState {
    key: MetricKey,
    time_range: RangeInclusive<Timestamp>,
}

struct ChartState {
    time_axis: TimeAxis,
    value_axis: ValueAxis,
    data: ChartData,
}

impl Default for State {
    fn default() -> Self {
        Self::Empty
    }
}

impl MainWindow {
    pub fn new(width: i32, height: i32, tx: Sender<Message>) -> Rc<Self> {
        let (screen_x, screen_y, screen_w, screen_h) = app::Screen::work_area_mouse().tup();
        let x = screen_x + (screen_w - width) / 2;
        let y = screen_y + (screen_h - height) / 2;

        let mut window = Window::default()
            .with_label("r2t2")
            .with_pos(x, y)
            .with_size(width, height);
        window.size_range(1, 1, 0, 0);
        window.make_resizable(true);

        let mut root = Grid::builder_with_factory(wrapper_factory());
        root.col().with_stretch(1).add();

        root.row().add();
        let mut menu = root.cell().unwrap().wrap(MenuBar::default());
        let mut open_item = menu.add_item("&File/_&Open...\t\t", Shortcut::Ctrl | 'o');
        let mut exit_item = menu.add_item("&File/E&xit\t\t", Shortcut::None);

        root.row()
            .with_stretch(1)
            .with_default_align(CellAlign::Stretch)
            .add();

        let mut work_area = Grid::builder_with_factory(wrapper_factory())
            .with_padding(10, 10, 10, 10)
            .with_col_spacing(10)
            .with_row_spacing(10);

        work_area.col().add();
        work_area.col().with_stretch(1).add();

        work_area.row().add();
        work_area
            .cell()
            .unwrap()
            .wrap(Frame::default().with_label("Start:"));
        let start_input = work_area.cell().unwrap().wrap(Input::default());

        work_area.row().add();
        work_area
            .cell()
            .unwrap()
            .wrap(Frame::default().with_label("End:"));
        let end_input = work_area.cell().unwrap().wrap(Input::default());

        work_area.row().add();
        work_area
            .cell()
            .unwrap()
            .wrap(Frame::default().with_label("Key:"));
        let key_input = work_area.cell().unwrap().wrap(Input::default());

        work_area.row().add();
        let mut draw_button = work_area
            .span(1, 2)
            .unwrap()
            .wrap(Button::default().with_label("Draw"));

        work_area
            .row()
            .with_stretch(1)
            .with_default_align(CellAlign::Stretch)
            .add();
        let mut chart = work_area.span(1, 2).unwrap().wrap(Frame::default());

        root.cell().unwrap().add(work_area.end());

        let root = root.end();
        root.layout_children();

        window.resize_callback(move |_, _, _, _, _| root.layout_children());

        fltk::draw::set_font(chart.label_font(), chart.label_size());
        let (max_val_w, max_val_h) = fltk::draw::measure("9,223,372,036,854,775,808 ", false);

        let this = Rc::new(Self {
            window,
            tx,
            start_input,
            end_input,
            key_input,
            draw_button: draw_button.clone(),
            chart: chart.clone(),
            data_margins: (max_val_w, 0, 0, max_val_h),
            state: Default::default(),
        });

        open_item.set_callback(weak_cb!(|this, _| this.on_open_file()));
        exit_item.set_callback(|_| app::quit());

        draw_button.deactivate();
        draw_button.set_callback(weak_cb!(|this, _| this.on_draw()));

        chart.draw(weak_cb!(|this, _| this.draw_chart()));

        this
    }

    pub fn show(&self) {
        self.window.clone().show();
    }

    pub fn update(&self, update: Update) {
        match update {
            Update::DataSetLoaded { start, end, mut keys } => {
                keys.sort();

                let mut state = self.state.borrow_mut();
                *state = State::Loaded(DataState { keys, time_range: start..=end });
                drop(state);

                self.draw_button.clone().activate();
            }
            Update::MetricSampled(points) => {
                let max_value = points.iter().map(|p| p.1).max().unwrap_or_default();
                let ticks = calculate_ticks(max_value, 5);

                let value_axis = ValueAxis {
                    range: 0..=max_value,
                    ticks,
                    font: (self.chart.label_font(), self.chart.label_size()),
                    color: Color::Light1,
                };

                let mut state = self.state.borrow_mut();

                let (data, selector) = match state.take() {
                    State::Selected { data, selector } => (data, selector),
                    _ => unreachable!(),
                };
                let chart = ChartState {
                    time_axis: TimeAxis { range: selector.time_range },
                    value_axis,
                    data: ChartData {
                        points,
                        color: Color::Black,
                        fill: Some(Color::from_hex(0xeeeeee)),
                    },
                };

                *state = State::Charted { data, chart };
                drop(state);

                self.chart.clone().redraw();
            }
        }
    }

    fn on_open_file(&self) {
        let mut dialog = NativeFileChooser::new(FileDialogType::BrowseFile);
        dialog.show();

        if let Some(filename) = dialog.filenames().first() {
            self.tx.send(Message::OpenFile(filename.clone()));
        }
    }

    fn on_draw(&self) {
        let selector = match self.parse_selector() {
            Ok(tuple) => tuple,
            Err(err) => {
                fltk::dialog::alert_default(&err.to_string());
                return;
            }
        };
        let (margin_left, _, margin_right, _) = self.data_margins;
        self.tx.send(Message::SampleMetric(
            selector.key.clone(),
            selector.time_range.clone(),
            (self.chart.w() - margin_left - margin_right) as _,
        ));

        let mut state = self.state.borrow_mut();
        let data = match state.take() {
            State::Loaded(data) => data,
            State::Selected { data, selector: _ } => data,
            State::Charted { data, chart: _ } => data,
            _ => unreachable!(),
        };
        *state = State::Selected { data, selector };
    }

    fn draw_chart(&self) {
        let x = self.chart.x();
        let y = self.chart.y();
        let w = self.chart.w();
        let h = self.chart.h();
        let (margin_left, margin_top, margin_right, margin_bottom) = self.data_margins;

        fltk::draw::draw_rect_fill(x, y, w, h, Color::Background2);

        let state = self.state.borrow();
        let chart_state = match &*state {
            State::Charted { data: _, chart } => chart,
            _ => return,
        };
        if chart_state.data.points.is_empty() {
            return;
        }

        fltk::draw::push_clip(x, y, w, h);

        draw_data_fill(
            x + margin_left,
            y + margin_top,
            w - margin_left - margin_right,
            h - margin_top - margin_bottom,
            &chart_state.time_axis,
            &chart_state.value_axis,
            &chart_state.data,
        );

        draw_value_axis(
            x + margin_left,
            y + margin_top,
            w - margin_left - margin_right,
            h - margin_top - margin_bottom,
            &chart_state.value_axis,
        );

        draw_data_line(
            x + margin_left,
            y + margin_top,
            w - margin_left - margin_right,
            h - margin_top - margin_bottom,
            &chart_state.time_axis,
            &chart_state.value_axis,
            &chart_state.data,
        );

        fltk::draw::pop_clip();
    }

    fn parse_selector(&self) -> anyhow::Result<SelectorState> {
        let start = DateTime::parse_from_rfc3339(&self.start_input.value())
            .context("error parsing start time")?
            .into();
        let end = DateTime::parse_from_rfc3339(&self.end_input.value())
            .context("error parsing end time")?
            .into();
        let key = (&self.key_input.value().split('|').collect::<Vec<_>>()[..]).into();

        let state = self.state.borrow();
        let data = match &*state {
            State::Loaded(data) => data,
            State::Selected { data, selector: _ } => data,
            State::Charted { data, chart: _ } => data,
            _ => unreachable!(),
        };

        if !data.time_range.contains(&start) {
            bail!("start time out of bounds");
        }

        if !data.time_range.contains(&end) {
            bail!("end time out of bounds");
        }

        if !data.keys.contains(&key) {
            bail!("key not in dataset");
        }

        Ok(SelectorState { key, time_range: start..=end })
    }
}

fn calculate_ticks(max_value: i64, max_ticks: i64) -> Vec<i64> {
    let magnitude = 10i64.pow(max_value.ilog10());
    let mut tick_delta = max_value * (100 / max_ticks) / magnitude;
    for td in [10, 20, 25, 50, 100, 200, 250, 500, 1000] {
        if tick_delta < td {
            tick_delta = td * magnitude / 100;
            break;
        }
    }
    (0..=max_value).step_by(tick_delta as _).collect()
}
