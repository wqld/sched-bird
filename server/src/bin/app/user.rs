use scylla::FromRow;

#[derive(Debug, Default, Clone, FromRow)]
pub struct User {
    pub id: String,
    pub channel: String,
}
