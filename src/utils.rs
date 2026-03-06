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

// ANSI escape codes
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

/// Renders OED4/MDX HTML to terminal with ANSI colors, bold, italic, and proper indentation.
///
/// Uses lol_html for streaming HTML parsing — no regex, no unescape/re-escape roundtrips.
///
/// Handles OED4-specific custom tags:
/// - `<se0>` headword entry line → newline before
/// - `<se4>` definition/sense → indentation + newline before
/// - `<se8>` quotation block → deeper indentation + newline before
/// - `<d>` date → yellow + space after
/// - `<ch>` citation header (author) → bold
/// - `<qt>` quotation text → italic
/// - `<ph>` phonetic → green
/// - `<hw>` headword → bold + underline
/// - `<ls>` sense label → bold
/// - `<w>` abbreviation → dim (simulating small-caps)
/// - `<a href="entry://...">` cross-reference link → cyan + underline
/// - `<spg>` spelling group → newline before, dim
/// - `<dg>` definition/etymology group → newline before
/// - `<script>`, `<link>`, `<style>` → stripped entirely
/// - `<b>`, `<strong>` → bold
/// - `<i>`, `<em>` → italic
/// - `<u>` → underline
/// - `<br>` → newline
/// - `<seg>` → separator line
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

            // <se4> definition/sense block: newline + indent
            element!("se4", {
                let indent = &indent_level;
                move |el| {
                    *indent.borrow_mut() = 4;
                    el.before(&format!("\n{}", SE4_INDENT), ContentType::Text);
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),

            // <se8> quotation block: newline + deeper indent
            element!("se8", {
                let indent = &indent_level;
                move |el| {
                    *indent.borrow_mut() = 8;
                    el.before(&format!("\n{}", SE8_INDENT), ContentType::Text);
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),

            // <q> individual quotation: newline + current indent
            element!("q", {
                let indent = &indent_level;
                move |el| {
                    let level = *indent.borrow();
                    el.before(&format!("\n{}", indent_str(level)), ContentType::Text);
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

            // <spg> spelling/forms group: newline, dim text
            element!("spg", {
                let indent = &indent_level;
                move |el| {
                    let level = *indent.borrow();
                    el.before(&format!("\n{}{}", indent_str(level), DIM), ContentType::Html);
                    push_end_tag_handler!(el, |end| {
                        end.before(DIM_OFF, ContentType::Html);
                        end.remove();
                        Ok(())
                    });
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),

            // <dg> definition/etymology group: newline before
            element!("dg", {
                let indent = &indent_level;
                move |el| {
                    let level = *indent.borrow();
                    el.before(&format!("\n{}", indent_str(level)), ContentType::Text);
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
                                &format!("{}{} [→{}]{}{}", UNDERLINE_OFF, DIM, target, DIM_OFF, COLOR_RESET),
                                ContentType::Html,
                            );
                            end.remove();
                            Ok(())
                        });
                    } else if !href.is_empty() {
                        el.before(&format!("{}{}", CYAN, UNDERLINE_ON), ContentType::Html);
                        push_end_tag_handler!(el, |end| {
                            end.before(&format!("{}{}", UNDERLINE_OFF, COLOR_RESET), ContentType::Html);
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

            // <br> → newline + current indent
            element!("br", {
                let indent = &indent_level;
                move |el| {
                    let level = *indent.borrow();
                    el.replace(&format!("\n{}", indent_str(level)), ContentType::Text);
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
            // OED4 wrappers: phon, gbl, gbr, n, c, cw, hg, idg, see, cnt
            element!("phon, gbl, gbr, n, c, cw, hg, idg, see, cnt, li", {
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

    // Post-process: condense 3+ consecutive newlines into 2, strip \r and leading tabs
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
            // Tab after newline is from the original HTML source indentation — skip it
            // (our element handlers apply proper indentation)
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
