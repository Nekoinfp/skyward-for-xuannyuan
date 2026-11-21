//! One page off the web, as a server answers it.
//!
//! What this may do is small on purpose: one GET, no scripts, no stylesheets,
//! no images. What comes back is the text a reader can be shown, and where it
//! came from — the rest belongs to a browser with an engine in it.

use anyhow::{Context, Result};
use ureq::ResponseExt;
use std::io::Read;

/// How much of a page is taken. Past this the rest is left: a recovery has
/// the phone's memory to itself and nothing to page out to, and a page of
/// markup grows to several times its size as text once it is read.
pub const MOST: u64 = 512 << 10;

/// What a page is, as far as showing it goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Content {
    /// Markup to be read into blocks.
    Html,
    /// Plain text, shown as it stands.
    Text,
    /// Something else, which is named but not shown.
    Other(String),
}

impl Content {
    /// What a `Content-Type` header says this is.
    pub fn of(header: Option<&str>) -> Content {
        let Some(text) = header else { return Content::Other(String::new()) };
        // `text/html; charset=utf-8` is markup; the parameter is not the type.
        let kind = text.split(';').next().unwrap_or(text).trim().to_ascii_lowercase();
        match kind.as_str() {
            "text/html" | "application/xhtml+xml" => Content::Html,
            "text/plain" => Content::Text,
            "" => Content::Html,
            other => Content::Other(other.to_string()),
        }
    }
}

/// A page as it came back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    /// Where it was finally read from, once the server stopped redirecting.
    pub url: String,
    /// The status the server gave it.
    pub status: u16,
    pub content: Content,
    /// What the server sent, as text.
    pub body: String,
    /// Whether there was more than was taken.
    pub clipped: bool,
}

impl Page {
    /// Whether a reader is shown this one, as opposed to being told of it.
    pub fn shown(&self) -> bool {
        matches!(self.content, Content::Html | Content::Text)
    }
}

/// Read `url`, following the server's redirects, and give what it answers.
pub fn get(agent: &ureq::Agent, url: &str) -> Result<Page> {
    let mut response = agent.get(url).call().with_context(|| format!("reach {url}"))?;
    let status = response.status().as_u16();
    let settled = response.get_uri().to_string();
    let header = response.headers().get("content-type").and_then(|v| v.to_str().ok());
    let content = Content::of(header);
    let mut held = Vec::new();
    let mut clipped = false;
    {
        // One byte past what is kept, so that a page exactly of that size is
        // not called clipped.
        let mut body = response.body_mut().with_config().limit(MOST + 1).reader();
        let mut buf = vec![0u8; 64 << 10];
        loop {
            match body.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    held.extend_from_slice(&buf[..n]);
                    if held.len() as u64 > MOST {
                        clipped = true;
                        held.truncate(MOST as usize);
                        break;
                    }
                }
                // A body that stops early still has what came before it.
                Err(e) if !held.is_empty() => {
                    let _ = e;
                    break;
                }
                Err(e) => return Err(e).with_context(|| format!("read {url}")),
            }
        }
    }
    Ok(Page { url: settled, status, content, body: text_of(held), clipped })
}

/// Bytes as text. Almost everything a page is written in is UTF-8; what is
/// not is shown with its bad bytes marked rather than dropped.
fn text_of(bytes: Vec<u8>) -> String {
    match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(e) => String::from_utf8_lossy(e.as_bytes()).into_owned(),
    }
}

/// How large a picture may be before it is left out: a page with one huge
/// photograph on it must not cost the recovery its memory.
pub const PICTURE_MOST: u64 = 2 << 20;

/// The bytes at `url`, if there are no more of them than `most`.
pub fn bytes(agent: &ureq::Agent, url: &str, most: u64) -> Result<Vec<u8>> {
    let mut response = agent.get(url).call().with_context(|| format!("reach {url}"))?;
    if response.status().as_u16() != 200 {
        anyhow::bail!("{url}: status {}", response.status().as_u16());
    }
    let mut held = Vec::new();
    let mut body = response.body_mut().with_config().limit(most + 1).reader();
    let mut buf = vec![0u8; 32 << 10];
    loop {
        match body.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                held.extend_from_slice(&buf[..n]);
                if held.len() as u64 > most {
                    anyhow::bail!("{url}: larger than {most} bytes");
                }
            }
            Err(e) if !held.is_empty() => {
                let _ = e;
                break;
            }
            Err(e) => return Err(e).with_context(|| format!("read {url}")),
        }
    }
    Ok(held)
}
/// A page as it stands with nothing read yet, for a reader to open on.
pub fn blank() -> Page {
    Page { url: String::new(), status: 0, content: Content::Html, body: String::new(), clipped: false }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_a_server_calls_a_page_decides_how_it_is_shown() {
        assert_eq!(Content::of(Some("text/html")), Content::Html);
        assert_eq!(Content::of(Some("text/html; charset=utf-8")), Content::Html);
        assert_eq!(Content::of(Some("TEXT/HTML")), Content::Html, "case is not the server's meaning");
        assert_eq!(Content::of(Some("application/xhtml+xml")), Content::Html);
        assert_eq!(Content::of(Some("text/plain; charset=gbk")), Content::Text);
        assert_eq!(Content::of(Some("image/png")), Content::Other("image/png".into()));
        assert_eq!(Content::of(None), Content::Other(String::new()), "a server that says nothing said nothing");
        assert_eq!(Content::of(Some("")), Content::Html, "a server that named nothing sent a page");
    }

    #[test]
    fn a_page_that_cannot_be_read_as_text_is_still_shown() {
        let text = text_of(vec![0x68, 0x69, 0xff, 0xfe, 0x21]);
        assert!(text.starts_with("hi"), "{text}");
        assert!(text.ends_with('!'), "{text}");
        assert!(text.contains('\u{fffd}') || text.len() > 3, "the bad bytes are marked, not dropped: {text}");
    }

    #[test]
    fn only_markup_and_plain_text_are_shown() {
        let page = |content| Page { url: "x".into(), status: 200, content, body: String::new(), clipped: false };
        assert!(page(Content::Html).shown());
        assert!(page(Content::Text).shown());
        assert!(!page(Content::Other("image/png".into())).shown());
    }
}
