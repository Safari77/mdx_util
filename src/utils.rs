use std::cell::RefCell;

use lol_html::html_content::ContentType;
use lol_html::{element, text, EndTagHandler, HtmlRewriter, Settings};

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

// ANSI escape codes — basic
const BOLD_ON: &str = "\x1b[1m";
const BOLD_OFF: &str = "\x1b[22m";
const ITALIC_ON: &str = "\x1b[3m";
const ITALIC_OFF: &str = "\x1b[23m";
const UNDERLINE_ON: &str = "\x1b[4m";
const UNDERLINE_OFF: &str = "\x1b[24m";
#[allow(dead_code)]
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const CYAN: &str = "\x1b[36m";
const DIM: &str = "\x1b[2m";
const DIM_OFF: &str = "\x1b[22m";
const COLOR_RESET: &str = "\x1b[39m";
const RESET_ALL: &str = "\x1b[0m";

// Indentation for se4 (definition sense) and se8 (quotation) blocks
const SE4_INDENT: &str = "  ";
const SE8_INDENT: &str = "    ";

/// Helper: push an end-tag handler onto the element.
/// Uses the lol_html >= 1.0 API: `el.end_tag_handlers()`.
/// Coerces the closure into `EndTagHandler<'static>` (Box<dyn FnOnce(...)>).
/// If the element can't have an end tag (void element), this is a no-op.
macro_rules! push_end_tag_handler {
    ($el:expr, $handler:expr) => {
        if let Some(handlers) = $el.end_tag_handlers() {
            let h: EndTagHandler = Box::new($handler);
            handlers.push(h);
        }
    };
}

/// Parse a CSS/HTML color value and return an ANSI escape sequence.
/// Supports:
///   - 6-digit hex: "CA0000", "#CA0000"
///   - 3-digit hex: "#F00"
///   - CSS named colors (common subset)
/// Returns 24-bit truecolor ANSI for kitty/modern terminals,
/// with fallback to basic 16-color for well-known names.
fn color_to_ansi(color: &str) -> Option<String> {
    let color = color.trim().trim_matches('"').trim_matches('\'');
    if color.is_empty() {
        return None;
    }

    // Try named colors first (case-insensitive)
    let lower = color.to_ascii_lowercase();
    match lower.as_str() {
        // Basic ANSI colors — use standard codes for maximum compatibility
        "black" => return Some("\x1b[30m".to_string()),
        "red" => return Some("\x1b[31m".to_string()),
        "green" => return Some("\x1b[32m".to_string()),
        "yellow" => return Some("\x1b[33m".to_string()),
        "blue" => return Some("\x1b[34m".to_string()),
        "magenta" | "fuchsia" => return Some("\x1b[35m".to_string()),
        "cyan" | "aqua" => return Some("\x1b[36m".to_string()),
        "white" => return Some("\x1b[37m".to_string()),
        // Bright variants
        "gray" | "grey" => return Some("\x1b[90m".to_string()),
        "lightgray" | "lightgrey" | "silver" => return Some("\x1b[37m".to_string()),
        // Named colors → 24-bit truecolor (kitty supports this natively)
        "darkred" | "maroon" => return Some(format!("\x1b[38;2;128;0;0m")),
        "darkgreen" => return Some(format!("\x1b[38;2;0;100;0m")),
        "darkblue" | "navy" => return Some(format!("\x1b[38;2;0;0;128m")),
        "darkcyan" | "teal" => return Some(format!("\x1b[38;2;0;128;128m")),
        "darkmagenta" | "purple" => return Some(format!("\x1b[38;2;128;0;128m")),
        "darkorange" => return Some(format!("\x1b[38;2;255;140;0m")),
        "darkslategray" | "darkslategrey" => return Some(format!("\x1b[38;2;47;79;79m")),
        "slategray" | "slategrey" => return Some(format!("\x1b[38;2;112;128;144m")),
        "dimgray" | "dimgrey" => return Some(format!("\x1b[38;2;105;105;105m")),
        "olive" => return Some(format!("\x1b[38;2;128;128;0m")),
        "olivedrab" => return Some(format!("\x1b[38;2;107;142;35m")),
        "brown" | "saddlebrown" => return Some(format!("\x1b[38;2;139;69;19m")),
        "sienna" => return Some(format!("\x1b[38;2;160;82;45m")),
        "chocolate" => return Some(format!("\x1b[38;2;210;105;30m")),
        "firebrick" => return Some(format!("\x1b[38;2;178;34;34m")),
        "crimson" => return Some(format!("\x1b[38;2;220;20;60m")),
        "indianred" => return Some(format!("\x1b[38;2;205;92;92m")),
        "tomato" => return Some(format!("\x1b[38;2;255;99;71m")),
        "orangered" => return Some(format!("\x1b[38;2;255;69;0m")),
        "coral" => return Some(format!("\x1b[38;2;255;127;80m")),
        "salmon" => return Some(format!("\x1b[38;2;250;128;114m")),
        "gold" => return Some(format!("\x1b[38;2;255;215;0m")),
        "khaki" => return Some(format!("\x1b[38;2;240;230;140m")),
        "limegreen" => return Some(format!("\x1b[38;2;50;205;50m")),
        "forestgreen" => return Some(format!("\x1b[38;2;34;139;34m")),
        "seagreen" => return Some(format!("\x1b[38;2;46;139;87m")),
        "steelblue" => return Some(format!("\x1b[38;2;70;130;180m")),
        "royalblue" => return Some(format!("\x1b[38;2;65;105;225m")),
        "dodgerblue" => return Some(format!("\x1b[38;2;30;144;255m")),
        "cornflowerblue" => return Some(format!("\x1b[38;2;100;149;237m")),
        "cadetblue" => return Some(format!("\x1b[38;2;95;158;160m")),
        "deepskyblue" => return Some(format!("\x1b[38;2;0;191;255m")),
        "mediumblue" => return Some(format!("\x1b[38;2;0;0;205m")),
        "midnightblue" => return Some(format!("\x1b[38;2;25;25;112m")),
        "blueviolet" => return Some(format!("\x1b[38;2;138;43;226m")),
        "darkviolet" => return Some(format!("\x1b[38;2;148;0;211m")),
        "darkorchid" => return Some(format!("\x1b[38;2;153;50;204m")),
        "mediumorchid" => return Some(format!("\x1b[38;2;186;85;211m")),
        "orchid" => return Some(format!("\x1b[38;2;218;112;214m")),
        "violet" => return Some(format!("\x1b[38;2;238;130;238m")),
        "plum" => return Some(format!("\x1b[38;2;221;160;221m")),
        "hotpink" => return Some(format!("\x1b[38;2;255;105;180m")),
        "deeppink" => return Some(format!("\x1b[38;2;255;20;147m")),
        "pink" => return Some(format!("\x1b[38;2;255;192;203m")),
        "rosybrown" => return Some(format!("\x1b[38;2;188;143;143m")),
        "tan" => return Some(format!("\x1b[38;2;210;180;140m")),
        "peru" => return Some(format!("\x1b[38;2;205;133;63m")),
        "burlywood" => return Some(format!("\x1b[38;2;222;184;135m")),
        "wheat" => return Some(format!("\x1b[38;2;245;222;179m")),
        _ => {}
    }

    // Try hex color
    let hex = color.strip_prefix('#').unwrap_or(color);
    let (r, g, b) = match hex.len() {
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            (r, g, b)
        }
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            (r, g, b)
        }
        _ => return None,
    };

    // Use 24-bit truecolor — kitty supports this natively
    Some(format!("\x1b[38;2;{};{};{}m", r, g, b))
}

/// Renders MDX dictionary HTML to terminal with ANSI colors, bold, italic.
///
/// Works with any dictionary format — handles both standard HTML tags and
/// dictionary-specific custom tags (OED4, Webster, etc.) without configuration.
///
/// Standard HTML handled:
/// - `<b>`, `<strong>` → bold
/// - `<i>`, `<em>` → italic
/// - `<u>` → underline
/// - `<br>` → newline
/// - `<font color="...">` → ANSI color (truecolor for kitty)
/// - `<font size="+1">` → bold (terminal has no font sizes)
/// - `<a href="entry://...">` → cyan underline with target shown
/// - `<script>`, `<style>`, `<link>` → stripped
/// - `&nbsp;` → preserved as space (used for indentation in Webster etc.)
///
/// OED4-specific:
/// - `<se0>`, `<se4>`, `<se8>` → structural indentation
/// - `<d>` → yellow (date), `<ch>` → bold (author), `<qt>` → italic (quote)
/// - `<ph>` → green (phonetic), `<hw>` → bold+underline (headword)
/// - `<ls>` → bold (sense label), `<w>` → dim (abbreviation)
/// - `<spg>` → dim (spelling group), `<dg>` → newline (etymology)
///
/// Other dictionaries:
/// - `<com>` → passthrough (Webster comments/metadata)
/// - Unknown tags → stripped, content kept
pub fn render_html_to_terminal(html: &str) -> String {
    let result = RefCell::new(String::with_capacity(html.len()));
    let indent_level: RefCell<u8> = RefCell::new(0);

    // Helper to get indent string for a level
    fn indent_str(level: u8) -> &'static str {
        match level {
            8 => SE8_INDENT,
            4 => SE4_INDENT,
            _ => "",
        }
    }

    let settings = Settings {
        element_content_handlers: vec![
            // === Strip <script> and all content inside ===
            element!("script", {
                move |el| {
                    el.remove();
                    Ok(())
                }
            }),
            text!("script", {
                move |t| {
                    t.remove();
                    Ok(())
                }
            }),
            // === Strip <style> and all content inside ===
            element!("style", {
                move |el| {
                    el.remove();
                    Ok(())
                }
            }),
            text!("style", {
                move |t| {
                    t.remove();
                    Ok(())
                }
            }),
            // === Strip <link> (self-closing, no content) ===
            element!("link", {
                move |el| {
                    el.remove();
                    Ok(())
                }
            }),
            // === <font> tag: handle color and size attributes ===
            // Works for Webster (<font color="CA0000">, <font size=+1>)
            // and any other dictionary using <font> tags
            element!("font", {
                move |el| {
                    let mut did_color = false;

                    // Handle color attribute
                    if let Some(color_val) = el.get_attribute("color") {
                        if let Some(ansi) = color_to_ansi(&color_val) {
                            el.before(&ansi, ContentType::Html);
                            did_color = true;
                        }
                    }

                    // Handle size attribute: size=+1 or larger → bold
                    if let Some(size_val) = el.get_attribute("size") {
                        let size_str = size_val.trim();
                        let is_large = size_str.starts_with('+')
                            || size_str.parse::<i32>().map_or(false, |n| n > 3);
                        if is_large {
                            el.before(BOLD_ON, ContentType::Html);
                            let needs_color_reset = did_color;
                            push_end_tag_handler!(el, move |end| {
                                let mut reset = BOLD_OFF.to_string();
                                if needs_color_reset {
                                    reset.push_str(COLOR_RESET);
                                }
                                end.before(&reset, ContentType::Html);
                                end.remove();
                                Ok(())
                            });
                            el.remove_and_keep_content();
                            return Ok(());
                        }
                    }

                    // If only color (no size), reset color at end
                    if did_color {
                        push_end_tag_handler!(el, |end| {
                            end.before(COLOR_RESET, ContentType::Html);
                            end.remove();
                            Ok(())
                        });
                    }

                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            // === Structural OED4 tags ===

            // <se0> headword entry line: double newline before
            element!("se0", {
                let indent = &indent_level;
                move |el| {
                    *indent.borrow_mut() = 0;
                    el.before("\n\n", ContentType::Text);
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            // <se4> definition/sense block: indent only (OED source has line breaks between blocks)
            element!("se4", {
                let indent = &indent_level;
                move |el| {
                    *indent.borrow_mut() = 4;
                    el.before(SE4_INDENT, ContentType::Text);
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            // <se8> quotation block: indent only (OED source has line breaks between blocks)
            element!("se8", {
                let indent = &indent_level;
                move |el| {
                    *indent.borrow_mut() = 8;
                    el.before(SE8_INDENT, ContentType::Text);
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            // <q> individual quotation: indent only
            element!("q", {
                let indent = &indent_level;
                move |el| {
                    let level = *indent.borrow();
                    el.before(indent_str(level), ContentType::Text);
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            // <seg> separator
            element!("seg", {
                move |el| {
                    el.replace("\n───\n", ContentType::Text);
                    Ok(())
                }
            }),
            // <spg> spelling/forms group: dim text
            element!("spg", {
                move |el| {
                    el.before(DIM, ContentType::Html);
                    push_end_tag_handler!(el, |end| {
                        end.before(DIM_OFF, ContentType::Html);
                        end.remove();
                        Ok(())
                    });
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            // <dg> definition/etymology group
            element!("dg", {
                move |el| {
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            // === Inline formatting: OED4-specific ===

            // <hw> headword: bold + underline
            element!("hw", {
                move |el| {
                    el.before(&format!("{}{}", BOLD_ON, UNDERLINE_ON), ContentType::Html);
                    push_end_tag_handler!(el, |end| {
                        end.before(&format!("{}{}", UNDERLINE_OFF, BOLD_OFF), ContentType::Html);
                        end.remove();
                        Ok(())
                    });
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            // <ph> phonetic transcription: green
            element!("ph", {
                move |el| {
                    el.before(GREEN, ContentType::Html);
                    push_end_tag_handler!(el, |end| {
                        end.before(COLOR_RESET, ContentType::Html);
                        end.remove();
                        Ok(())
                    });
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            // <d> date: yellow, space after
            element!("d", {
                move |el| {
                    el.before(YELLOW, ContentType::Html);
                    push_end_tag_handler!(el, |end| {
                        end.before(&format!("{} ", COLOR_RESET), ContentType::Html);
                        end.remove();
                        Ok(())
                    });
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            // <ch> citation header (author): bold
            element!("ch", {
                move |el| {
                    el.before(BOLD_ON, ContentType::Html);
                    push_end_tag_handler!(el, |end| {
                        end.before(BOLD_OFF, ContentType::Html);
                        end.remove();
                        Ok(())
                    });
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            // <qt> quotation text: italic
            element!("qt", {
                move |el| {
                    el.before(ITALIC_ON, ContentType::Html);
                    push_end_tag_handler!(el, |end| {
                        end.before(ITALIC_OFF, ContentType::Html);
                        end.remove();
                        Ok(())
                    });
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            // <ls> sense label/number: bold
            element!("ls", {
                move |el| {
                    el.before(BOLD_ON, ContentType::Html);
                    push_end_tag_handler!(el, |end| {
                        end.before(BOLD_OFF, ContentType::Html);
                        end.remove();
                        Ok(())
                    });
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            // <w> abbreviation word: dim (simulates small-caps look)
            element!("w", {
                move |el| {
                    el.before(DIM, ContentType::Html);
                    push_end_tag_handler!(el, |end| {
                        end.before(DIM_OFF, ContentType::Html);
                        end.remove();
                        Ok(())
                    });
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            // <a> links: if href starts with entry://, show as cyan underline with target
            // Otherwise just keep content
            element!("a", {
                move |el| {
                    let href = el.get_attribute("href").unwrap_or_default();
                    if href.starts_with("entry://") {
                        let target = href["entry://".len()..].to_string();
                        el.before(&format!("{}{}", CYAN, UNDERLINE_ON), ContentType::Html);
                        push_end_tag_handler!(el, move |end| {
                            end.before(
                                &format!(
                                    "{}{} [→{}]{}{}",
                                    UNDERLINE_OFF, DIM, target, DIM_OFF, COLOR_RESET
                                ),
                                ContentType::Html,
                            );
                            end.remove();
                            Ok(())
                        });
                    } else if !href.is_empty() {
                        el.before(&format!("{}{}", CYAN, UNDERLINE_ON), ContentType::Html);
                        push_end_tag_handler!(el, |end| {
                            end.before(
                                &format!("{}{}", UNDERLINE_OFF, COLOR_RESET),
                                ContentType::Html,
                            );
                            end.remove();
                            Ok(())
                        });
                    }
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            // === Standard HTML formatting ===

            // <b>, <strong> → bold
            element!("b, strong", {
                move |el| {
                    el.before(BOLD_ON, ContentType::Html);
                    push_end_tag_handler!(el, |end| {
                        end.before(BOLD_OFF, ContentType::Html);
                        end.remove();
                        Ok(())
                    });
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            // <i>, <em> → italic
            element!("i, em", {
                move |el| {
                    el.before(ITALIC_ON, ContentType::Html);
                    push_end_tag_handler!(el, |end| {
                        end.before(ITALIC_OFF, ContentType::Html);
                        end.remove();
                        Ok(())
                    });
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            // <u> → underline
            element!("u", {
                move |el| {
                    el.before(UNDERLINE_ON, ContentType::Html);
                    push_end_tag_handler!(el, |end| {
                        end.before(UNDERLINE_OFF, ContentType::Html);
                        end.remove();
                        Ok(())
                    });
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            // <br> → newline (no indent — Webster uses &nbsp; for indentation)
            element!("br", {
                move |el| {
                    el.replace("\n", ContentType::Text);
                    Ok(())
                }
            }),
            // <p>, <div>, <tr> → newline after content
            element!("p, div, tr", {
                let indent = &indent_level;
                move |el| {
                    let indent2 = indent.clone();
                    push_end_tag_handler!(el, move |end| {
                        let level = *indent2.borrow();
                        end.before(&format!("\n{}", indent_str(level)), ContentType::Text);
                        end.remove();
                        Ok(())
                    });
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            // <td> → tab
            element!("td", {
                move |el| {
                    el.before("\t", ContentType::Text);
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            // <sup> → just keep content (no terminal superscript)
            element!("sup", {
                move |el| {
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            // <small> → keep content
            element!("small", {
                move |el| {
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            // <st> struck-through text in OED → dim
            element!("st", {
                move |el| {
                    el.before(DIM, ContentType::Html);
                    push_end_tag_handler!(el, |end| {
                        end.before(DIM_OFF, ContentType::Html);
                        end.remove();
                        Ok(())
                    });
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            // === Passthrough structural wrappers: remove tag, keep content ===
            // OED4: phon, gbl, gbr, n, c, cw, hg, idg, see, cnt
            // Webster: com (comment/metadata tag)
            element!("phon, gbl, gbr, n, c, cw, hg, idg, see, cnt, li, com", {
                move |el| {
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            // The OED4 root element (lol_html lowercases tag names per HTML5 spec)
            element!("oed4", {
                move |el| {
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
        ],
        ..Settings::default()
    };

    let mut rewriter = HtmlRewriter::new(settings, |chunk: &[u8]| {
        if let Ok(text) = std::str::from_utf8(chunk) {
            result.borrow_mut().push_str(text);
        }
    });

    if let Err(e) = rewriter.write(html.as_bytes()) {
        eprintln!("HTML render error: {}", e);
    }
    if let Err(e) = rewriter.end() {
        eprintln!("HTML render end error: {}", e);
    }

    let raw = result.into_inner();

    // Post-process:
    // - Replace &nbsp; with space (lol_html passes entities through as-is)
    // - Strip \r
    // - Strip leading tabs after newlines (from OED source indentation)
    // - Condense 3+ consecutive newlines into 2
    let raw = raw.replace("&nbsp;", " ");

    let mut cleaned = String::with_capacity(raw.len());
    let mut newline_count = 0u32;
    for ch in raw.chars() {
        if ch == '\n' {
            newline_count += 1;
            if newline_count <= 2 {
                cleaned.push(ch);
            }
        } else if ch == '\r' {
            // Skip \r (from \r\n in the raw data)
            continue;
        } else if ch == '\t' && newline_count > 0 {
            // Tab after newline is from the original OED HTML source indentation — skip it
            // (our element handlers apply proper indentation for OED;
            //  Webster uses &nbsp; which is already converted to spaces above)
            continue;
        } else {
            newline_count = 0;
            cleaned.push(ch);
        }
    }

    // Append reset at end to clear any lingering ANSI state
    cleaned.push_str(RESET_ALL);

    cleaned.trim().to_string()
}
