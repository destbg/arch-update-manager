use crate::models::history_action::HistoryAction;

#[derive(Clone, Debug)]
pub struct HistoryTransaction {
    pub timestamp: String,
    pub command: Option<String>,
    pub actions: Vec<HistoryAction>,
}

impl HistoryTransaction {
    pub fn summary(&self) -> String {
        let mut counts: Vec<(&str, usize)> = Vec::new();
        for kind in [
            "upgraded",
            "downgraded",
            "installed",
            "reinstalled",
            "removed",
        ] {
            let count = self.actions.iter().filter(|a| a.action == kind).count();
            if count > 0 {
                counts.push((kind, count));
            }
        }
        return counts
            .iter()
            .map(|(kind, count)| format!("{} {}", count, kind))
            .collect::<Vec<_>>()
            .join(", ");
    }
}
