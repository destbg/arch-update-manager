pub fn release_notes_url(homepage: &str) -> Option<String> {
    let stripped = homepage
        .strip_prefix("https://")
        .or_else(|| homepage.strip_prefix("http://"))?;

    let (host, path) = stripped.split_once('/')?;
    let host = host.trim_end_matches('.');

    let segments: Vec<&str> = path
        .trim_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();

    if segments.len() < 2 {
        return None;
    }

    if host.eq_ignore_ascii_case("github.com") {
        let owner = segments[0];
        let repo = segments[1].trim_end_matches(".git");
        return Some(format!("https://github.com/{}/{}/releases", owner, repo));
    }

    if is_gitlab_host(host) {
        let mut project_segments: Vec<&str> = Vec::new();
        for seg in &segments {
            if *seg == "-" {
                break;
            }
            project_segments.push(seg);
        }
        if project_segments.len() < 2 {
            return None;
        }
        if let Some(last) = project_segments.last_mut() {
            *last = last.trim_end_matches(".git");
        }
        return Some(format!(
            "https://{}/{}/-/releases",
            host,
            project_segments.join("/")
        ));
    }

    return None;
}

fn is_gitlab_host(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    if host == "gitlab.com" {
        return true;
    }
    return host.starts_with("gitlab.") || host.contains(".gitlab.");
}
