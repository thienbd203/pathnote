use super::{attributes::*, marks::Mark, node::Node, Document, Reference};
use crate::AppError;
use std::collections::HashSet;
impl Node {
    pub(super) fn children(&self) -> &[Node] {
        match self {
            Self::Image { content, .. }
            | Self::File { content, .. }
            | Self::Paragraph { content, .. }
            | Self::Heading { content, .. }
            | Self::BulletList { content, .. }
            | Self::OrderedList { content, .. }
            | Self::ListItem { content, .. }
            | Self::TaskList { content, .. }
            | Self::TaskItem { content, .. }
            | Self::Blockquote { content, .. }
            | Self::CodeBlock { content, .. }
            | Self::Table { content, .. }
            | Self::TableRow { content, .. }
            | Self::TableCell { content, .. }
            | Self::TableHeader { content, .. }
            | Self::Vocabulary { content, .. }
            | Self::VocabularyEntry { content, .. }
            | Self::VocabularySense { content, .. }
            | Self::VocabularyExample { content, .. } => content,
            _ => &[],
        }
    }
    fn block(&self) -> bool {
        matches!(
            self,
            Self::Exercise { .. }
                | Self::Image { .. }
                | Self::File { .. }
                | Self::Paragraph { .. }
                | Self::Heading { .. }
                | Self::BulletList { .. }
                | Self::OrderedList { .. }
                | Self::TaskList { .. }
                | Self::Blockquote { .. }
                | Self::CodeBlock { .. }
                | Self::Table { .. }
                | Self::HorizontalRule {}
                | Self::Vocabulary { .. }
                | Self::Pronunciation { .. }
        )
    }
    fn valid(&self, depth: usize, count: &mut usize, version: i32) -> bool {
        *count += 1;
        if depth > 32 || *count > 10000 {
            return false;
        }
        let c = self.children();
        let shape = match self {
            Self::Image { .. } => c
                .iter()
                .all(|n| matches!(n,Self::Text {marks,..} if marks.is_empty())),
            Self::File { .. } => c.is_empty(),
            Self::Paragraph { attrs, .. } => {
                (version >= 3 || (attrs.text_align.is_none() && attrs.indent.is_none()))
                    && attrs.indent.unwrap_or(0) <= 6
                    && c.iter()
                        .all(|n| matches!(n, Self::Text { .. } | Self::HardBreak { .. }))
            }
            Self::Heading { attrs, .. } => {
                (1..=3).contains(&attrs.level)
                    && (attrs.ai_import_id.is_none() || version >= 3 && attrs.level == 2)
                    && (version >= 3 || (attrs.text_align.is_none() && attrs.indent.is_none()))
                    && attrs.indent.unwrap_or(0) <= 6
                    && c.iter()
                        .all(|n| matches!(n, Self::Text { .. } | Self::HardBreak { .. }))
            }
            Self::BulletList { .. } | Self::OrderedList { .. } => {
                !c.is_empty() && c.iter().all(|n| matches!(n, Self::ListItem { .. }))
            }
            Self::TaskList { .. } => {
                !c.is_empty() && c.iter().all(|n| matches!(n, Self::TaskItem { .. }))
            }
            Self::ListItem { .. } | Self::TaskItem { .. } => {
                matches!(c.first(), Some(Self::Paragraph { .. })) && c.iter().all(Self::block)
            }
            Self::Blockquote { .. } | Self::TableCell { .. } | Self::TableHeader { .. } => {
                !c.is_empty() && c.iter().all(Self::block)
            }
            Self::CodeBlock { .. } => c
                .iter()
                .all(|n| matches!(n,Self::Text{marks,..} if marks.is_empty())),
            Self::Table { .. } => {
                !c.is_empty()
                    && c.len() <= 100
                    && c.iter().all(|n| matches!(n, Self::TableRow { .. }))
                    && valid_table(c)
            }
            Self::TableRow { .. } => {
                !c.is_empty()
                    && c.iter()
                        .all(|n| matches!(n, Self::TableCell { .. } | Self::TableHeader { .. }))
            }
            Self::Vocabulary { .. } => {
                !c.is_empty()
                    && c.iter().all(|n| matches!(n, Self::VocabularyEntry { .. }))
                    && unique_ids(c)
            }
            Self::VocabularyEntry { .. } => {
                !c.is_empty()
                    && c.len() <= 10
                    && c.iter().all(|n| matches!(n, Self::VocabularySense { .. }))
            }
            Self::VocabularySense { .. } => {
                !c.is_empty()
                    && c.len() <= 20
                    && c.iter()
                        .all(|n| matches!(n, Self::VocabularyExample { .. }))
            }
            Self::VocabularyExample { .. } => c.is_empty(),
            Self::Text { text, marks } => !text.is_empty() && valid_marks(marks, version),
            Self::HardBreak { marks } => valid_marks(marks, version),
            _ => true,
        };
        if !shape {
            return false;
        }
        if version < 2
            && matches!(
                self,
                Self::TaskList { .. }
                    | Self::TaskItem { .. }
                    | Self::Blockquote { .. }
                    | Self::HorizontalRule {}
                    | Self::CodeBlock { .. }
                    | Self::Table { .. }
            )
        {
            return false;
        }
        if version < 3
            && matches!(
                self,
                Self::Exercise { .. }
                    | Self::Image { .. }
                    | Self::File { .. }
                    | Self::Vocabulary { .. }
                    | Self::VocabularyEntry { .. }
                    | Self::VocabularySense { .. }
                    | Self::VocabularyExample { .. }
                    | Self::Pronunciation { .. }
            )
        {
            return false;
        }
        let attrs = match self {
            Self::Exercise { attrs } => attrs.exercise_id > 0,
            Self::Image { attrs, .. } => {
                attrs.media_id > 0
                    && attrs.alt.as_deref().is_none_or(|s| len(s, 200))
                    && !matches!(attrs.align, Some(Align::Justify))
                    && [attrs.width, attrs.height]
                        .iter()
                        .all(|v| v.is_none_or(|n| (1..=8192).contains(&n)))
            }
            Self::File { attrs, .. } => attrs.media_id > 0,
            Self::OrderedList { attrs, .. } => {
                attrs.start.unwrap_or(1) > 0
                    && attrs
                        .numbering
                        .as_deref()
                        .is_none_or(|v| ["1", "a", "A", "i", "I"].contains(&v))
            }
            Self::CodeBlock { attrs, .. } => attrs.language.as_deref().is_none_or(|l| {
                !l.is_empty()
                    && l.len() <= 40
                    && l.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"_+#.-".contains(&b))
            }),
            Self::TableCell { attrs, .. } | Self::TableHeader { attrs, .. } => valid_cell(attrs),
            Self::VocabularyEntry { attrs, .. } => {
                uuid(&attrs.id)
                    && len(&attrs.word, 1000)
                    && len(&attrs.ipa, 6000)
                    && strings(&attrs.topics, 60)
            }
            Self::VocabularySense { attrs, .. } => uuid(&attrs.id) && len(&attrs.meaning, 6000),
            Self::VocabularyExample { attrs, .. } => {
                uuid(&attrs.id)
                    && len(&attrs.text, 1000)
                    && strings(&attrs.highlights, 200)
                    && attrs.ipa.as_deref().is_none_or(|v| len(v, 6000))
                    && attrs.meaning.as_deref().is_none_or(|v| len(v, 6000))
                    && attrs
                        .meaning_highlights
                        .as_deref()
                        .is_none_or(|v| strings(v, 200))
            }
            Self::Pronunciation { attrs } => {
                len(&attrs.word, 1000)
                    && len(&attrs.ipa, 6000)
                    && attrs.meaning.as_deref().is_none_or(|v| len(v, 6000))
            }
            _ => true,
        };
        attrs && c.iter().all(|n| n.valid(depth + 1, count, version))
    }
}
fn len(s: &str, max: usize) -> bool {
    s.chars().count() <= max
}
fn uuid(s: &str) -> bool {
    s.len() == 36
        && s.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
            }
        })
}
fn strings(s: &[String], max: usize) -> bool {
    s.len() <= 20
        && s.iter().all(|v| !v.trim().is_empty() && len(v, max))
        && s.iter().collect::<HashSet<_>>().len() == s.len()
}
fn color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].bytes().all(|b| b.is_ascii_hexdigit())
}
fn valid_marks(m: &[Mark], version: i32) -> bool {
    let mut seen = HashSet::new();
    for mark in m {
        if !seen.insert(std::mem::discriminant(mark)) {
            return false;
        }
        match mark {
            Mark::Link { attrs } => {
                let Ok(url) = reqwest::Url::parse(&attrs.href) else {
                    return false;
                };
                if attrs.href.len() > 2048
                    || attrs.href.bytes().any(|b| b <= 32 || b == 127)
                    || !["https", "http", "mailto"].contains(&url.scheme())
                    || (["http", "https"].contains(&url.scheme()) && url.host_str().is_none())
                    || attrs
                        .target
                        .as_deref()
                        .is_some_and(|v| !["_blank", "_self"].contains(&v))
                    || attrs
                        .rel
                        .as_deref()
                        .is_some_and(|v| v != "noopener noreferrer nofollow")
                    || attrs.class.is_some()
                    || attrs.title.as_deref().is_some_and(|v| !len(v, 200))
                {
                    return false;
                }
            }
            Mark::TextStyle { attrs }
                if attrs.color.as_deref().is_some_and(|v| !color(v))
                    || attrs.background_color.as_deref().is_some_and(|v| !color(v)) =>
            {
                return false;
            }
            _ => {}
        }
        if version < 2 && matches!(mark, Mark::Underline {} | Mark::Link { .. }) {
            return false;
        }
        if version < 3
            && matches!(
                mark,
                Mark::TextStyle { .. } | Mark::Superscript {} | Mark::Subscript {}
            )
        {
            return false;
        }
    }
    !(m.iter().any(|v| matches!(v, Mark::Code {})) && m.len() > 1
        || m.iter().any(|v| matches!(v, Mark::Superscript {}))
            && m.iter().any(|v| matches!(v, Mark::Subscript {})))
}
fn valid_cell(a: &CellAttrs) -> bool {
    let w = a.colspan.unwrap_or(1);
    let h = a.rowspan.unwrap_or(1);
    (1..=20).contains(&w)
        && (1..=100).contains(&h)
        && !matches!(a.align, Some(Align::Justify))
        && a.colwidth
            .as_ref()
            .is_none_or(|v| v.len() == w as usize && v.iter().all(|n| *n <= 2000))
}
fn valid_table(rows: &[Node]) -> bool {
    let mut grid = vec![[false; 20]; rows.len()];
    for (y, row) in rows.iter().enumerate() {
        let mut x = 0;
        for cell in row.children() {
            while x < 20 && grid[y][x] {
                x += 1
            }
            let a = match cell {
                Node::TableCell { attrs, .. } | Node::TableHeader { attrs, .. } => attrs,
                _ => return false,
            };
            let w = a.colspan.unwrap_or(1) as usize;
            let h = a.rowspan.unwrap_or(1) as usize;
            if w == 0 || h == 0 || x + w > 20 || y + h > rows.len() {
                return false;
            }
            for line in grid.iter_mut().skip(y).take(h) {
                for slot in line.iter_mut().skip(x).take(w) {
                    if *slot {
                        return false;
                    }
                    *slot = true
                }
            }
            x += w
        }
    }
    let width = grid[0].iter().take_while(|v| **v).count();
    width > 0
        && grid
            .iter()
            .all(|r| r.iter().enumerate().all(|(x, v)| *v == (x < width)))
}
fn unique_ids(nodes: &[Node]) -> bool {
    fn visit<'a>(n: &'a Node, seen: &mut HashSet<&'a str>) -> bool {
        let id = match n {
            Node::VocabularyEntry { attrs, .. } => Some(attrs.id.as_str()),
            Node::VocabularySense { attrs, .. } => Some(attrs.id.as_str()),
            Node::VocabularyExample { attrs, .. } => Some(attrs.id.as_str()),
            _ => None,
        };
        id.is_none_or(|v| seen.insert(v)) && n.children().iter().all(|c| visit(c, seen))
    }
    let mut seen = HashSet::new();
    nodes.iter().all(|n| visit(n, &mut seen))
}
pub fn validate(content: &serde_json::Value, version: i32) -> Result<Vec<Reference>, AppError> {
    if !(1..=3).contains(&version) {
        return Err(AppError::validation(
            "schema_version",
            "Phiên bản ghi chép chưa hỗ trợ.",
        ));
    }
    let raw = serde_json::to_vec(content)
        .map_err(|_| AppError::validation("content", "JSON không hợp lệ."))?;
    if raw.len() > 262144 {
        return Err(AppError::validation("content", "Ghi chép tối đa 256 KiB."));
    }
    let doc: Document = serde_json::from_slice(&raw).map_err(|_| {
        AppError::validation(
            "content",
            "Cấu trúc ghi chép không hợp lệ hoặc chứa block chưa hỗ trợ.",
        )
    })?;
    let _ = &doc.kind;
    let mut count = 1;
    if doc.content.is_empty()
        || !doc
            .content
            .iter()
            .all(|n| n.block() && n.valid(1, &mut count, version))
    {
        return Err(AppError::validation(
            "content",
            "Cấu trúc ghi chép không hợp lệ.",
        ));
    }
    fn refs(node: &Node, found: &mut Vec<Reference>) {
        match node {
            Node::Heading { attrs, .. } if attrs.ai_import_id.is_some() => {
                found.push(Reference::AIStudy(attrs.ai_import_id.unwrap()))
            }
            Node::Exercise { attrs } => found.push(Reference::Exercise(attrs.exercise_id)),
            Node::Image { attrs, .. } => found.push(Reference::Media {
                id: attrs.media_id,
                kind: "image",
            }),
            Node::File { attrs, .. } => found.push(Reference::Media {
                id: attrs.media_id,
                kind: "file",
            }),
            _ => {}
        }
        for child in node.children() {
            refs(child, found)
        }
    }
    let mut found = Vec::new();
    for node in &doc.content {
        refs(node, &mut found)
    }
    Ok(found)
}
