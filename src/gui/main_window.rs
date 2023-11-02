use std::cell::RefCell;
use std::ops::RangeInclusive;
use std::rc::Rc;

use anyhow::{bail, Context};
use chrono::DateTime;
use fltk::app::{self, Sender};
use fltk::button::Button;
use fltk::dialog::{FileDialogType, NativeFileChooser};
use fltk::enums::Shortcut;
use fltk::frame::Frame;
use fltk::input::Input;
use fltk::menu::MenuBar;
use fltk::misc::InputChoice;
use fltk::prelude::*;
use fltk::window::Window;
use fltk_float::grid::{CellAlign, Grid};
use fltk_float::{SimpleWrapper, Size};

use crate::ftdc::{MetricKey, Timestamp};
use crate::gui::menu::MenuConvenienceExt;
use crate::Message;

use super::chart::ChartListView;
use super::layout::wrapper_factory;
use super::weak_cb;

pub struct MainWindow {
    window: Window,
    tx: Sender<Message>,
    start_input: Input,
    end_input: Input,
    draw_button: Button,
    chart: ChartListView,
    state: RefCell<State>,
}

pub enum Update {
    DataSetLoaded {
        start: Timestamp,
        end: Timestamp,
        keys: Vec<MetricKey>,
    },
    MetricsSampled(Vec<(MetricKey, Vec<(Timestamp, f64)>)>),
}

enum State {
    Empty,
    Loaded(DataState),
    Selected {
        data: DataState,
        selector: SelectorState,
    },
    Charted(DataState),
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
    time_range: RangeInclusive<Timestamp>,
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
        let open_item_id = menu.add_item("&File/_&Open...\t\t", Shortcut::Ctrl | 'o');
        let exit_item_id = menu.add_item("&File/E&xit\t\t", Shortcut::None);
        menu.end();

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
            .with_horz_align(CellAlign::End)
            .wrap(Frame::default().with_label("Start:"));
        let start_input = work_area.cell().unwrap().wrap(Input::default());

        work_area.row().add();
        work_area
            .cell()
            .unwrap()
            .with_horz_align(CellAlign::End)
            .wrap(Frame::default().with_label("End:"));
        let end_input = work_area.cell().unwrap().wrap(Input::default());

        work_area.row().add();
        work_area
            .cell()
            .unwrap()
            .with_horz_align(CellAlign::End)
            .wrap(Frame::default().with_label("Chart Size:"));
        let mut chart_size_choice = work_area.cell().unwrap().wrap(InputChoice::default());
        chart_size_choice.input().set_readonly(true);
        chart_size_choice.add("Small");
        chart_size_choice.add("Medium");
        chart_size_choice.add("Large");
        chart_size_choice.set_value_index(0);

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
        let mut chart = ChartListView::default();
        work_area
            .span(1, 2)
            .unwrap()
            .add(SimpleWrapper::new(chart.widget(), Size::default()));

        root.cell().unwrap().add(work_area.end());

        let root = root.end();
        root.layout_children();

        window.resize_callback(move |_, _, _, _, _| root.layout_children());

        let style = chart.style();
        fltk::draw::set_font(style.value_text_font.0, style.value_text_font.1);
        let (max_val_w, _) = fltk::draw::measure("9,223,372,036,854,775,808 ", false);
        chart.set_value_axis_width(max_val_w);
        chart.set_key_width(chart.w() - chart.chart_width() - chart.value_axis_width() - 2);
        chart.set_chart_height(20);
        chart.set_chart_spacing(40);
        chart.set_value_ticks(0);

        let this = Rc::new(Self {
            window,
            tx,
            start_input,
            end_input,
            draw_button: draw_button.clone(),
            chart: chart.clone(),
            state: Default::default(),
        });

        menu.at(open_item_id)
            .unwrap()
            .set_callback(weak_cb!(|this, _| this.on_open_file()));
        menu.at(exit_item_id).unwrap().set_callback(|_| app::quit());

        chart_size_choice.set_callback({
            let mut chart = chart.clone();
            move |input| {
                let size = input.menu_button().value() * 50 + 20;
                chart.set_chart_height(size);
                if size >= 70 {
                    chart.set_value_ticks(5);
                } else {
                    chart.set_value_ticks(0);
                }
            }
        });

        draw_button.deactivate();
        draw_button.set_callback(weak_cb!(|this, _| this.on_draw()));

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
            Update::MetricsSampled(samples) => {
                let mut state = self.state.borrow_mut();

                let (data, selector) = match state.take() {
                    State::Selected { data, selector } => (data, selector),
                    _ => unreachable!(),
                };

                *state = State::Charted(data);
                drop(state);

                let mut chart = self.chart.clone();
                chart.set_time_range(selector.time_range.clone());
                chart.set_data(samples);
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

        let mut state = self.state.borrow_mut();
        let data = match state.take() {
            State::Loaded(data) => data,
            State::Selected { data, selector: _ } => data,
            State::Charted(data) => data,
            _ => unreachable!(),
        };

        self.tx.send(Message::SampleMetrics(
            data.keys.clone(),
            selector.time_range.clone(),
            self.chart.chart_width() as _,
        ));

        *state = State::Selected { data, selector };
    }

    fn parse_selector(&self) -> anyhow::Result<SelectorState> {
        let start = DateTime::parse_from_rfc3339(&self.start_input.value())
            .context("error parsing start time")?
            .into();
        let end = DateTime::parse_from_rfc3339(&self.end_input.value())
            .context("error parsing end time")?
            .into();

        let state = self.state.borrow();
        let data = match &*state {
            State::Loaded(data) => data,
            State::Selected { data, selector: _ } => data,
            State::Charted(data) => data,
            _ => unreachable!(),
        };

        if !data.time_range.contains(&start) {
            bail!("start time out of bounds");
        }

        if !data.time_range.contains(&end) {
            bail!("end time out of bounds");
        }

        Ok(SelectorState { time_range: start..=end })
    }
}
