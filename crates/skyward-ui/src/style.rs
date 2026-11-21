//! What a page's stylesheet says, as far as showing the page needs it.
//!
//! This is not a CSS engine and does not try to be one: there is no cascade
//! beyond the last rule winning, no inheritance beyond what is copied down by
//! hand, no layout algorithm, and no rules inside `@media`. What it reads is
//! the small set of declarations that decide how a page's words stand —
//! whether a run of links is a row or a column, how large a heading is, what
//! colour it is — which is the difference between a page that reads like the
//! page and a page that reads like a transcript.

use crate::color::Rgba;

/// How an element is laid out, of the values that change how a page reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display {
    /// A block of its own, one after another.
    Block,
    /// Inline with the words around it.
    Inline,
    /// A line that holds others laid along it.
    Row,
    /// A column of them.
    Column,
    /// Not shown at all.
    None,
}

/// Where a run of words stands in the room it is given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Centre,
    Right,
}

/// The declarations this build acts on.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Style {
    pub display: Option<Display>,
    /// In pixels at the page's own size, `em` and `%` worked out against it.
    pub font_size: Option<f32>,
    pub color: Option<Rgba>,
    pub background: Option<Rgba>,
    pub align: Option<Align>,
    pub bold: Option<bool>,
    /// Space above and below, in pixels: margins and padding together, which
    /// a page uses to separate one thing from the next.
    pub space_before: Option<f32>,
    pub space_after: Option<f32>,
}

impl Style {
    /// This style laid over `under`: what it says wins, and what it does not
    /// speak of stays as it was.
    pub fn over(&self, under: &Style) -> Style {
        Style {
            display: self.display.or(under.display),
            font_size: self.font_size.or(under.font_size),
            color: self.color.or(under.color),
            background: self.background.or(under.background),
            align: self.align.or(under.align),
            bold: self.bold.or(under.bold),
            space_before: self.space_before.or(under.space_before),
            space_after: self.space_after.or(under.space_after),
        }
    }

    /// Whether it says anything at all.
    pub fn is_empty(&self) -> bool {
        *self == Style::default()
    }
}

/// What a rule reaches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selector {
    /// Every element of a tag.
    Tag(String),
    /// Every element with a class.
    Class(String),
    /// The element with an id.
    Id(String),
    /// Everything.
    All,
    /// Several of these at once, as `h1, h2, h3` writes them.
    Any(Vec<Selector>),
}

impl Selector {
    /// Whether this reaches an element that is `tag`, has these `classes`
    /// and is called `id`.
    ///
    /// Only the last part of a compound selector is looked at: `.a .b` is
    /// read as `.b`, which over-applies a descendant rule but never
    /// under-applies it, and a page that leans on scoping for its look is
    /// beyond what this reads anyway.
    pub fn reaches(&self, tag: &str, classes: &[&str], id: Option<&str>) -> bool {
        match self {
            Selector::All => true,
            Selector::Tag(name) => name == "*" || name.eq_ignore_ascii_case(tag),
            Selector::Class(name) => classes.iter().any(|class| class == name),
            Selector::Id(name) => id == Some(name.as_str()),
            Selector::Any(parts) => parts.iter().any(|part| part.reaches(tag, classes, id)),
        }
    }
}

/// One rule of a sheet.
#[derive(Debug, Clone, PartialEq)]
pub struct Rule {
    pub selector: Selector,
    pub style: Style,
}

/// A stylesheet, in the order its rules were written.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Sheet {
    pub rules: Vec<Rule>,
}

impl Sheet {
    /// What this sheet says about an element, the last rule that reaches it
    /// winning where two disagree.
    pub fn style_of(&self, tag: &str, classes: &[&str], id: Option<&str>) -> Style {
        let mut held = Style::default();
        for rule in &self.rules {
            if rule.selector.reaches(tag, classes, id) {
                held = rule.style.over(&held);
            }
        }
        held
    }

    /// Whether it holds any rule at all.
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

/// The stylesheet `css` describes, at most `MOST_RULES` of it.
pub fn parse(css: &str) -> Sheet {
    const MOST_RULES: usize = 4_000;
    let mut sheet = Sheet::default();
    let text = strip_comments(css);
    let bytes: Vec<char> = text.chars().collect();
    let mut at = 0usize;
    while at < bytes.len() && sheet.rules.len() < MOST_RULES {
        // A block's selector runs to its opening brace.
        let Some(open) = find(&bytes, at, '{') else { break };
        let selector_text: String = bytes[at..open].iter().collect();
        // At-rules are stepped over whole: their contents are rules for
        // other conditions, which are not this page's as it is being read.
        let Some(close) = matching_brace(&bytes, open) else { break };
        if !selector_text.trim_start().starts_with('@') {
            let body: String = bytes[open + 1..close].iter().collect();
            let style = declarations(&body);
            if !style.is_empty() {
                if let Some(selector) = selector(&selector_text) {
                    sheet.rules.push(Rule { selector, style });
                }
            }
        }
        at = close + 1;
    }
    sheet
}

/// Comments taken out, so that a brace inside one is not read as a brace.
fn strip_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let chars: Vec<char> = css.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
            i += 2;
            while i < chars.len() && !(chars[i] == '*' && chars.get(i + 1) == Some(&'/')) {
                i += 1;
            }
            i += 2;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn find(chars: &[char], from: usize, what: char) -> Option<usize> {
    (from..chars.len()).find(|&i| chars[i] == what)
}

fn matching_brace(chars: &[char], open: usize) -> Option<usize> {
    let mut depth = 0i32;
    for (i, &c) in chars.iter().enumerate().skip(open) {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// The selector a block's text names, if it names one this build reads.
fn selector(text: &str) -> Option<Selector> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let parts: Vec<Selector> = trimmed
        .split(',')
        .filter_map(|part| {
            // The last part of a compound, as `module` notes.
            let last = part.split_whitespace().next_back().unwrap_or(part).trim();
            let last = last.split('>').next_back().unwrap_or(last).trim();
            if last.is_empty() || last == "*" {
                return Some(Selector::All);
            }
            // Pseudo-classes and attribute tests are dropped: `a:hover` is
            // still an `a`.
            let clean: String = last.split([':', '[', '.']).next().unwrap_or(last).trim().to_string();
            if let Some(id) = last.strip_prefix('#') {
                let id: String = id.split([':', '[', '.']).next().unwrap_or(id).to_string();
                return (!id.is_empty()).then_some(Selector::Id(id));
            }
            if let Some(class) = last.split('.').nth(1) {
                let class: String = class.split([':', '[']).next().unwrap_or(class).to_string();
                if !class.is_empty() {
                    return Some(Selector::Class(class));
                }
            }
            if clean.is_empty() {
                return None;
            }
            Some(Selector::Tag(clean.to_ascii_lowercase()))
        })
        .collect();
    match parts.len() {
        0 => None,
        1 => parts.into_iter().next(),
        _ => Some(Selector::Any(parts)),
    }
}

/// The declarations of one rule's body.
fn declarations(body: &str) -> Style {
    let mut style = Style::default();
    for part in body.split(';') {
        let Some((name, value)) = part.split_once(':') else { continue };
        let (name, value) = (name.trim().to_ascii_lowercase(), value.trim());
        // `!important` and vendor trails do not change what is meant here.
        let value = value.split("!important").next().unwrap_or(value).trim();
        match name.as_str() {
            "display" => {
                style.display = Some(match value.to_ascii_lowercase().as_str() {
                    "inline" | "inline-block" | "inline-flex" => Display::Inline,
                    "flex" | "inline-grid" | "grid" => Display::Row,
                    "none" => Display::None,
                    _ => Display::Block,
                });
            }
            "font-size" => style.font_size = length(value),
            "color" => style.color = colour(value),
            "background-color" | "background" => style.background = colour(value),
            "text-align" => {
                style.align = Some(match value.to_ascii_lowercase().as_str() {
                    "center" | "centre" => Align::Centre,
                    "right" | "end" => Align::Right,
                    _ => Align::Left,
                })
            }
            "font-weight" => {
                style.bold = Some(match value.to_ascii_lowercase().as_str() {
                    "bold" | "bolder" | "600" | "700" | "800" | "900" => true,
                    "normal" | "lighter" | "100" | "200" | "300" | "400" | "500" => false,
                    // A weight this build does not know: the declaration is
                    // skipped, not the whole rule.
                    _ => continue,
                })
            }
            "margin" | "padding" | "margin-top" | "margin-bottom" | "padding-top" | "padding-bottom" => {
                if let Some(px) = length(value) {
                    if name.ends_with("top") || name == "margin" || name == "padding" {
                        style.space_before = Some(px);
                    }
                    if name.ends_with("bottom") || name == "margin" || name == "padding" {
                        style.space_after = Some(px);
                    }
                }
            }
            _ => {}
        }
    }
    style
}

/// A length in pixels. `em` and `%` are worked out against the page's own
/// size, which is what a recovery shows text at when nothing says otherwise.
pub const BASE_PX: f32 = 16.0;

fn length(value: &str) -> Option<f32> {
    let value = value.trim();
    if let Some(px) = value.strip_suffix("px") {
        return px.trim().parse::<f32>().ok();
    }
    if let Some(em) = value.strip_suffix("rem").or_else(|| value.strip_suffix("em")) {
        return em.trim().parse::<f32>().ok().map(|n| n * BASE_PX);
    }
    if let Some(pct) = value.strip_suffix('%') {
        return pct.trim().parse::<f32>().ok().map(|n| n / 100.0 * BASE_PX);
    }
    // A bare number is not a length.
    None
}

/// A colour, as `#rgb`, `#rrggbb`, `rgb(...)` or one of the names a page uses
/// for its text.
fn colour(value: &str) -> Option<Rgba> {
    let value = value.trim();
    if let Some(hex) = value.strip_prefix('#') {
        let digits: Vec<u8> = hex.chars().filter_map(|c| c.to_digit(16).map(|d| d as u8)).collect();
        return match digits.len() {
            3 => Some(Rgba::rgb(digits[0] * 17, digits[1] * 17, digits[2] * 17)),
            6 => Some(Rgba::rgb(digits[0] * 16 + digits[1], digits[2] * 16 + digits[3], digits[4] * 16 + digits[5])),
            _ => None,
        };
    }
    if let Some(inner) = value.strip_prefix("rgb(").or_else(|| value.strip_prefix("rgba(")) {
        let inner = inner.trim_end_matches(')');
        let parts: Vec<f32> = inner
            .split(',')
            .take(3)
            .filter_map(|n| n.trim().trim_end_matches('%').parse::<f32>().ok())
            .collect();
        if parts.len() == 3 {
            let a = |v: f32| v.clamp(0.0, 255.0) as u8;
            return Some(Rgba::rgb(a(parts[0]), a(parts[1]), a(parts[2])));
        }
        return None;
    }
    // The names that carry a page's meaning: text and its background.
    let lower = value.to_ascii_lowercase();
    if lower == "transparent" {
        return None;
    }
    let named = match lower.as_str() {
        "black" => (0, 0, 0),
        "white" => (255, 255, 255),
        "red" => (255, 0, 0),
        "green" => (0, 128, 0),
        "blue" => (0, 0, 255),
        "gray" | "grey" => (128, 128, 128),
        "silver" => (192, 192, 192),
        "orange" => (255, 165, 0),
        "yellow" => (255, 255, 0),
        "purple" => (128, 0, 128),
        "navy" => (0, 0, 128),
        "teal" => (0, 128, 128),
        "maroon" => (128, 0, 0),
        "olive" => (128, 128, 0),
        "lime" => (0, 255, 0),
        "aqua" | "cyan" => (0, 255, 255),
        "fuchsia" | "magenta" => (255, 0, 255),
        _ => return None,
    };
    Some(Rgba::rgb(named.0, named.1, named.2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sheet_is_read_into_rules() {
        let sheet = parse("h1 { font-size: 32px; color: #333 } .nav { display: flex } #top { display: none }");
        assert_eq!(sheet.rules.len(), 3);
        assert_eq!(sheet.style_of("h1", &[], None).font_size, Some(32.0));
        assert_eq!(sheet.style_of("div", &["nav"], None).display, Some(Display::Row));
        assert_eq!(sheet.style_of("div", &[], Some("top")).display, Some(Display::None));
    }

    #[test]
    fn the_last_rule_that_reaches_an_element_wins() {
        let sheet = parse("a { color: red } a { color: blue }");
        assert_eq!(sheet.style_of("a", &[], None).color, Some(Rgba::rgb(0, 0, 255)));
    }

    #[test]
    fn what_this_build_does_not_read_is_stepped_over() {
        // At-rules whole, comments including their braces, pseudo-classes
        // and attribute tests dropped to what they name.
        let sheet = parse(
            "@media (max-width: 600px) { body { display: none } }
             /* a comment with { a brace } in it */
             a:hover, a.x[id] { color: #fff }
             .row { display: flex; gap: 10px; unknown-prop: 1 }",
        );
        let a = sheet.style_of("a", &[], None);
        assert_eq!(a.color, Some(Rgba::rgb(255, 255, 255)), "the media rule is not this page's");
        assert_eq!(a.display, None);
        assert_eq!(sheet.style_of("div", &["row"], None).display, Some(Display::Row));
    }

    #[test]
    fn a_length_is_read_in_the_units_a_page_writes() {
        assert_eq!(length("16px"), Some(16.0));
        assert_eq!(length("2em"), Some(32.0));
        assert_eq!(length("1.5rem"), Some(24.0));
        assert_eq!(length("150%"), Some(24.0));
        assert_eq!(length("bold"), None, "a bare word is not a length");
    }

    #[test]
    fn a_colour_is_read_as_it_is_written() {
        assert_eq!(colour("#f00"), Some(Rgba::rgb(255, 0, 0)));
        assert_eq!(colour("#00ff00"), Some(Rgba::rgb(0, 255, 0)));
        assert_eq!(colour("rgb(1, 2, 3)"), Some(Rgba::rgb(1, 2, 3)));
        assert_eq!(colour("white"), Some(Rgba::rgb(255, 255, 255)));
        assert_eq!(colour("transparent"), None);
        assert_eq!(colour("nonsense"), None);
    }

    #[test]
    fn a_broken_stylesheet_is_read_as_far_as_it_goes() {
        for broken in [
            "",
            "h1 {",
            "}{",
            "h1 { color: ",
            "@media { }",
            "/* unterminated",
            "a { display: flex",
            "p { color: #zzz }",
        ] {
            let sheet = parse(broken);
            // Nothing about reading a sheet may end the recovery.
            let _ = sheet.style_of("p", &["x"], Some("y"));
        }
    }

    #[test]
    fn a_rule_that_says_nothing_useful_is_not_kept() {
        let sheet = parse("p { unknown: 1; gap: 2px } div { color: red }");
        assert_eq!(sheet.rules.len(), 1, "{:?}", sheet.rules);
    }
}
