use crate::models::history_action::HistoryAction;
use crate::models::history_transaction::HistoryTransaction;

const PACMAN_LOG: &str = "/var/log/pacman.log";

pub fn get_update_history(limit: usize) -> Vec<HistoryTransaction> {
    let Ok(content) = std::fs::read_to_string(PACMAN_LOG) else {
        return Vec::new();
    };

    let mut transactions: Vec<HistoryTransaction> = Vec::new();
    let mut last_command: Option<String> = None;
    let mut current: Option<HistoryTransaction> = None;

    for line in content.lines() {
        let Some((timestamp, rest)) = split_log_line(line) else {
            continue;
        };

        if let Some(command) = rest.strip_prefix("[PACMAN] Running ") {
            last_command = Some(command.trim().trim_matches('\'').to_string());
            continue;
        }

        let Some(alpm) = rest.strip_prefix("[ALPM] ") else {
            continue;
        };

        if alpm.starts_with("transaction started") {
            current = Some(HistoryTransaction {
                timestamp: timestamp.to_string(),
                command: last_command.clone(),
                actions: Vec::new(),
            });
            continue;
        }

        if alpm.starts_with("transaction completed") {
            push_transaction(&mut transactions, current.take());
            continue;
        }

        if let Some(action) = parse_action(alpm) {
            if let Some(transaction) = current.as_mut() {
                transaction.actions.push(action);
            }
        }
    }

    push_transaction(&mut transactions, current.take());

    transactions.reverse();
    transactions.truncate(limit);
    return transactions;
}

fn push_transaction(list: &mut Vec<HistoryTransaction>, transaction: Option<HistoryTransaction>) {
    if let Some(transaction) = transaction {
        if !transaction.actions.is_empty() {
            list.push(transaction);
        }
    }
}

fn split_log_line(line: &str) -> Option<(&str, &str)> {
    let line = line.strip_prefix('[')?;
    let end = line.find(']')?;
    let timestamp = &line[..end];
    let rest = line[end + 1..].trim_start();
    return Some((timestamp, rest));
}

fn parse_action(alpm: &str) -> Option<HistoryAction> {
    let mut parts = alpm.splitn(3, ' ');
    let action = parts.next()?;
    if !matches!(
        action,
        "upgraded" | "installed" | "removed" | "downgraded" | "reinstalled"
    ) {
        return None;
    }

    let package = parts.next()?.to_string();
    let inner = parts
        .next()
        .unwrap_or("")
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')');

    let (old_version, new_version) = if let Some((old, new)) = inner.split_once(" -> ") {
        (Some(old.trim().to_string()), Some(new.trim().to_string()))
    } else if inner.is_empty() {
        (None, None)
    } else if action == "removed" {
        (Some(inner.trim().to_string()), None)
    } else {
        (None, Some(inner.trim().to_string()))
    };

    return Some(HistoryAction {
        action: action.to_string(),
        package,
        old_version,
        new_version,
    });
}
