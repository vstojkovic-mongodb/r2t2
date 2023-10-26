use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use bson::Document;
use fltk::app;
use ftdc::{read_chunk, Chunk, Error, MetricKey, Result, Timestamp};
use gui::Update;

mod ftdc;
mod gui;

use self::gui::MainWindow;

#[derive(Debug)]
pub enum Message {
    OpenFile(PathBuf),
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
                }
            }
        }
    });

    main_window.show();
    app.run().unwrap();
}
