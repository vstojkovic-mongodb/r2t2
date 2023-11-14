use std::borrow::Borrow;
use std::collections::HashMap;
use std::ops::Index;
use std::rc::Rc;

mod key;
mod time;

pub use self::key::MetricKey;
pub use self::time::{unix_millis_to_timestamp, Timestamp};

pub struct Descriptor {
    pub id: usize,
    pub key: MetricKey,
    pub name: String,
    pub scale: f64,
}

pub struct Descriptors {
    by_id: Vec<Rc<Descriptor>>,
    by_key: HashMap<MetricKey, Vec<Rc<Descriptor>>>,
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

impl Descriptors {
    pub fn new() -> Self {
        Self { by_id: Vec::new(), by_key: HashMap::new() }
    }

    pub fn add(&mut self, mut desc: Descriptor) {
        desc.id = self.by_id.len();
        let desc = Rc::new(desc);

        self.by_id.push(Rc::clone(&desc));
        self.by_key
            .entry(desc.key.clone())
            .or_insert_with(Vec::new)
            .push(desc);
    }

    pub fn contains_key(&self, key: &MetricKey) -> bool {
        self.by_key.contains_key(key)
    }

    pub fn iter(&self) -> impl Iterator<Item = Rc<Descriptor>> + '_ {
        self.by_id.iter().map(|desc| Rc::clone(desc))
    }
}

impl Index<usize> for Descriptors {
    type Output = Rc<Descriptor>;
    fn index(&self, index: usize) -> &Self::Output {
        &self.by_id[index]
    }
}
