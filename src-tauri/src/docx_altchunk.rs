//! Some .docx files store the body as `w:altChunk` (MHTML/HTML), not Word XML.
//! downmark only reads `word/document.xml` text, so those convert to empty MD.

use std::fs::File;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

pub fn unwrap_docx_for_convert(path: &Path) -> Result<Option<String>, String> {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase());
    if ext.as_deref() != Some("docx") {
        return Ok(None);
    }
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mut zip = match ZipArchive::new(file) {
        Ok(z) => z,
        Err(_) => return Ok(None),
    };
    let xml = match read_zip(&mut zip, "word/document.xml") {
        Some(b) => String::from_utf8_lossy(&b).into_owned(),
        None => return Ok(None),
    };
    let ids = altchunk_rids(&xml);
    if ids.is_empty() {
        return Ok(None);
    }
    let rels = read_zip(&mut zip, "word/_rels/document.xml.rels")
        .ok_or_else(|| "docx altChunk has no document.xml.rels".to_string())?;
    let rels = String::from_utf8_lossy(&rels);
    let mut htmls = Vec::new();
    for id in ids {
        let target = rel_target(&rels, &id)
            .ok_or_else(|| format!("docx altChunk r:id={id} has no Relationship"))?;
        let part = package_part("word/_rels/document.xml.rels", &target);
        let bytes = read_zip(&mut zip, &part)
            .ok_or_else(|| format!("docx altChunk part missing: {part}"))?;
        let lower = part.to_ascii_lowercase();
        let html = if lower.ends_with(".mht") || lower.ends_with(".mhtml") {
            html_from_mht(&bytes).ok_or_else(|| format!("could not parse MHTML in {part}"))?
        } else {
            decode_html_bytes(&bytes)
        };
        if !html.trim().is_empty() {
            htmls.push(html);
        }
    }
    if htmls.is_empty() {
        return Err("docx altChunk contained no HTML".into());
    }
    Ok(Some(sanitize_html_for_downmark(&htmls.join("\n"))))
}

/// downmark drops HTML tables when cells use `<p>` / custom `<field>` tags.
pub fn sanitize_html_for_downmark(html: &str) -> String {
    let mut s = strip_tag_block(html, "style");
    s = strip_tag_block(&s, "script");
    for tag in ["field", "label", "button", "p"] {
        s = rename_html_tag(&s, tag, "span");
    }
    s
}

fn strip_tag_block(html: &str, tag: &str) -> String {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let open_l = open.to_ascii_lowercase();
    let close_l = close.to_ascii_lowercase();
    let lower = html.to_ascii_lowercase();
    let mut out = String::with_capacity(html.len());
    let mut i = 0;
    while let Some(rel) = lower[i..].find(&open_l) {
        let start = i + rel;
        out.push_str(&html[i..start]);
        let after_open = start + open.len();
        let close_at = lower[after_open..].find(&close_l);
        match close_at {
            Some(c) => i = after_open + c + close.len(),
            None => return out,
        }
    }
    out.push_str(&html[i..]);
    out
}

fn rename_html_tag(html: &str, from: &str, to: &str) -> String {
    let from_l = from.to_ascii_lowercase();
    let lower = html.to_ascii_lowercase();
    let l = lower.as_bytes();
    let from_b = from_l.as_bytes();
    let mut out = String::with_capacity(html.len());
    let mut i = 0;
    while i < html.len() {
        if html.as_bytes()[i] == b'<' {
            let mut name_at = i + 1;
            let slash = name_at < html.len() && html.as_bytes()[name_at] == b'/';
            if slash {
                name_at += 1;
            }
            if l.get(name_at..name_at + from_b.len()) == Some(from_b) {
                let after = name_at + from_b.len();
                if after == l.len()
                    || matches!(l[after], b' ' | b'\t' | b'\n' | b'\r' | b'>' | b'/')
                {
                    out.push('<');
                    if slash {
                        out.push('/');
                    }
                    out.push_str(to);
                    i = after;
                    continue;
                }
            }
        }
        let ch = html[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

pub fn markdown_is_blank(path: &Path) -> bool {
    match std::fs::read_to_string(path) {
        Ok(s) => s.trim().is_empty(),
        Err(_) => true,
    }
}

fn read_zip<R: Read + std::io::Seek>(zip: &mut ZipArchive<R>, name: &str) -> Option<Vec<u8>> {
    let n = name.trim_start_matches('/');
    let mut file = zip.by_name(n).ok()?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).ok()?;
    Some(buf)
}

fn altchunk_rids(document_xml: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let mut rest = document_xml;
    while let Some(i) = rest.find("altChunk") {
        let slice = &rest[i..];
        let end = slice.find('>').unwrap_or(slice.len());
        let tag = &slice[..end];
        if let Some(id) = attr_value(tag, "r:id") {
            ids.push(id);
        }
        rest = &slice[end.min(slice.len())..];
        if rest.is_empty() {
            break;
        }
    }
    ids
}

fn rel_target(rels_xml: &str, rid: &str) -> Option<String> {
    let mut rest = rels_xml;
    while let Some(i) = rest.find("<Relationship") {
        let slice = &rest[i..];
        let end = slice.find("/>").or_else(|| slice.find('>')).unwrap_or(slice.len());
        let tag = &slice[..end];
        if attr_value(tag, "Id").as_deref() == Some(rid) {
            return attr_value(tag, "Target");
        }
        rest = &slice[end.min(slice.len())..];
        if rest.is_empty() {
            break;
        }
    }
    None
}

fn attr_value(tag: &str, name: &str) -> Option<String> {
    let key = format!("{name}=\"");
    let i = tag.find(&key)?;
    let rest = &tag[i + key.len()..];
    let j = rest.find('"')?;
    Some(rest[..j].to_string())
}

fn package_part(rels_path: &str, target: &str) -> String {
    let t = target.trim();
    if t.starts_with('/') {
        return t.trim_start_matches('/').to_string();
    }
    let base = rels_path
        .rsplit_once('/')
        .map(|(d, _)| d)
        .unwrap_or("")
        .trim_end_matches("/_rels")
        .trim_end_matches("_rels");
    if base.is_empty() {
        t.to_string()
    } else {
        format!("{}/{}", base.trim_end_matches('/'), t)
    }
}

fn html_from_mht(raw: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(raw);
    let boundary = mht_boundary(&text)?;
    let marker = format!("--{boundary}");
    for part in text.split(&marker) {
        let part = part.trim();
        if part.is_empty() || part.starts_with("--") {
            continue;
        }
        let Some((headers, body)) = split_headers(part) else {
            continue;
        };
        let hl = headers.to_ascii_lowercase();
        if !hl.contains("text/html") {
            continue;
        }
        let decoded = if hl.contains("quoted-printable") {
            decode_quoted_printable(body)
        } else {
            body.to_string()
        };
        if !decoded.trim().is_empty() {
            return Some(decoded);
        }
    }
    None
}

fn mht_boundary(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let i = lower.find("boundary=")?;
    let rest = text[i + "boundary=".len()..].trim_start();
    let rest = rest.trim_start_matches('"').trim_start_matches('\'');
    let end = rest
        .find(|c: char| c == '"' || c == '\'' || c == ';' || c.is_whitespace())
        .unwrap_or(rest.len());
    let b = rest[..end].trim();
    if b.is_empty() {
        None
    } else {
        Some(b.to_string())
    }
}

fn split_headers(part: &str) -> Option<(&str, &str)> {
    if let Some(i) = part.find("\r\n\r\n") {
        return Some((&part[..i], &part[i + 4..]));
    }
    if let Some(i) = part.find("\n\n") {
        return Some((&part[..i], &part[i + 2..]));
    }
    None
}

fn decode_html_bytes(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn decode_quoted_printable(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'=' {
            if i + 1 < bytes.len() && (bytes[i + 1] == b'\n' || bytes[i + 1] == b'\r') {
                i += 1;
                if bytes[i] == b'\r' && i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                    i += 1;
                }
                i += 1;
                continue;
            }
            if i + 2 < bytes.len() {
                if let Ok(s) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                    if let Ok(b) = u8::from_str_radix(s, 16) {
                        out.push(b);
                        i += 3;
                        continue;
                    }
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    fn write_docx(parts: &[(&str, &str)]) -> Vec<u8> {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut z = ZipWriter::new(&mut buf);
            let opts = SimpleFileOptions::default();
            for (name, body) in parts {
                z.start_file(*name, opts).unwrap();
                z.write_all(body.as_bytes()).unwrap();
            }
            z.finish().unwrap();
        }
        buf.into_inner()
    }

    #[test]
    fn qp_soft_break_and_equals() {
        assert_eq!(decode_quoted_printable("a=\r\nb=3Dc"), "ab=c");
    }

    #[test]
    fn mht_extracts_html_part() {
        let mht = "MIME-Version: 1.0\r\nContent-Type: multipart/related;\r\n    type=\"text/html\";\r\n    boundary=\"----=mhtDocumentPart\"\r\n\r\n------=mhtDocumentPart\r\nContent-Type: text/html;\r\n    charset=\"utf-8\"\r\nContent-Transfer-Encoding: quoted-printable\r\n\r\n<html>住院病案=E9=A6=96=E9=A1=B5</html>\r\n------=mhtDocumentPart--\r\n";
        let b = mht_boundary(mht);
        assert_eq!(b.as_deref(), Some("----=mhtDocumentPart"), "{b:?}");
        let marker = format!("--{}", b.unwrap());
        let n = mht.split(&marker).count();
        assert!(n >= 2, "parts={n} marker={marker:?}");
        let html = html_from_mht(mht.as_bytes()).expect("html");
        assert!(html.contains("住院病案首页"), "{html:?}");
    }

    #[test]
    fn normal_docx_is_none() {
        let bytes = write_docx(&[(
            "word/document.xml",
            r#"<w:document><w:body><w:p><w:t>Hi</w:t></w:p></w:body></w:document>"#,
        )]);
        let dir = std::env::temp_dir().join(format!("ml-docx-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("normal.docx");
        std::fs::write(&p, bytes).unwrap();
        assert!(unwrap_docx_for_convert(&p).unwrap().is_none());
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn altchunk_mht_unwraps_html() {
        let doc = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:altChunk r:id="htmlChunk" /></w:body></w:document>"#;
        let rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/aFChunk" Target="/word/afchunk.mht" Id="htmlChunk" /></Relationships>"#;
        let mht = "MIME-Version: 1.0\r\nContent-Type: multipart/related;\r\n    type=\"text/html\";\r\n    boundary=\"----=mhtDocumentPart\"\r\n\r\n------=mhtDocumentPart\r\nContent-Type: text/html;\r\n    charset=\"utf-8\"\r\nContent-Transfer-Encoding: quoted-printable\r\n\r\n<html><body>住院病案=E9=A6=96=E9=A1=B5</body></html>\r\n------=mhtDocumentPart--\r\n";
        let bytes = write_docx(&[
            ("word/document.xml", doc),
            ("word/_rels/document.xml.rels", rels),
            ("word/afchunk.mht", mht),
        ]);
        let dir = std::env::temp_dir().join(format!("ml-docx-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("alt.docx");
        std::fs::write(&p, bytes).unwrap();
        let html = unwrap_docx_for_convert(&p).unwrap().expect("html");
        assert!(html.contains("住院病案首页"), "{html}");
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn sanitize_turns_p_and_field_into_span() {
        let html = "<style>.x{}</style><table><tr><td><p>甲</p><field>乙</field></td></tr></table>";
        let s = sanitize_html_for_downmark(html);
        assert!(!s.to_ascii_lowercase().contains("<style"), "{s}");
        assert!(!s.contains("<p>"), "{s}");
        assert!(!s.to_ascii_lowercase().contains("<field"), "{s}");
        assert!(s.contains("<table>"), "{s}");
        assert!(s.contains("甲") && s.contains("乙"), "{s}");
        assert!(s.contains("<span"), "{s}");
    }

    #[test]
    fn downmark_keeps_table_after_sanitize() {
        let bin = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("third-party")
            .join("downmark")
            .join("downmark");
        if !bin.is_file() {
            return;
        }
        let html = sanitize_html_for_downmark(
            "<html><body><table><tr><td><p>A</p></td><td><field>B</field></td></tr><tr><td>1</td><td>2</td></tr></table></body></html>",
        );
        let dir = std::env::temp_dir().join(format!("ml-dm-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let inp = dir.join("t.html");
        let out = dir.join("t.md");
        std::fs::write(&inp, html).unwrap();
        let st = std::process::Command::new(&bin)
            .arg("-x")
            .arg(".html")
            .arg("-o")
            .arg(&out)
            .arg(&inp)
            .status()
            .unwrap();
        assert!(st.success());
        let md = std::fs::read_to_string(&out).unwrap();
        assert!(md.contains('|'), "{md}");
        let _ = std::fs::remove_file(&inp);
        let _ = std::fs::remove_file(&out);
    }
}
