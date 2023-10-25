use chrono::{DateTime, Utc};

pub type Timestamp = DateTime<Utc>;

pub fn unix_millis_to_timestamp(millis: i64) -> Timestamp {
    const NANOS_PER_MILLISECOND: u32 = std::time::Duration::from_millis(1).as_nanos() as u32;
    let secs = millis / 1000;
    let nanos = (millis % 1000) as u32 * NANOS_PER_MILLISECOND;
    DateTime::from_timestamp(secs, nanos).unwrap()
}
