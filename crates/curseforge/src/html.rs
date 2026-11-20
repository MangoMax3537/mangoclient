//! HTML to markdown-lite text. CurseForge descriptions are HTML written by
//! third parties; nothing of it may reach the UI as markup.
//!
//! Kept: paragraphs, line breaks, headings (`## `), list items (`- `) and
//! http(s) links as `[text](url)`. Dropped: every other tag, `script`/`style`
//! contents, images, all attributes, comments.

const MAX_INPUT: usize = 512 * 1024;

fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let end = rest.find(';').filter(|e| *e <= 10);
        let Some(end) = end else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let name = &rest[1..end];
        let decoded = match name {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            n if n.starts_with("#x") || n.starts_with("#X") => u32::from_str_radix(&n[2..], 16)
                .ok()
                .and_then(char::from_u32),
            n if n.starts_with('#') => n[1..].parse::<u32>().ok().and_then(char::from_u32),
            _ => None,
        };
        match decoded {
            Some(c) if c != '\0' => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            _ => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Value of attribute `name` inside a tag body.
fn attr(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(i) = lower[from..].find(name) {
        let at = from + i;
        from = at + name.len();
        let before_ok = at == 0 || lower.as_bytes()[at - 1].is_ascii_whitespace();
        let after = tag[from..].trim_start();
        let Some(value) = after.strip_prefix('=') else {
            continue;
        };
        if !before_ok {
            continue;
        }
        let value = value.trim_start();
        let (quote, body) = match value.chars().next() {
            Some(q @ ('"' | '\'')) => (Some(q), &value[1..]),
            _ => (None, value),
        };
        let end = match quote {
            Some(q) => body.find(q)?,
            None => body.find(char::is_whitespace).unwrap_or(body.len()),
        };
        return Some(decode_entities(&body[..end]));
    }
    None
}

fn safe_link(href: &str) -> bool {
    let h = href.trim();
    (h.starts_with("https://") || h.starts_with("http://"))
        && !h.contains(['(', ')', ' ', '\n', '\r'])
        && h.len() <= 2048
}

pub fn html_to_text(html: &str) -> String {
    let html = if html.len() > MAX_INPUT {
        let mut cut = MAX_INPUT;
        while !html.is_char_boundary(cut) {
            cut -= 1;
        }
        &html[..cut]
    } else {
        html
    };
    let mut out = String::new();
    // Link currently open: (href, byte offset in `out` where its text starts).
    let mut link: Option<(String, usize)> = None;
    let mut rest = html;
    while !rest.is_empty() {
        let Some(lt) = rest.find('<') else {
            out.push_str(&decode_entities(rest));
            break;
        };
        out.push_str(&decode_entities(&rest[..lt]));
        rest = &rest[lt..];
        if let Some(after) = rest.strip_prefix("<!--") {
            rest = after.find("-->").map_or("", |e| &after[e + 3..]);
            continue;
        }
        let Some(gt) = rest.find('>') else {
            // A lone `<` in text.
            out.push_str(&decode_entities(rest));
            break;
        };
        let body = &rest[1..gt];
        rest = &rest[gt + 1..];
        let closing = body.starts_with('/');
        let name: String = body
            .trim_start_matches('/')
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase();
        match name.as_str() {
            "script" | "style" if !closing => {
                // Skip to the matching close tag.
                let lower = rest.to_ascii_lowercase();
                let close = format!("</{name}");
                rest = match lower.find(&close) {
                    Some(i) => {
                        let tail = &rest[i..];
                        tail.find('>').map_or("", |g| &tail[g + 1..])
                    }
                    None => "",
                };
            }
            "br" => out.push('\n'),
            "p" | "div" | "ul" | "ol" | "table" | "tr" | "blockquote" | "pre" | "hr" => {
                out.push_str("\n\n")
            }
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                if closing {
                    out.push_str("\n\n");
                } else {
                    out.push_str("\n\n## ");
                }
            }
            "li" if !closing => out.push_str("\n- "),
            "li" => out.push('\n'),
            "a" if !closing => {
                if let Some(href) = attr(body, "href").filter(|h| safe_link(h)) {
                    link = Some((href.trim().to_string(), out.len()));
                }
            }
            "a" => {
                if let Some((href, start)) = link.take() {
                    let text = out.split_off(start);
                    let text = text.trim();
                    if text.is_empty() || text == href {
                        out.push_str(&href);
                    } else {
                        out.push_str(&format!("[{}]({href})", text.replace(['[', ']'], "")));
                    }
                }
            }
            _ => {}
        }
    }
    // Real tags are gone: a `<` left over came from text or an entity and
    // must not be able to start markup in a renderer downstream.
    tidy(&out).replace('<', "\u{2039}")
}

/// Trim lines, collapse runs of blank lines to one.
fn tidy(s: &str) -> String {
    let mut out = String::new();
    let mut blank = 0;
    for line in s.lines() {
        let line = line.trim_end();
        let line = if line.trim().is_empty() { "" } else { line };
        if line.is_empty() {
            blank += 1;
            continue;
        }
        if !out.is_empty() {
            out.push('\n');
            if blank > 0 {
                out.push('\n');
            }
        }
        blank = 0;
        out.push_str(line.trim_start_matches(' '));
    }
    out
}
