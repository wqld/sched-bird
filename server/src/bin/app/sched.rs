use chrono::NaiveDate;
use scylla::{frame::value::Timestamp, FromRow};
use serde::ser::{Serialize, SerializeStruct, Serializer};

#[derive(Debug, FromRow, Clone, PartialEq)]
pub struct Sched {
    pub channel: String,
    pub id: String,
    pub sched: String,
    pub date_at: NaiveDate,
    pub create_at: Timestamp,
}

impl Serialize for Sched {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Sched", 5)?;
        state.serialize_field("channel", &self.channel)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("sched", &self.sched)?;
        state.serialize_field("date_at", &self.date_at)?;
        state.serialize_field("create_at", "123")?;
        state.end()
    }
}
