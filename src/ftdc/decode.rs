use std::collections::HashMap;
use std::io::Read;

use bson::{Bson, Document};

use crate::metric::{unix_millis_to_timestamp, MetricKey};

use super::{MetricsChunk, Result};

pub(super) struct MetricsDecoder {
    num_deltas: usize,
    metrics: Vec<(MetricKey, Vec<i64>)>,
}

impl MetricsDecoder {
    pub fn new(num_keys: usize, num_deltas: usize) -> Self {
        Self { num_deltas, metrics: Vec::with_capacity(num_keys) }
    }

    pub fn collect_metrics(&mut self, doc: Document) {
        let mut prefix = MetricKey::new();
        self.collect_element_metrics(&Bson::Document(doc), &mut prefix);
    }

    pub fn decode_deltas<R: Read>(&mut self, reader: &mut R) -> Result<()> {
        let mut num_zeroes = 0;
        for (_, values) in self.metrics.iter_mut() {
            let mut value = values[0];
            let mut deltas_left = self.num_deltas;
            while deltas_left > 0 {
                if num_zeroes > 0 {
                    let zeroes_to_use = std::cmp::min(deltas_left, num_zeroes);
                    for _ in 0..zeroes_to_use {
                        values.push(value);
                    }
                    deltas_left -= zeroes_to_use;
                    num_zeroes -= zeroes_to_use;
                    continue;
                }

                // we should've just used proper LEB128 for negative deltas, but here we are
                let delta = leb128::read::unsigned(reader)? as i64;
                if delta != 0 {
                    value += delta;
                    values.push(value);
                    deltas_left -= 1;
                } else {
                    num_zeroes = 1 + leb128::read::unsigned(reader)? as usize;
                }
            }
        }
        Ok(())
    }

    pub fn finish(self) -> MetricsChunk {
        let metrics: HashMap<_, _> = self.metrics.into_iter().collect();
        let timestamps = metrics["start"]
            .iter()
            .map(|&millis| unix_millis_to_timestamp(millis))
            .collect();
        MetricsChunk { timestamps, metrics }
    }

    fn collect_element_metrics(&mut self, elem: &Bson, prefix: &mut MetricKey) {
        match elem {
            Bson::Document(doc) => self.collect_children(prefix, doc),
            Bson::Array(array) => self.collect_children(
                prefix,
                array
                    .into_iter()
                    .enumerate()
                    .map(|(idx, elem)| (idx.to_string(), elem)),
            ),
            Bson::DateTime(value) => self.add_metric(prefix, value.timestamp_millis()),
            Bson::Timestamp(value) => {
                let t = Bson::Int64(value.time as i64);
                let i = Bson::Int64(value.increment as i64);
                self.collect_children(prefix, [("t", &t), ("i", &i)]);
            }
            Bson::Int64(value) => self.add_metric(prefix, *value),
            Bson::Int32(value) => self.add_metric(prefix, *value as i64),
            Bson::Double(value) => self.add_metric(prefix, *value as i64),
            Bson::Boolean(value) => self.add_metric(prefix, if *value { 1 } else { 0 }),
            _ => (), // TODO: Log
        }
    }

    fn collect_children<'e, K: AsRef<str>, I: IntoIterator<Item = (K, &'e Bson)>>(
        &mut self,
        prefix: &mut MetricKey,
        children: I,
    ) {
        let prefix_len = prefix.len();
        for (key, elem) in children {
            prefix.push(key.as_ref());
            self.collect_element_metrics(elem, prefix);
            prefix.truncate(prefix_len);
        }
    }

    fn add_metric(&mut self, key: &MetricKey, init_val: i64) {
        let mut values = Vec::with_capacity(self.num_deltas + 1);
        values.push(init_val);

        self.metrics.push((key.clone(), values));
    }
}
