use std::borrow::{Borrow, Cow};
use std::fmt::{Debug, Formatter};
use std::hash::{Hash, Hasher};

use serde::de::{SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};

#[derive(Clone)]
pub struct MetricKey {
    key: String,
    indices: Vec<(usize, usize)>,
}

impl Debug for MetricKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MetricKey[")?;

        let mut first = true;
        for elem in self.iter() {
            if first {
                first = false;
            } else {
                f.write_str(", ")?;
            }
            f.write_str(elem)?;
        }

        f.write_str("]")?;
        Ok(())
    }
}

impl Borrow<str> for MetricKey {
    fn borrow(&self) -> &str {
        &self.key
    }
}

impl Hash for MetricKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key.hash(state)
    }
}

impl PartialEq for MetricKey {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl Eq for MetricKey {}

impl PartialOrd for MetricKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(&other))
    }
}

impl Ord for MetricKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.key.cmp(&other.key)
    }
}

impl<S: AsRef<str>> From<&[S]> for MetricKey {
    fn from(elems: &[S]) -> Self {
        let mut result = Self::new();
        for elem in elems {
            result.push(elem.as_ref());
        }
        result
    }
}

impl<'de> Deserialize<'de> for MetricKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct KeyVisitor;

        impl<'de> Visitor<'de> for KeyVisitor {
            type Value = MetricKey;

            fn expecting(&self, f: &mut Formatter) -> std::fmt::Result {
                f.write_str("a nonempty sequence of strings")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut key = MetricKey::new();

                // NOTE: We have to use Cow, because JSON deserialization might need to unescape
                // the value, which would require ownership. For more info, see:
                // https://github.com/serde-rs/serde/issues/1413#issuecomment-494892266
                key.push(
                    seq.next_element::<Cow<str>>()?
                        .ok_or_else(|| serde::de::Error::custom("key cannot be empty"))?
                        .as_ref(),
                );
                while let Some(elem) = seq.next_element::<Cow<str>>()? {
                    key.push(elem.as_ref());
                }

                Ok(key)
            }
        }

        deserializer.deserialize_seq(KeyVisitor)
    }
}

impl MetricKey {
    pub fn new() -> Self {
        Self { key: String::new(), indices: vec![] }
    }

    pub fn push(&mut self, elem: &str) {
        if !self.indices.is_empty() {
            self.key.push('\0');
        }
        let start = self.key.len();
        let end = start + elem.len();
        self.indices.push((start, end));
        self.key.push_str(elem);
    }

    pub fn len(&self) -> usize {
        self.indices.len()
    }

    pub fn truncate(&mut self, len: usize) {
        if len >= self.len() {
            return;
        }

        if len == 0 {
            self.key.truncate(0);
            self.indices.truncate(0);
            return;
        }

        self.key.truncate(self.indices[len - 1].1);
        self.indices.truncate(len);
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.indices
            .iter()
            .map(|&(start, end)| &self.key[start..end])
    }
}
