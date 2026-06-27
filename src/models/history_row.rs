use crate::models::history_action::HistoryAction;
use crate::models::history_transaction::HistoryTransaction;

#[derive(Clone, Debug)]
pub enum HistoryRow {
    Transaction(HistoryTransaction),
    Action(HistoryAction),
}
