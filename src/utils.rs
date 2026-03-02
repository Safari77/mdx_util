use regex::Regex;

pub fn take_chars(s: &str, n: usize) -> &str {
    let byte_end = s
        .char_indices()
        .nth(n)
        .map(|(idx, _)| idx)
        .unwrap_or_else(|| s.len());
    &s[..byte_end]
}

pub fn unescape_entities(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&nbsp;", " ")
}

/// Renders basic HTML formatting (newlines, bold, italic) and colors specific words to the terminal
pub fn render_html_to_terminal(html: &str) -> String {
    let mut text = unescape_entities(html);

    // 1. Replace structural tags with newlines
    let re_newline = Regex::new(r"(?i)<br\s*/?>|</p>|</li>|<div[^>]*>|<tr[^>]*>").unwrap();
    text = re_newline.replace_all(&text, "\n").to_string();

    let re_tab = Regex::new(r"(?i)<td[^>]*>").unwrap();
    text = re_tab.replace_all(&text, "\t").to_string();

    // 2. Highlight parts of speech in RED
    // We do this BEFORE removing HTML tags so word boundaries (\b) work properly.
    // \x1b[31m turns text red, \x1b[39m resets only the color (leaving italics/bold intact)
    let re_pos = Regex::new(r"(?i)\b(transitive verb|intransitive verb|verb|noun|adjective|adverb|pronoun|preposition|conjunction|interjection)\b").unwrap();
    text = re_pos.replace_all(&text, "\x1b[31m$1\x1b[39m").to_string();

    // 3. ANSI escape codes for bold and italic
    // \x1b[1m is bold, \x1b[22m resets bold
    // \x1b[3m is italic, \x1b[23m resets italic
    let re_bold_open = Regex::new(r"(?i)<b[^>]*>|<strong[^>]*>").unwrap();
    let re_bold_close = Regex::new(r"(?i)</b>|</strong>").unwrap();
    let re_italic_open = Regex::new(r"(?i)<i[^>]*>|<em[^>]*>").unwrap();
    let re_italic_close = Regex::new(r"(?i)</i>|</em>").unwrap();

    text = re_bold_open.replace_all(&text, "\x1b[1m").to_string();
    text = re_bold_close.replace_all(&text, "\x1b[22m").to_string();
    text = re_italic_open.replace_all(&text, "\x1b[3m").to_string();
    text = re_italic_close.replace_all(&text, "\x1b[23m").to_string();

    // 4. Remove any other HTML tags (like <span>, <font>, etc.)
    let re_tags = Regex::new(r"<[^>]+>").unwrap();
    text = re_tags.replace_all(&text, "").to_string();

    // 5. Condense multiple blank lines into a single blank line
    let re_multi_nl = Regex::new(r"\n{3,}").unwrap();
    text = re_multi_nl.replace_all(&text, "\n\n").to_string();

    text.trim().to_string()
}
