use chrono::NaiveDate;
use scylla::{frame::value::Timestamp, FromRow};

#[derive(Debug, FromRow, Clone, PartialEq)]
pub struct Sched {
    pub group: String,
    pub id: String,
    pub sched: String,
    pub date_at: NaiveDate,
    pub create_at: Timestamp,
}
