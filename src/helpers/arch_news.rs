use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use regex::Regex;

use crate::helpers::elevated::chown_to_user;
use crate::helpers::network::http_get;
use crate::helpers::tray_state::state_dir;
use crate::models::news_item::NewsItem;
use crate::models::news_state::NewsState;

const NEWS_URL: &str = "https://archlinux.org/feeds/news/";

pub const NEWS_TIMEOUT_SECS: u32 = 5;

pub fn news_to_show() -> Vec<NewsItem> {
    let items = match fetch_arch_news() {
        Ok(items) => items,
        Err(_) => return Vec::new(),
    };

    if items.is_empty() {
        return Vec::new();
    }

    let latest = items[0].pub_date;

    let Some(last_seen) = read_last_seen() else {
        let _ = write_last_seen(latest);
        return Vec::new();
    };

    let new_count = items
        .iter()
        .filter(|item| item.pub_date > last_seen)
        .count();

    if latest > last_seen {
        let _ = write_last_seen(latest);
    }

    if new_count == 0 {
        return Vec::new();
    }

    let show_count = new_count.max(2);
    return items.into_iter().take(show_count).collect();
}

fn fetch_arch_news() -> Result<Vec<NewsItem>> {
    let body = http_get(NEWS_URL, NEWS_TIMEOUT_SECS)?;
    let mut items = parse_news(&body);
    items.sort_by(|a, b| b.pub_date.cmp(&a.pub_date));
    return Ok(items);
}

fn parse_news(xml: &str) -> Vec<NewsItem> {
    let item_re = Regex::new(r"(?s)<item>(.*?)</item>").unwrap();
    let title_re = Regex::new(r"(?s)<title>(.*?)</title>").unwrap();
    let link_re = Regex::new(r"(?s)<link>(.*?)</link>").unwrap();
    let date_re = Regex::new(r"(?s)<pubDate>(.*?)</pubDate>").unwrap();
    let desc_re = Regex::new(r"(?s)<description>(.*?)</description>").unwrap();

    let mut items = Vec::new();

    for cap in item_re.captures_iter(xml) {
        let block = &cap[1];

        let Some(date_raw) = date_re.captures(block).map(|c| c[1].trim().to_string()) else {
            continue;
        };
        let Some(pub_date) = parse_date(&date_raw) else {
            continue;
        };

        let title = title_re
            .captures(block)
            .map(|c| clean_text(&c[1]))
            .unwrap_or_default();
        let link = link_re
            .captures(block)
            .map(|c| c[1].trim().to_string())
            .unwrap_or_default();
        let body = desc_re
            .captures(block)
            .map(|c| html_to_text(&c[1]))
            .unwrap_or_default();

        items.push(NewsItem {
            title,
            link,
            pub_date,
            body,
        });
    }

    return items;
}

fn parse_date(s: &str) -> Option<DateTime<Utc>> {
    return DateTime::parse_from_rfc2822(s.trim())
        .ok()
        .map(|d| d.with_timezone(&Utc));
}

fn clean_text(s: &str) -> String {
    return unescape_entities(strip_cdata(s).trim());
}

fn html_to_text(s: &str) -> String {
    let raw = strip_cdata(s);
    let html = unescape_entities(raw.trim());
    let html = html
        .replace("</p>", "\n\n")
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("</li>", "\n");
    let stripped = strip_tags(&html);
    let text = unescape_entities(&stripped);
    return collapse_blank_lines(text.trim());
}

fn strip_cdata(s: &str) -> String {
    return s.replace("<![CDATA[", "").replace("]]>", "");
}

fn strip_tags(s: &str) -> String {
    let re = Regex::new(r"(?s)<[^>]*>").unwrap();
    return re.replace_all(s, "").into_owned();
}

fn unescape_entities(s: &str) -> String {
    let out = s
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ");
    return out.replace("&amp;", "&");
}

fn collapse_blank_lines(s: &str) -> String {
    let re = Regex::new(r"\n{3,}").unwrap();
    return re.replace_all(s, "\n\n").into_owned();
}

fn news_state_file() -> Option<PathBuf> {
    return state_dir().map(|d| d.join("news_seen.json"));
}

fn read_last_seen() -> Option<DateTime<Utc>> {
    let path = news_state_file()?;
    let content = fs::read_to_string(&path).ok()?;
    let state: NewsState = serde_json::from_str(&content).ok()?;
    return Some(state.last_seen);
}

fn write_last_seen(last_seen: DateTime<Utc>) -> Result<()> {
    let dir = state_dir().ok_or_else(|| anyhow::anyhow!("Could not determine state directory"))?;
    fs::create_dir_all(&dir).context("Failed to create state directory")?;
    chown_to_user(&dir);

    let path =
        news_state_file().ok_or_else(|| anyhow::anyhow!("Could not determine news state path"))?;
    let state = NewsState { last_seen };
    let content = serde_json::to_string_pretty(&state).context("Failed to serialize news state")?;
    fs::write(&path, content).context("Failed to write news state file")?;
    chown_to_user(&path);

    return Ok(());
}
