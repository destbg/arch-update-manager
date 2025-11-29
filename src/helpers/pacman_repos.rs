use std::collections::{BTreeSet, HashMap, HashSet};
use std::error::Error;
use std::process::Command;

pub fn group_repos_by_base_server() -> Result<HashMap<String, BTreeSet<String>>, Box<dyn Error>> {
    let repo_out = Command::new("pacman-conf").arg("--repo-list").output()?;
    if !repo_out.status.success() {
        return Err("pacman-conf --repo-list failed".into());
    }

    let repo_names: HashSet<String> = String::from_utf8(repo_out.stdout)?
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    let conf_out = Command::new("pacman-conf").arg("--verbose").output()?;
    if !conf_out.status.success() {
        return Err("pacman-conf --verbose failed".into());
    }

    let conf = String::from_utf8(conf_out.stdout)?;
    let mut current_repo: Option<String> = None;
    let mut servers: HashMap<String, BTreeSet<String>> = HashMap::new();

    for line in conf.lines() {
        let line = line.trim();

        if line.starts_with('[') && line.ends_with(']') {
            let name = &line[1..line.len() - 1];
            if repo_names.contains(name) {
                current_repo = Some(name.to_string());
            } else {
                current_repo = None;
            }
            continue;
        }

        if !line.starts_with("Server") {
            continue;
        }

        let repo = match &current_repo {
            Some(r) => r,
            None => continue,
        };

        if let Some(eq_pos) = line.find('=') {
            let url = line[eq_pos + 1..].trim();
            if url.is_empty() {
                continue;
            }
            let base = base_from_url(url).to_string();
            servers.entry(base).or_default().insert(repo.clone());
        }
    }

    return Ok(servers);
}

pub fn unique_repo_sets(by_server: &HashMap<String, BTreeSet<String>>) -> BTreeSet<Vec<String>> {
    let mut sets = BTreeSet::new();
    for repos in by_server.values() {
        let v: Vec<String> = repos.iter().cloned().collect();
        sets.insert(v);
    }
    return sets;
}

fn base_from_url(url: &str) -> &str {
    let url = url.trim();
    if let Some(pos) = url.find("://") {
        let after = pos + 3;
        let rest = &url[after..];
        if let Some(slash_pos) = rest.find('/') {
            return &url[..after + slash_pos];
        } else {
            return url;
        }
    } else {
        return url.split('/').next().unwrap_or(url);
    }
}
