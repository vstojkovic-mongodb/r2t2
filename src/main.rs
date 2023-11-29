use std::collections::HashMap;
use std::fs::File;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use bson::Document;
use fltk::app;
use metric::{Descriptor, Descriptors};

mod ftdc;
mod gui;
mod metric;

use self::ftdc::{read_chunk, Chunk, Error, Result};
use self::gui::MainWindow;
use self::gui::Update;
use self::metric::{MetricKey, Timestamp};

#[derive(Debug)]
pub enum Message {
    OpenFile(PathBuf),
    LoadDescriptors(PathBuf),
    SampleMetrics(Vec<usize>, RangeInclusive<Timestamp>, usize),
}

struct DataSet {
    descriptors: Descriptors,
    metadata: Document,
    timestamps: Vec<Timestamp>,
    raw_data: HashMap<MetricKey, Vec<f64>>,
}

impl DataSet {
    fn new() -> Self {
        Self {
            descriptors: Descriptors::new(),
            metadata: Document::new(),
            timestamps: vec![],
            raw_data: HashMap::new(),
        }
    }

    fn open_ftdc_file(&mut self, path: &Path) -> Result<()> {
        let mut file = File::open(path)?;
        self.metadata.clear();
        self.timestamps.clear();
        self.raw_data.clear();

        loop {
            match read_chunk(&mut file) {
                Ok(chunk) => match chunk {
                    Chunk::Metadata(doc) => {
                        if self.metadata.is_empty() {
                            self.metadata = doc;
                        } else {
                            // TODO: Log
                        }
                    }
                    Chunk::Data(mut chunk) => {
                        let num_values = chunk.timestamps.len();

                        for (key, values) in self.raw_data.iter_mut() {
                            match chunk.metrics.remove(key) {
                                Some(chunk_values) => {
                                    values.extend(chunk_values.into_iter().map(|v| v as f64))
                                }
                                None => values.extend((0..num_values).map(|_| f64::NAN)),
                            };
                        }

                        for (key, chunk_values) in chunk.metrics {
                            if !self.descriptors.contains_key(&key) {
                                self.descriptors
                                    .add(Descriptor::default_for_key(key.clone()));
                            }
                            let values = match self.raw_data.get_mut(&key) {
                                Some(values) => values,
                                None => self.raw_data.entry(key).or_insert_with(Vec::new),
                            };
                            values.extend((0..self.timestamps.len()).map(|_| f64::NAN));
                            values.extend(chunk_values.into_iter().map(|v| v as f64));
                        }

                        self.timestamps.append(&mut chunk.timestamps);
                    }
                },
                Err(Error::EOF) => return Ok(()),
                Err(err) => return Err(err),
            }
        }
    }

    fn load_descriptors(&mut self, path: &Path) -> std::io::Result<()> {
        let file = File::open(path)?;
        self.descriptors = serde_json::from_reader(file)?;
        for key in self.raw_data.keys() {
            if !self.descriptors.contains_key(key) {
                self.descriptors
                    .add(Descriptor::default_for_key(key.clone()));
            }
        }
        Ok(())
    }

    fn sample_metrics(
        &self,
        ids: Vec<usize>,
        range: RangeInclusive<Timestamp>,
        num_samples: usize,
    ) -> HashMap<usize, Vec<(Timestamp, f64)>> {
        let mut result = HashMap::with_capacity(ids.len());

        for id in ids {
            let desc = Rc::clone(&self.descriptors[id]);
            let values = match self.raw_data.get(&desc.key) {
                Some(values) => values,
                None => {
                    result.insert(id, vec![]);
                    continue;
                }
            };

            let mut start_idx = match self.timestamps.binary_search(range.start()) {
                Ok(idx) => idx,
                Err(idx) => idx,
            };
            let end_idx = match self.timestamps.binary_search(range.end()) {
                Ok(idx) => idx,
                Err(idx) => idx - 1,
            };

            let mut samples = Vec::with_capacity(num_samples);
            let delta = (*range.end() - *range.start()).num_milliseconds() / (num_samples as i64);
            let mut sample_time = range.start().timestamp_millis();

            while (end_idx - start_idx) >= num_samples {
                let start_time = self.timestamps[start_idx];
                if start_time.timestamp_millis() >= sample_time {
                    let value = values[start_idx];
                    if !value.is_nan() {
                        samples.push((start_time, value / desc.scale));
                    }
                    sample_time += delta;
                }
                start_idx += 1;
            }
            samples.extend(
                (start_idx..=end_idx)
                    .into_iter()
                    .filter(|&idx| !values[idx].is_nan())
                    .map(|idx| (self.timestamps[idx], values[idx] / desc.scale)),
            );

            result.insert(id, samples);
        }

        result
    }
}

fn main() {
    let app = app::App::default();
    let (tx, rx) = app::channel();

    let main_window = MainWindow::new(1280, 720, tx);
    let mut dataset = DataSet::new();

    app::add_check({
        let main_window = Rc::clone(&main_window);
        move |_| {
            while let Some(msg) = rx.recv() {
                match msg {
                    Message::OpenFile(path) => {
                        match dataset.open_ftdc_file(&path) {
                            Err(err) => {
                                fltk::dialog::alert_default(&format!(
                                    "Error loading FTDC file: {}",
                                    err
                                ));
                            }
                            Ok(()) => {
                                // TODO: What if empty?
                                main_window.update(Update::DataSetLoaded {
                                    start: *dataset.timestamps.first().unwrap(),
                                    end: *dataset.timestamps.last().unwrap(),
                                    transients: dataset.descriptors.transients().clone(),
                                });
                            }
                        }
                    }
                    Message::LoadDescriptors(path) => match dataset.load_descriptors(&path) {
                        Err(err) => {
                            fltk::dialog::alert_default(&format!(
                                "Error loading descriptors: {}",
                                err
                            ));
                        }
                        Ok(()) => main_window.update(Update::DescriptorsLoaded {
                            sections: dataset.descriptors.sections().clone(),
                            transients: dataset.descriptors.transients().clone(),
                        }),
                    },
                    Message::SampleMetrics(ids, range, num_samples) => {
                        main_window.update(Update::MetricsSampled(dataset.sample_metrics(
                            ids,
                            range,
                            num_samples,
                        )));
                    }
                }
            }
        }
    });

    main_window.show();
    app.run().unwrap();
}
