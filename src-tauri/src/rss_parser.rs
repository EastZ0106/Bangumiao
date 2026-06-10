use quick_xml::de::from_str;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashSet;

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename = "rss")]
pub struct RssFeed {
    pub channel: RssChannel,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct RssChannel {
    pub title: String,
    pub link: String,
    #[serde(rename = "item", default)]
    pub items: Vec<RssItem>,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct RssItem {
    pub title: String,
    pub link: String,
    #[serde(rename = "enclosure", default)]
    pub enclosure: Option<Enclosure>,
    #[serde(rename = "pubDate", default)]
    pub pub_date: String,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct Enclosure {
    #[serde(rename = "@url")]
    pub url: String,
    #[serde(rename = "@type")]
    pub r#type: String,
    #[serde(rename = "@length", default)]
    pub length: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct EpisodeInfo {
    pub title: String,
    pub torrent_url: String,
    pub magnet_uri: String,
    pub pub_date: String,
    pub episode_number: Option<f64>,
}

pub fn parse_rss(xml: &str) -> Result<RssFeed, Box<dyn std::error::Error>> {
    let feed: RssFeed = from_str(xml)?;
    Ok(feed)
}

pub fn extract_new_episodes(
    feed: &RssFeed,
    seen_titles: &HashSet<String>,
) -> Vec<EpisodeInfo> {
    feed.channel
        .items
        .iter()
        .filter(|item| !seen_titles.contains(&item.title))
        .map(|item| {
            let torrent_url = item
                .enclosure
                .as_ref()
                .map(|e| e.url.clone())
                .unwrap_or_else(|| item.link.clone());
            let magnet_uri = extract_magnet(&item.link);
            EpisodeInfo {
                title: item.title.clone(),
                torrent_url,
                magnet_uri,
                pub_date: item.pub_date.clone(),
                episode_number: extract_episode_number(&item.title),
            }
        })
        .collect()
}

fn extract_magnet(link: &str) -> String {
    // Mikanani sometimes puts magnet in the link field
    if link.starts_with("magnet:") {
        link.to_string()
    } else {
        String::new()
    }
}

pub fn extract_episode_number(title: &str) -> Option<f64> {
    // Patterns like: 第01话, 第1話, ep01, EP01, #01, - 01, etc.
    let patterns = [
        regex_lite::Regex::new(r"第\s*(\d+(?:\.\d+)?)\s*[话話]"),
        regex_lite::Regex::new(r"[Ee][Pp]?\s*(\d+(?:\.\d+)?)"),
        regex_lite::Regex::new(r"#\s*(\d+(?:\.\d+)?)"),
        regex_lite::Regex::new(r"-\s*(\d+(?:\.\d+)?)\s*(?:\[|$)"),
    ];

    for re in patterns.iter().flatten() {
        if let Some(caps) = re.captures(title) {
            if let Some(m) = caps.get(1) {
                if let Ok(n) = m.as_str().parse::<f64>() {
                    return Some(n);
                }
            }
        }
    }
    None
}
