use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct NewsItem {
    pub title: String,
    pub link: String,
    pub pub_date: DateTime<Utc>,
    pub body: String,
}
