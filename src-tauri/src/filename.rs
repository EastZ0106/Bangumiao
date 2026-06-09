use std::path::Path;

pub struct ParsedFilename {
    pub anime_title: String,
    pub episode_number: Option<f64>,
    pub raw_filename: String,
}

pub fn parse(filename: &str) -> ParsedFilename {
    let name = strip_extension(filename);

    // Strategy: extract tags in brackets, then infer title and episode number
    let (_cleaned, title) = extract_title(name);

    let ep = parse_episode_number(name);

    ParsedFilename {
        anime_title: title,
        episode_number: ep,
        raw_filename: filename.to_string(),
    }
}

fn strip_extension(filename: &str) -> &str {
    // Remove file extension and parenthesized quality tags at the end
    let base = Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(filename);
    base
}

fn extract_title(name: &str) -> (String, String) {
    // Remove known bracket tags to isolate the title

    // Try to find the main title: everything after the first bracket group(s)
    // Example: [LoliHouse] Youkoso Jitsuryoku... S4 - 01 [WebRip 1080p...]
    // Title should be: Youkoso Jitsuryoku Shijou Shugi no Kyoushitsu e S4

    // Strategy: collect text between first non-subtitle-info bracket and last info bracket
    let parts: Vec<&str> = name.split(&['[', ']'][..]).collect();
    let mut text_parts: Vec<&str> = Vec::new();
    let mut idx = 0;

    while idx < parts.len() {
        let part = parts[idx].trim();
        if part.is_empty() {
            idx += 1;
            continue;
        }

        // Check if this part looks like a subtitle group / codec / resolution tag
        if is_tag_part(part) {
            idx += 1;
            continue;
        }

        // This is likely part of the title
        text_parts.push(part);
        idx += 1;
    }

    let mut title = text_parts
        .join(" ")
        .trim()
        .to_string();

    if title.is_empty() {
        title = name.to_string();
    }

    // Remove episode number suffix from title
    title = strip_episode_suffix(&title);

    let result = name.to_string();

    (result, title)
}

fn is_tag_part(s: &str) -> bool {
    let _upper = s.to_uppercase();
    let tag_keywords = [
        "MP4", "MKV", "AVI", "HEVC", "H264", "H265", "X264", "X265", "AAC", "EAC3",
        "FLAC", "OPUS", "1080P", "720P", "480P", "2160P", "4K", "WEBRIP", "BDRIP",
        "WEB-DL", "BD", "CHS", "CHT", "GB", "BIG5", "JP", "CHI", "JPN", "SRT",
        "ASS", "PGS", "10BIT", "8BIT", "HI10P", "SUBPLEASE", "ERAWP", "RAW",
        "MOVIE", "OVA", "OAD", "ONA", "TV", "FINALE", "V2", "V3", "REV",
    ];

    // Check if s contains mostly known tag patterns
    let parts: Vec<&str> = s.split(&['&', '-', '_', ' ', '+'][..]).collect();
    let tag_count = parts.iter().filter(|p| {
        let p_upper = p.to_uppercase();
        tag_keywords.iter().any(|tag| p_upper == *tag || p_upper.starts_with(tag))
    }).count();

    if parts.is_empty() {
        return false;
    }

    // Check for subtitle group patterns (often ending with &xxx or just names)
    let subtitle_groups = [
        "LOLIHOUSE", "BEANSUB", "FZSD", "PRE-S", "Y-RAWS", "NC-RAWS",
        "COOLGIRLS", "MAKAIKAISUB", "SWSUB", "KISSSUB", "SAKURATOON",
    ];

    // If more than half the parts are tags, treat it as a tag part
    tag_count as f64 / parts.len() as f64 >= 0.5
        || parts.iter().any(|p| subtitle_groups.contains(&p))
        || parts.iter().any(|p| {
            let p_upper = p.to_uppercase();
            subtitle_groups.iter().any(|g| p_upper == *g || p_upper.contains(g))
        })
}

fn strip_episode_suffix(title: &str) -> String {
    let patterns = [
        (r"\s*[-–]\s*\d+(?:\.\d+)?\s*$", ""),
        (r"\s*[第#][\d.]+[话話]\s*$", ""),
        (r"\s*[Ee][Pp]?\s*\d+(?:\.\d+)?\s*$", ""),
    ];

    let mut result = title.to_string();
    for (pat, _) in &patterns {
        if let Ok(re) = regex_lite::Regex::new(pat) {
            result = re.replace(&result, "").to_string();
        }
    }
    result.trim().to_string()
}

fn parse_episode_number(name: &str) -> Option<f64> {
    // Try various common patterns
    let patterns = [
        regex_lite::Regex::new(r"(?:^|\s)[-–]\s*(\d+(?:\.\d+)?)\s*(?:\s|\[|$)"),  // " - 01" or "- 01 ["
        regex_lite::Regex::new(r"第\s*(\d+(?:\.\d+)?)\s*[话話]"),      // "第01话"
        regex_lite::Regex::new(r"[Ee][Pp]\s*(\d+(?:\.\d+)?)"),         // "EP01", "ep 01"
        regex_lite::Regex::new(r"#\s*(\d+(?:\.\d+)?)"),                // "#01"
        regex_lite::Regex::new(r"\s(\d{2,3})\s"),                       // standalone " 01 " (less confident)
    ];

    for re in patterns.iter() {
        if let Ok(re) = re {
            if let Some(caps) = re.captures(name) {
                if let Some(m) = caps.get(1) {
                    if let Ok(n) = m.as_str().parse::<f64>() {
                        return Some(n);
                    }
                }
            }
        }
    }

    // Fallback: find any isolated number that looks like an episode
    if let Ok(re) = regex_lite::Regex::new(r"[-–\s](\d{2,3})(?:\s|\[|\.\w+$)") {
        if let Some(caps) = re.captures(name) {
            if let Some(m) = caps.get(1) {
                if let Ok(n) = m.as_str().parse::<f64>() {
                    if n >= 1.0 && n <= 999.0 {
                        return Some(n);
                    }
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lolihouse() {
        let r = parse("[LoliHouse] Youkoso Jitsuryoku Shijou Shugi no Kyoushitsu e S4 - 01 [WebRip 1080p HEVC-10bit AAC SRTx2].mkv");
        assert!(r.anime_title.contains("Youkoso"));
        assert_eq!(r.episode_number, Some(1.0));
    }

    #[test]
    fn test_multi_group() {
        let r = parse("[Pre-S&三明治摸鱼部&Y-Raws] 超时空辉夜姬 Cosmic Princess Kaguya [WebRip 1080P HEVC 10Bit EAC3&AAC MKV][CHI_JPN].mkv");
        assert!(r.anime_title.contains("Kaguya") || r.anime_title.contains("辉夜姬"));
    }

    #[test]
    fn test_beansub() {
        let r = parse("[BeanSub&FZSD][Chainsaw_Man_Reze_Arc][MOVIE][GB][1080P][x264_AAC].mp4");
        assert!(r.anime_title.contains("Chainsaw"));
    }
}
