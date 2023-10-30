use std::collections::HashMap;
use std::fs::File;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use bson::Document;
use fltk::app;

mod ftdc;
mod gui;

use self::ftdc::{read_chunk, Chunk, Error, MetricKey, Result, Timestamp};
use self::gui::MainWindow;
use self::gui::Update;

#[derive(Debug)]
pub enum Message {
    OpenFile(PathBuf),
    SampleMetric(MetricKey, RangeInclusive<Timestamp>, usize),
}

struct DataSet {
    metadata: Document,
    timestamps: Vec<Timestamp>,
    metrics: HashMap<MetricKey, Vec<i64>>,
}

impl DataSet {
    fn new() -> Self {
        Self {
            metadata: Document::new(),
            timestamps: vec![],
            metrics: HashMap::new(),
        }
    }

    fn open_ftdc_file(&mut self, path: &Path) -> Result<()> {
        let mut file = File::open(path)?;
        self.metadata.clear();
        self.timestamps.clear();
        self.metrics.clear();
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
                        self.timestamps.append(&mut chunk.timestamps);
                        for (key, mut values) in chunk.metrics {
                            self.metrics
                                .entry(key)
                                .or_insert_with(Vec::new)
                                .append(&mut values);
                        }
                    }
                },
                Err(Error::EOF) => return Ok(()),
                Err(err) => return Err(err),
            }
        }
    }

    fn sample_metric(
        &self,
        key: &MetricKey,
        range: RangeInclusive<Timestamp>,
        num_samples: usize,
    ) -> Vec<(Timestamp, i64)> {
        let values = &self.metrics[key];

        let mut start_idx = match self.timestamps.binary_search(range.start()) {
            Ok(idx) => idx,
            Err(idx) => idx,
        };
        let end_idx = match self.timestamps.binary_search(range.end()) {
            Ok(idx) => idx,
            Err(idx) => idx - 1,
        };

        let mut result = Vec::with_capacity(num_samples);
        let delta = (range.end().timestamp_millis() - range.start().timestamp_millis())
            / (num_samples as i64);
        let mut sample_time = range.start().timestamp_millis();

        while (end_idx - start_idx) >= num_samples {
            let start_time = self.timestamps[start_idx];
            if start_time.timestamp_millis() >= sample_time {
                result.push((start_time, values[start_idx]));
                sample_time += delta;
            }
            start_idx += 1;
        }
        result.extend(
            (start_idx..=end_idx)
                .into_iter()
                .map(|idx| (self.timestamps[idx], values[idx])),
        );

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
                                    keys: dataset.metrics.keys().cloned().collect(),
                                });
                            }
                        }
                    }
                    Message::SampleMetric(key, range, num_samples) => {
                        main_window.update(Update::MetricSampled(dataset.sample_metric(
                            &key,
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
