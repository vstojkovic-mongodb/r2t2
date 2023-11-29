use std::cell::RefCell;
use std::collections::HashMap;
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

use crate::gui::menu::MenuConvenienceExt;
use crate::metric::{Descriptor, Section, Timestamp, TimestampFormat};
use crate::Message;

use super::chart::{ChartListSection, ChartListView, SectionState};
use super::layout::wrapper_factory;
use super::weak_cb;

pub struct MainWindow {
    window: Window,
    tx: Sender<Message>,
    start_input: Input,
    end_input: Input,
    set_zoom_button: Button,
    reset_zoom_button: Button,
    chart: ChartListView,
    state: RefCell<State>,
}

pub enum Update {
    DataSetLoaded {
        start: Timestamp,
        end: Timestamp,
        transients: Vec<Rc<Descriptor>>,
    },
    DescriptorsLoaded {
        sections: Vec<Section>,
        transients: Vec<Rc<Descriptor>>,
    },
    MetricsSampled(HashMap<usize, Vec<(Timestamp, f64)>>),
}

#[derive(Debug, Default)]
struct State {
    sections: Vec<Section>,
    sections_dirty: DirtyFlag,
    transients: Vec<Rc<Descriptor>>,
    data_time_range: Option<RangeInclusive<Timestamp>>,
    zoom_time_range: Option<RangeInclusive<Timestamp>>,
}

#[derive(Debug, Clone, Copy)]
enum DirtyFlag {
    Dirty,
    Clean,
}

impl Default for DirtyFlag {
    fn default() -> Self {
        Self::Dirty
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
        let open_item_id = menu.add_item("&File/&Open...\t\t", Shortcut::Ctrl | 'o');
        let load_descriptors_id = menu.add_item("&File/_&Load Descriptors...", Shortcut::None);
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
        work_area.col().add();
        work_area.col().with_stretch(1).add();
        work_area.col().add();
        work_area.col().add();

        work_area.row().add();
        work_area
            .cell()
            .unwrap()
            .with_horz_align(CellAlign::End)
            .wrap(Frame::default().with_label("Start:"));
        let start_input = work_area.cell().unwrap().wrap(Input::default());
        work_area
            .cell()
            .unwrap()
            .with_horz_align(CellAlign::End)
            .wrap(Frame::default().with_label("End:"));
        let end_input = work_area.cell().unwrap().wrap(Input::default());
        let mut set_zoom_button = work_area
            .cell()
            .unwrap()
            .wrap(Button::default().with_label("Set Zoom"));
        let mut reset_zoom_button = work_area
            .cell()
            .unwrap()
            .wrap(Button::default().with_label("Reset Zoom"));

        work_area.row().add();
        work_area
            .cell()
            .unwrap()
            .with_horz_align(CellAlign::End)
            .wrap(Frame::default().with_label("Chart Size:"));
        let mut chart_size_choice = work_area.span(1, 5).unwrap().wrap(InputChoice::default());
        chart_size_choice.input().set_readonly(true);
        chart_size_choice.add("Small");
        chart_size_choice.add("Medium");
        chart_size_choice.add("Large");
        chart_size_choice.set_value_index(0);

        work_area
            .row()
            .with_stretch(1)
            .with_default_align(CellAlign::Stretch)
            .add();
        let mut chart = ChartListView::default();
        work_area
            .span(1, 6)
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
            set_zoom_button: set_zoom_button.clone(),
            reset_zoom_button: reset_zoom_button.clone(),
            chart: chart.clone(),
            state: Default::default(),
        });

        menu.at(open_item_id)
            .unwrap()
            .set_callback(weak_cb!(|this, _| this.on_open_file()));
        menu.at(load_descriptors_id)
            .unwrap()
            .set_callback(weak_cb!(|this, _| this.on_load_descriptors()));
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

        set_zoom_button.deactivate();
        set_zoom_button.set_callback(weak_cb!(|this, _| this.on_set_zoom()));

        reset_zoom_button.set_callback(weak_cb!(|this, _| this.on_reset_zoom()));
        reset_zoom_button.deactivate();

        this
    }

    pub fn show(&self) {
        self.window.clone().show();
    }

    pub fn update(&self, update: Update) {
        match update {
            Update::DataSetLoaded { start, end, transients } => {
                let mut state = self.state.borrow_mut();

                state.set_transients(transients);
                state.data_time_range = Some(start..=end);

                if let Some(zoom) = state.zoom_time_range.as_mut() {
                    let zoom_start = std::cmp::max(start, *zoom.start());
                    let zoom_end = std::cmp::max(end, *zoom.end());
                    *zoom = zoom_start..=zoom_end;
                }

                let sample_range = state.sample_range().unwrap();

                self.populate_zoom(&sample_range);
                self.set_zoom_button.clone().activate();

                drop(state);

                self.request_metrics_sample();
            }
            Update::DescriptorsLoaded { sections, transients } => {
                let mut state = self.state.borrow_mut();
                state.set_sections(sections);
                state.set_transients(transients);

                if state.data_time_range.is_none() {
                    return;
                }

                drop(state);

                self.request_metrics_sample();
            }
            Update::MetricsSampled(samples) => {
                let mut state = self.state.borrow_mut();

                let mut chart_data = Vec::with_capacity(state.sections.len() + 1);
                for (idx, section) in state.sections.iter().enumerate() {
                    let section_state = if let DirtyFlag::Dirty = state.sections_dirty {
                        SectionState::Expanded
                    } else {
                        self.chart.section_state(idx)
                    };
                    chart_data.push(ChartListSection {
                        name: section.name.clone(),
                        state: section_state,
                        charts: section
                            .metrics
                            .iter()
                            .map(|desc| {
                                (
                                    Rc::clone(desc),
                                    samples.get(&desc.id).cloned().unwrap_or_default(),
                                )
                            })
                            .collect(),
                    });
                }
                let transients_state = if let DirtyFlag::Dirty = state.sections_dirty {
                    SectionState::Expanded
                } else {
                    self.chart.section_state(self.chart.section_count() - 1)
                };
                chart_data.push(ChartListSection {
                    name: UNKNOWN_SECTION.to_string(),
                    state: transients_state,
                    charts: state
                        .transients
                        .iter()
                        .map(|desc| {
                            (
                                Rc::clone(desc),
                                samples.get(&desc.id).cloned().unwrap_or_default(),
                            )
                        })
                        .collect(),
                });
                state.sections_dirty = DirtyFlag::Clean;

                let sample_range = state.sample_range().unwrap();

                drop(state);

                let mut chart = self.chart.clone();
                chart.set_time_range(sample_range);
                chart.set_data(chart_data);
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

    fn on_load_descriptors(&self) {
        let mut dialog = NativeFileChooser::new(FileDialogType::BrowseFile);
        dialog.set_filter("JSON Files\t*.json");
        dialog.show();

        if let Some(filename) = dialog.filenames().first() {
            self.tx.send(Message::LoadDescriptors(filename.clone()));
        }
    }

    fn on_set_zoom(&self) {
        let zoom_range = match self.parse_zoom() {
            Ok(range) => Some(range),
            Err(err) => {
                fltk::dialog::alert_default(&err.to_string());
                return;
            }
        };

        let mut state = self.state.borrow_mut();
        let can_reset = state.data_time_range != zoom_range;
        state.zoom_time_range = zoom_range;

        drop(state);

        if can_reset {
            self.reset_zoom_button.clone().activate();
        } else {
            self.reset_zoom_button.clone().deactivate();
        }
        self.request_metrics_sample();
    }

    fn on_reset_zoom(&self) {
        let mut state = self.state.borrow_mut();

        state.zoom_time_range = None;
        self.populate_zoom(state.data_time_range.as_ref().unwrap());

        drop(state);

        self.reset_zoom_button.clone().deactivate();
        self.request_metrics_sample();
    }

    fn request_metrics_sample(&self) {
        let state = self.state.borrow();
        self.tx.send(Message::SampleMetrics(
            state.descriptors().map(|desc| desc.id).collect(),
            state.sample_range().unwrap(),
            self.chart.chart_width() as _,
        ));
    }

    fn populate_zoom(&self, zoom_time_range: &RangeInclusive<Timestamp>) {
        self.start_input
            .clone()
            .set_value(&zoom_time_range.start().to_timestamp_string());
        self.end_input
            .clone()
            .set_value(&zoom_time_range.end().to_timestamp_string());
    }

    fn parse_zoom(&self) -> anyhow::Result<RangeInclusive<Timestamp>> {
        let start = DateTime::parse_from_rfc3339(&self.start_input.value())
            .context("error parsing start time")?
            .into();
        let end = DateTime::parse_from_rfc3339(&self.end_input.value())
            .context("error parsing end time")?
            .into();

        let state = self.state.borrow();
        let data_time_range = state.data_time_range.as_ref().unwrap();

        if !data_time_range.contains(&start) {
            bail!("start time out of bounds");
        }

        if !data_time_range.contains(&end) {
            bail!("end time out of bounds");
        }

        Ok(start..=end)
    }
}

impl State {
    fn descriptors(&self) -> impl Iterator<Item = &Rc<Descriptor>> {
        self.sections
            .iter()
            .flat_map(|section| section.metrics.iter())
            .chain(self.transients.iter())
    }

    fn sample_range(&self) -> Option<RangeInclusive<Timestamp>> {
        self.zoom_time_range
            .as_ref()
            .or_else(|| self.data_time_range.as_ref())
            .cloned()
    }

    fn set_sections(&mut self, sections: Vec<Section>) {
        self.sections = sections;
        self.sections_dirty = DirtyFlag::Dirty;
        for section in self.sections.iter_mut() {
            section.metrics.sort_by(|lhs, rhs| lhs.name.cmp(&rhs.name));
        }
    }

    fn set_transients(&mut self, transients: Vec<Rc<Descriptor>>) {
        self.transients = transients;
        self.transients.sort_by(|lhs, rhs| lhs.name.cmp(&rhs.name));
    }
}

const UNKNOWN_SECTION: &str = "UNKNOWN";
