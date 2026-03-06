use std::cell::RefCell;

use lol_html::html_content::ContentType;
use lol_html::{comments, doc_comments, element, text, EndTagHandler, HtmlRewriter, Settings};

pub fn take_chars(s: &str, n: usize) -> &str {
    let byte_end = s
        .char_indices()
        .nth(n)
        .map(|(idx, _)| idx)
        .unwrap_or_else(|| s.len());
    &s[..byte_end]
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

// Assumed dark terminal background for contrast calculation (roughly #1e1e1e)
const BG_LUMINANCE: f64 = 0.031;
// Minimum contrast ratio for readable text on dark background (WCAG AA for normal text = 4.5)
const MIN_CONTRAST_RATIO: f64 = 4.5;

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

/// Compute relative luminance of an sRGB color per WCAG 2.1.
/// Input: (r, g, b) as u8 values 0–255.
/// Output: luminance in range [0.0, 1.0].
fn relative_luminance(r: u8, g: u8, b: u8) -> f64 {
    // Convert sRGB to linear
    fn linearize(c: u8) -> f64 {
        let s = c as f64 / 255.0;
        if s <= 0.04045 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * linearize(r) + 0.7152 * linearize(g) + 0.0722 * linearize(b)
}

/// Compute WCAG contrast ratio between two luminances.
/// Returns value >= 1.0 (1:1 means same color, 21:1 is max).
fn contrast_ratio(l1: f64, l2: f64) -> f64 {
    let (lighter, darker) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
    (lighter + 0.05) / (darker + 0.05)
}

/// Lighten an RGB color until it reaches the minimum contrast ratio against the dark background.
/// Returns the (possibly lightened) (r, g, b).
fn ensure_contrast(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let lum = relative_luminance(r, g, b);
    let ratio = contrast_ratio(lum, BG_LUMINANCE);
    if ratio >= MIN_CONTRAST_RATIO {
        return (r, g, b);
    }

    // Lighten by blending toward white until contrast is sufficient.
    // Binary search for the minimum blend factor.
    let mut lo: f64 = 0.0;
    let mut hi: f64 = 1.0;
    for _ in 0..20 {
        let mid = (lo + hi) / 2.0;
        let nr = r as f64 + (255.0 - r as f64) * mid;
        let ng = g as f64 + (255.0 - g as f64) * mid;
        let nb = b as f64 + (255.0 - b as f64) * mid;
        let l = relative_luminance(nr as u8, ng as u8, nb as u8);
        if contrast_ratio(l, BG_LUMINANCE) >= MIN_CONTRAST_RATIO {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    let factor = hi;
    let nr = (r as f64 + (255.0 - r as f64) * factor).min(255.0) as u8;
    let ng = (g as f64 + (255.0 - g as f64) * factor).min(255.0) as u8;
    let nb = (b as f64 + (255.0 - b as f64) * factor).min(255.0) as u8;
    (nr, ng, nb)
}

/// Parse a CSS/HTML color value and return an ANSI escape sequence.
/// Supports:
///   - 6-digit hex: "CA0000", "#CA0000"
///   - 3-digit hex: "#F00"
///   - CSS named colors (common subset)
/// Returns 24-bit truecolor ANSI for kitty/modern terminals.
/// Colors are checked for contrast against dark terminal background and
/// lightened if needed (WCAG AA 4.5:1 ratio).
fn color_to_ansi(color: &str) -> Option<String> {
    let color = color.trim().trim_matches('"').trim_matches('\'');
    if color.is_empty() {
        return None;
    }

    // Parse to (r, g, b) first, then contrast-check, then emit ANSI
    let (r, g, b) = parse_color_to_rgb(color)?;
    let (r, g, b) = ensure_contrast(r, g, b);
    Some(format!("\x1b[38;2;{};{};{}m", r, g, b))
}

/// Parse a CSS color name or hex value to (r, g, b).
fn parse_color_to_rgb(color: &str) -> Option<(u8, u8, u8)> {
    let lower = color.to_ascii_lowercase();
    // Named colors
    let rgb = match lower.as_str() {
        "black" => (0, 0, 0),
        "red" => (255, 0, 0),
        "green" | "lime" => (0, 128, 0),
        "yellow" => (255, 255, 0),
        "blue" => (0, 0, 255),
        "magenta" | "fuchsia" => (255, 0, 255),
        "cyan" | "aqua" => (0, 255, 255),
        "white" => (255, 255, 255),
        "gray" | "grey" => (128, 128, 128),
        "lightgray" | "lightgrey" | "silver" => (192, 192, 192),
        "darkred" | "maroon" => (128, 0, 0),
        "darkgreen" => (0, 100, 0),
        "darkblue" | "navy" => (0, 0, 128),
        "darkcyan" | "teal" => (0, 128, 128),
        "darkmagenta" | "purple" => (128, 0, 128),
        "darkorange" => (255, 140, 0),
        "darkslategray" | "darkslategrey" => (47, 79, 79),
        "slategray" | "slategrey" => (112, 128, 144),
        "dimgray" | "dimgrey" => (105, 105, 105),
        "olive" => (128, 128, 0),
        "olivedrab" => (107, 142, 35),
        "brown" | "saddlebrown" => (139, 69, 19),
        "sienna" => (160, 82, 45),
        "chocolate" => (210, 105, 30),
        "firebrick" => (178, 34, 34),
        "crimson" => (220, 20, 60),
        "indianred" => (205, 92, 92),
        "tomato" => (255, 99, 71),
        "orangered" => (255, 69, 0),
        "coral" => (255, 127, 80),
        "salmon" => (250, 128, 114),
        "gold" => (255, 215, 0),
        "khaki" => (240, 230, 140),
        "limegreen" => (50, 205, 50),
        "forestgreen" => (34, 139, 34),
        "seagreen" => (46, 139, 87),
        "steelblue" => (70, 130, 180),
        "royalblue" => (65, 105, 225),
        "dodgerblue" => (30, 144, 255),
        "cornflowerblue" => (100, 149, 237),
        "cadetblue" => (95, 158, 160),
        "deepskyblue" => (0, 191, 255),
        "mediumblue" => (0, 0, 205),
        "midnightblue" => (25, 25, 112),
        "blueviolet" => (138, 43, 226),
        "darkviolet" => (148, 0, 211),
        "darkorchid" => (153, 50, 204),
        "mediumorchid" => (186, 85, 211),
        "orchid" => (218, 112, 214),
        "violet" => (238, 130, 238),
        "plum" => (221, 160, 221),
        "hotpink" => (255, 105, 180),
        "deeppink" => (255, 20, 147),
        "pink" => (255, 192, 203),
        "rosybrown" => (188, 143, 143),
        "tan" => (210, 180, 140),
        "peru" => (205, 133, 63),
        "burlywood" => (222, 184, 135),
        "wheat" => (245, 222, 179),
        _ => {
            // Try hex
            let hex = lower.strip_prefix('#').unwrap_or(&lower);
            return match hex.len() {
                6 => {
                    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                    Some((r, g, b))
                }
                3 => {
                    let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
                    let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
                    let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
                    Some((r, g, b))
                }
                _ => None,
            };
        }
    };
    Some(rgb)
}

/// Extract a CSS property value from an inline style string.
/// e.g. extract_style_property("font-size:90%;color:#111111;", "color") → Some("#111111")
fn extract_style_property<'a>(style: &'a str, property: &str) -> Option<&'a str> {
    for part in style.split(';') {
        let part = part.trim();
        if let Some((_key, value)) = part.split_once(':') {
            let key = _key.trim();
            if key.eq_ignore_ascii_case(property) {
                return Some(value.trim());
            }
        }
    }
    None
}

/// Renders MDX dictionary HTML to terminal with ANSI colors, bold, italic.
///
/// Works with any dictionary format — handles both standard HTML tags and
/// dictionary-specific custom tags (OED4, Webster, Collins, etc.) without configuration.
pub fn render_html_to_terminal(html: &str) -> String {
    let result = RefCell::new(String::with_capacity(html.len()));
    let indent_level: RefCell<u8> = RefCell::new(0);

    fn indent_str(level: u8) -> &'static str {
        match level {
            8 => SE8_INDENT,
            4 => SE4_INDENT,
            _ => "",
        }
    }

    let settings = Settings {
        element_content_handlers: vec![
            // === Strip HTML comments inside any element ===
            comments!("*", {
                move |c| {
                    c.remove();
                    Ok(())
                }
            }),
            // === Headers: h1, h2, h3 ===
            element!("h1, h2, h3", {
                move |el| {
                    el.before(BOLD_ON, ContentType::Html);
                    push_end_tag_handler!(el, |end| {
                        end.before(BOLD_OFF, ContentType::Html);
                        end.before("\n", ContentType::Text);
                        end.remove();
                        Ok(())
                    });
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            // === Dictionary Sense: <sense> ===
            element!("sense", {
                move |el| {
                    el.before(&format!("{}{}", BOLD_ON, YELLOW), ContentType::Html);
                    push_end_tag_handler!(el, |end| {
                        end.before(&format!("{}{}", COLOR_RESET, BOLD_OFF), ContentType::Html);
                        end.remove();
                        Ok(())
                    });
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            // === Word Detail Class: .wordDetail ===
            element!(".wordDetail", {
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
            // === Strip <link>, <img>, <meta> (void elements, no content) ===
            element!("link", {
                move |el| {
                    el.remove();
                    Ok(())
                }
            }),
            element!("img", {
                move |el| {
                    el.remove();
                    Ok(())
                }
            }),
            element!("meta", {
                move |el| {
                    el.remove();
                    Ok(())
                }
            }),
            // === <hr> → horizontal rule ===
            element!("hr", {
                move |el| {
                    el.replace(&format!("\n{}", "─".repeat(40)), ContentType::Text);
                    Ok(())
                }
            }),
            // === <font> tag: handle color and size attributes ===
            element!("font", {
                move |el| {
                    let mut did_color = false;

                    if let Some(color_val) = el.get_attribute("color") {
                        if let Some(ansi) = color_to_ansi(&color_val) {
                            el.before(&ansi, ContentType::Html);
                            did_color = true;
                        }
                    }

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
            // === <span> tag: parse inline style for color ===
            element!("span", {
                move |el| {
                    let mut did_color = false;

                    if let Some(style) = el.get_attribute("style") {
                        if let Some(color_val) = extract_style_property(&style, "color") {
                            if let Some(ansi) = color_to_ansi(color_val) {
                                el.before(&ansi, ContentType::Html);
                                did_color = true;
                            }
                        }
                    }

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
            element!("se0", {
                let indent = &indent_level;
                move |el| {
                    *indent.borrow_mut() = 0;
                    el.before("\n\n", ContentType::Text);
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            element!("se4", {
                let indent = &indent_level;
                move |el| {
                    *indent.borrow_mut() = 4;
                    el.before(&format!("\n{}", SE4_INDENT), ContentType::Text);
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            element!("se8", {
                let indent = &indent_level;
                move |el| {
                    *indent.borrow_mut() = 8;
                    el.before(&format!("\n{}", SE8_INDENT), ContentType::Text);
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            element!("q", {
                let indent = &indent_level;
                move |el| {
                    let level = *indent.borrow();
                    el.before(&format!("\n{}", indent_str(level)), ContentType::Text);
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            element!("seg", {
                move |el| {
                    el.replace("\n───\n", ContentType::Text);
                    Ok(())
                }
            }),
            element!("spg", {
                move |el| {
                    el.before(&format!("\n{}", DIM), ContentType::Html);
                    push_end_tag_handler!(el, |end| {
                        end.before(DIM_OFF, ContentType::Html);
                        end.remove();
                        Ok(())
                    });
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            element!("dg", {
                move |el| {
                    el.before("\n", ContentType::Text);
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            // === OED4 inline formatting ===

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
            // <ph> phonetic: green
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
            // <ch> citation author: bold
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
            // <ls> sense label: bold
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
            // <w> abbreviation: dim
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
            // === Collins dictionary: <def> definition → content kept, <posp> part of speech → italic ===
            element!("def", {
                move |el| {
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            element!("posp", {
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
            // === Collins: strip entry-index (navigation links, not content) ===
            element!("entry-index", {
                move |el| {
                    el.remove();
                    Ok(())
                }
            }),
            text!("entry-index", {
                move |t| {
                    t.remove();
                    Ok(())
                }
            }),
            // === <a> links ===
            element!("a", {
                move |el| {
                    let href = el.get_attribute("href").unwrap_or_default();
                    if href.starts_with("entry://") {
                        let target = href["entry://".len()..].to_string();
                        el.before(&format!("{}{}", CYAN, UNDERLINE_ON), ContentType::Html);
                        // Only show [→target] for actual dictionary entry links,
                        // not for meta/guide links (which contain '#' fragments)
                        let show_target = !target.contains('#');
                        push_end_tag_handler!(el, move |end| {
                            if show_target {
                                end.before(
                                    &format!(
                                        "{}{} [→{}]{}{}",
                                        UNDERLINE_OFF, DIM, target, DIM_OFF, COLOR_RESET
                                    ),
                                    ContentType::Html,
                                );
                            } else {
                                end.before(
                                    &format!("{}{}", UNDERLINE_OFF, COLOR_RESET),
                                    ContentType::Html,
                                );
                            }
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
            element!("br", {
                move |el| {
                    el.replace("\n", ContentType::Text);
                    Ok(())
                }
            }),
            element!("p, div, tr, section", {
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
            element!("td", {
                move |el| {
                    el.before("\t", ContentType::Text);
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            element!("sup", {
                move |el| {
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
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
            // Webster: com
            // Translation: trn
            // Lists: ul, ol, li
            // Collins: superentry, entry, hwblk, hwgrp, hwunit, datablk,
            //          gramcat, pospgrp, pospunit, sensecat, defgrp, defunit
            element!("phon, gbl, gbr, n, c, cw, hg, idg, see, cnt, com, trn", {
                move |el| {
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            element!("ul, ol, li", {
                move |el| {
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            // Collins dictionary structural tags — with spacing
            element!("superentry, entry, hwgrp, hwunit, datablk", {
                move |el| {
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            // <hwblk> headword block: add newline after to separate from definition
            element!("hwblk", {
                move |el| {
                    push_end_tag_handler!(el, |end| {
                        end.before("\n", ContentType::Text);
                        end.remove();
                        Ok(())
                    });
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            // <gramcat> grammar category: add space before for separation
            element!("gramcat", {
                move |el| {
                    el.before(" ", ContentType::Text);
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            // <sensecat> sense: add newline before for new definition line
            element!("sensecat", {
                move |el| {
                    el.before("\n", ContentType::Text);
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            element!("pospgrp, pospunit, defgrp, defunit", {
                move |el| {
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
            // The OED4 root element
            element!("oed4", {
                move |el| {
                    el.remove_and_keep_content();
                    Ok(())
                }
            }),
        ],
        // === Strip HTML comments at document level (not inside any element) ===
        document_content_handlers: vec![doc_comments!(|c| {
            c.remove();
            Ok(())
        })],
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
    // 1. Replace &nbsp; with space (lol_html passes entities through as-is)
    // 2. Strip OED source formatting: \n followed by \t (inter-tag whitespace)
    // 3. Condense 3+ consecutive newlines into 2.
    let raw = raw.replace("&nbsp;", " ");
    let chars: Vec<char> = raw.chars().collect();
    let len = chars.len();

    let mut cleaned = String::with_capacity(raw.len());
    let mut newline_count = 0u32;
    let mut i = 0;
    while i < len {
        let ch = chars[i];
        match ch {
            '\r' => {
                i += 1;
            }
            '\n' => {
                // Check if this \n is followed by \t (OED source inter-tag whitespace)
                let mut j = i + 1;
                while j < len && chars[j] == '\t' {
                    j += 1;
                }
                if j > i + 1 {
                    // \n followed by tabs → OED source formatting, skip entirely
                    i = j;
                } else {
                    newline_count += 1;
                    if newline_count <= 2 {
                        cleaned.push('\n');
                    }
                    i += 1;
                }
            }
            _ => {
                newline_count = 0;
                cleaned.push(ch);
                i += 1;
            }
        }
    }

    cleaned.push_str(RESET_ALL);
    cleaned.trim().to_string()
}
