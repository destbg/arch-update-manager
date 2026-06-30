use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum CheckSchedule {
    Hourly,
    EverySixHours,
    EveryTwelveHours,
    Daily,
    Weekly,
}

impl Default for CheckSchedule {
    fn default() -> Self {
        return CheckSchedule::Daily;
    }
}

impl CheckSchedule {
    pub fn id(&self) -> &'static str {
        return match self {
            CheckSchedule::Hourly => "hourly",
            CheckSchedule::EverySixHours => "6h",
            CheckSchedule::EveryTwelveHours => "12h",
            CheckSchedule::Daily => "daily",
            CheckSchedule::Weekly => "weekly",
        };
    }

    pub fn from_id(id: &str) -> CheckSchedule {
        return match id {
            "hourly" => CheckSchedule::Hourly,
            "6h" => CheckSchedule::EverySixHours,
            "12h" => CheckSchedule::EveryTwelveHours,
            "weekly" => CheckSchedule::Weekly,
            _ => CheckSchedule::Daily,
        };
    }

    pub fn label(&self) -> &'static str {
        return match self {
            CheckSchedule::Hourly => "Hourly",
            CheckSchedule::EverySixHours => "Every 6 hours",
            CheckSchedule::EveryTwelveHours => "Every 12 hours",
            CheckSchedule::Daily => "Daily",
            CheckSchedule::Weekly => "Weekly",
        };
    }

    pub fn to_oncalendar(&self) -> &'static str {
        return match self {
            CheckSchedule::Hourly => "hourly",
            CheckSchedule::EverySixHours => "0/6:00",
            CheckSchedule::EveryTwelveHours => "0/12:00",
            CheckSchedule::Daily => "daily",
            CheckSchedule::Weekly => "weekly",
        };
    }

    pub fn all() -> [CheckSchedule; 5] {
        return [
            CheckSchedule::Hourly,
            CheckSchedule::EverySixHours,
            CheckSchedule::EveryTwelveHours,
            CheckSchedule::Daily,
            CheckSchedule::Weekly,
        ];
    }
}
