use std::borrow::Borrow;
use std::collections::HashMap;
use std::fmt::Formatter;
use std::ops::Index;
use std::rc::Rc;

use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};

mod key;
mod time;

pub use self::key::MetricKey;
pub use self::time::{unix_millis_to_timestamp, Timestamp, TimestampFormat};

#[derive(Debug, Clone, Deserialize)]
pub struct Descriptor {
    #[serde(skip)]
    pub id: usize,

    pub key: MetricKey,
    pub name: String,

    #[serde(default = "default_scale")]
    pub scale: f64,
}

#[derive(Debug, Clone)]
pub struct Section {
    pub name: String,
    pub metrics: Vec<Rc<Descriptor>>,
}

pub struct Descriptors {
    by_id: Vec<Rc<Descriptor>>,
    by_key: HashMap<MetricKey, Vec<Rc<Descriptor>>>,
    sections: Vec<Section>,
}

impl Descriptor {
    pub fn default_for_key(key: MetricKey) -> Self {
        let key_str: &str = key.borrow();
        let mut name = String::with_capacity(key_str.len());
        let mut first = true;
        for elem in key.iter() {
            if first {
                first = false;
            } else {
                name.push(' ');
            }
            name.push_str(elem);
        }

        Self { id: usize::MAX, key, name, scale: 1.0 }
    }
}

fn default_scale() -> f64 {
    1.0
}

impl Descriptors {
    pub fn new() -> Self {
        Self {
            by_id: Vec::new(),
            by_key: HashMap::new(),
            sections: Vec::new(),
        }
    }

    pub fn begin_section(&mut self, name: String) {
        self.sections.push(Section { name, metrics: Vec::new() });
    }

    pub fn add(&mut self, mut desc: Descriptor) {
        desc.id = self.by_id.len();
        let desc = Rc::new(desc);

        self.by_id.push(Rc::clone(&desc));
        self.by_key
            .entry(desc.key.clone())
            .or_insert_with(Vec::new)
            .push(Rc::clone(&desc));
        self.sections.last_mut().unwrap().metrics.push(desc);
    }

    pub fn contains_key(&self, key: &MetricKey) -> bool {
        self.by_key.contains_key(key)
    }

    pub fn sections(&self) -> &Vec<Section> {
        &self.sections
    }
}

impl Index<usize> for Descriptors {
    type Output = Rc<Descriptor>;
    fn index(&self, index: usize) -> &Self::Output {
        &self.by_id[index]
    }
}

impl<'de> Deserialize<'de> for Descriptors {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct MapVisitor;
        struct SeqVisitor<'d> {
            descriptors: &'d mut Descriptors,
        }

        impl<'de, 'd> DeserializeSeed<'de> for SeqVisitor<'d> {
            type Value = Self;

            fn deserialize<D: Deserializer<'de>>(
                self,
                deserializer: D,
            ) -> Result<Self::Value, D::Error> {
                deserializer.deserialize_seq(self)
            }
        }

        impl<'de, 'd> Visitor<'de> for SeqVisitor<'d> {
            type Value = Self;

            fn expecting(&self, f: &mut Formatter) -> std::fmt::Result {
                f.write_str("a list of descriptors")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                while let Some(desc) = seq.next_element()? {
                    self.descriptors.add(desc);
                }

                Ok(self)
            }
        }

        impl<'de> Visitor<'de> for MapVisitor {
            type Value = Descriptors;

            fn expecting(&self, f: &mut Formatter) -> std::fmt::Result {
                f.write_str("a map of sections")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut descriptors = Descriptors::new();

                while let Some(name) = map.next_key()? {
                    descriptors.begin_section(name);
                    map.next_value_seed(SeqVisitor { descriptors: &mut descriptors })?;
                }

                Ok(descriptors)
            }
        }

        deserializer.deserialize_map(MapVisitor)
    }
}
