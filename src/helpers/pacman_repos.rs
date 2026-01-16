use std::collections::{BTreeSet, HashMap, HashSet};
use std::error::Error;
use std::process::Command;

pub fn get_repository_groups() -> Result<Vec<Vec<String>>, Box<dyn Error>> {
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

    let mut seen_sets: HashSet<Vec<String>> = HashSet::new();
    for repos in servers.into_values() {
        let mut repos_vec: Vec<String> = repos.into_iter().collect();
        repos_vec.sort();
        seen_sets.insert(repos_vec);
    }

    let mut result: Vec<Vec<String>> = seen_sets.into_iter().collect();
    result.sort_by(|a, b| a.join(",").cmp(&b.join(",")));

    return Ok(result);
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
