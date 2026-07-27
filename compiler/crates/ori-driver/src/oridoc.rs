use ori_diagnostics::{FileId, Span};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default)]
pub struct OridocDoc {
    pub body: Vec<String>,
    pub params: Vec<(String, String)>,
    pub returns: Option<String>,
}

#[derive(Clone, Debug)]
pub struct OridocEntry {
    pub namespace: String,
    pub kind: String,
    pub target: String,
    pub symbol: String,
    pub doc: OridocDoc,
    pub path: PathBuf,
    pub file_id: Option<FileId>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct OridocDiagnostic {
    pub message: String,
    pub span: Span,
    pub action: &'static str,
}

#[derive(Clone, Debug, Default)]
pub struct OridocFile {
    pub namespace: Option<String>,
    pub entries: Vec<OridocEntry>,
    pub diagnostics: Vec<OridocDiagnostic>,
}

#[derive(Clone, Debug, Default)]
pub struct OridocIndex {
    entries: BTreeMap<String, Vec<OridocEntry>>,
}

impl OridocIndex {
    pub fn insert(&mut self, entry: OridocEntry) {
        self.entries
            .entry(entry.symbol.clone())
            .or_default()
            .push(entry);
    }

    pub fn get(&self, symbol: &str) -> Option<&OridocEntry> {
        self.entries.get(symbol).and_then(|entries| entries.first())
    }

    pub fn entries(&self) -> impl Iterator<Item = &OridocEntry> {
        self.entries.values().flat_map(|entries| entries.iter())
    }

    pub fn symbols(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }
}

#[derive(Clone, Debug)]
struct Line<'a> {
    text: &'a str,
    start: usize,
    end: usize,
}

#[derive(Clone, Debug)]
struct EntryBuilder {
    namespace: String,
    kind: String,
    target: String,
    path: PathBuf,
    span_start: usize,
    span_end: usize,
    doc: OridocDoc,
    section: Section,
}

#[derive(Clone, Debug, Default)]
enum Section {
    #[default]
    Body,
    Param(String),
    Returns,
}

pub fn parse_oridoc(path: &Path, source: &str) -> OridocFile {
    let mut file = OridocFile::default();
    let mut namespace: Option<String> = None;
    let mut current: Option<EntryBuilder> = None;

    for line in source_lines(source) {
        let trimmed = line.text.trim();
        if trimmed.is_empty() || trimmed.starts_with("--") {
            if let Some(builder) = current.as_mut() {
                builder.push_line("");
            }
            continue;
        }

        if let Some(builder) = current.as_mut() {
            if trimmed == "end" {
                let mut builder = current.take().expect("entry builder exists");
                builder.span_end = line.end;
                file.entries.push(builder.finish());
                continue;
            }
            if builder.try_section_header(trimmed) {
                continue;
            }
            builder.push_line(trimmed);
            continue;
        }

        if trimmed == "oridoc 1" {
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("namespace ") {
            let value = rest.trim();
            if is_valid_qualified_name(value) {
                namespace = Some(value.to_string());
                file.namespace = namespace.clone();
            } else {
                file.diagnostics.push(OridocDiagnostic {
                    message: format!("invalid `.oridoc` namespace `{value}`"),
                    span: Span::new(line.start, line.end),
                    action: "use a dotted Ori namespace, for example `namespace app.math`",
                });
            }
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("doc ") {
            let Some(ns) = namespace.clone() else {
                file.diagnostics.push(OridocDiagnostic {
                    message: "`doc` entry appears before `namespace`".into(),
                    span: Span::new(line.start, line.end),
                    action: "declare `namespace ...` before the first `doc ...` entry",
                });
                continue;
            };
            let mut parts = rest.splitn(2, char::is_whitespace);
            let kind = parts.next().unwrap_or("").trim();
            let target = parts.next().unwrap_or("").trim();
            if kind.is_empty() || target.is_empty() {
                file.diagnostics.push(OridocDiagnostic {
                    message: "`.oridoc` doc entry needs a kind and a target".into(),
                    span: Span::new(line.start, line.end),
                    action: "write entries like `doc func add` or `doc method User.name`",
                });
                continue;
            }
            let symbol = qualified_symbol(&ns, kind, target);
            current = Some(EntryBuilder {
                namespace: ns,
                kind: kind.to_string(),
                target: target.to_string(),
                path: path.to_path_buf(),
                span_start: line.start,
                span_end: line.end,
                doc: OridocDoc::default(),
                section: Section::Body,
            });
            if symbol.is_empty() {
                file.diagnostics.push(OridocDiagnostic {
                    message: "`.oridoc` doc entry has an invalid target".into(),
                    span: Span::new(line.start, line.end),
                    action: "use a local name such as `add` or `User.name`",
                });
                current = None;
            }
            continue;
        }

        file.diagnostics.push(OridocDiagnostic {
            message: format!("unexpected `.oridoc` line `{trimmed}`"),
            span: Span::new(line.start, line.end),
            action: "use `oridoc 1`, `namespace ...`, or a `doc ... end` block",
        });
    }

    if let Some(builder) = current {
        file.diagnostics.push(OridocDiagnostic {
            message: format!(
                "`.oridoc` doc entry `{} {}` is missing `end`",
                builder.kind, builder.target
            ),
            span: Span::new(builder.span_start, builder.span_end),
            action: "close the documentation block with `end`",
        });
    }

    file
}

pub fn hover_markdown(entry: &OridocEntry) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "`{}`", entry.symbol);
    let _ = writeln!(out);
    let _ = writeln!(out, "Kind: {}", entry.kind);
    let _ = writeln!(out);
    append_doc_body(&mut out, &entry.doc);
    out.trim_end().to_string()
}

pub fn append_doc_body(out: &mut String, doc: &OridocDoc) {
    if !doc.body.is_empty() {
        for line in &doc.body {
            let _ = writeln!(out, "{line}");
        }
        let _ = writeln!(out);
    }

    if !doc.params.is_empty() {
        let _ = writeln!(out, "Parameters:");
        for (name, description) in &doc.params {
            if description.is_empty() {
                let _ = writeln!(out, "- `{name}`");
            } else {
                let _ = writeln!(out, "- `{name}`: {description}");
            }
        }
        let _ = writeln!(out);
    }

    if let Some(returns) = &doc.returns {
        let _ = writeln!(out, "Returns: {returns}");
        let _ = writeln!(out);
    }
}

fn source_lines(source: &str) -> Vec<Line<'_>> {
    let mut out = Vec::new();
    let mut start = 0usize;
    for line in source.split_inclusive('\n') {
        let end = start + line.len();
        out.push(Line {
            text: line.trim_end_matches('\n').trim_end_matches('\r'),
            start,
            end,
        });
        start = end;
    }
    if source.is_empty() {
        out.push(Line {
            text: "",
            start: 0,
            end: 0,
        });
    }
    out
}

fn qualified_symbol(namespace: &str, kind: &str, target: &str) -> String {
    if kind == "module" && (target == "self" || target == namespace) {
        return namespace.to_string();
    }
    if target == namespace || target.starts_with(&format!("{namespace}.")) {
        return target.to_string();
    }
    if !is_valid_qualified_name(target) {
        return String::new();
    }
    format!("{namespace}.{target}")
}

fn is_valid_qualified_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .split('.')
            .all(|part| !part.is_empty() && is_valid_ident(part))
}

fn is_valid_ident(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

impl EntryBuilder {
    fn try_section_header(&mut self, line: &str) -> bool {
        if let Some(rest) = line.strip_prefix("summary:") {
            self.section = Section::Body;
            self.push_line(rest.trim());
            return true;
        }
        if let Some(rest) = line.strip_prefix("details:") {
            self.section = Section::Body;
            self.push_line(rest.trim());
            return true;
        }
        if let Some(rest) = line
            .strip_prefix("returns:")
            .or_else(|| line.strip_prefix("return:"))
        {
            self.section = Section::Returns;
            self.push_line(rest.trim());
            return true;
        }
        if let Some(rest) = line.strip_prefix("param ") {
            let Some((name, description)) = rest.split_once(':') else {
                return false;
            };
            let name = name.trim();
            self.section = Section::Param(name.to_string());
            self.doc
                .params
                .push((name.to_string(), description.trim().to_string()));
            return true;
        }
        false
    }

    fn push_line(&mut self, line: &str) {
        match &self.section {
            Section::Body => self.doc.body.push(line.to_string()),
            Section::Param(name) => {
                if line.is_empty() {
                    return;
                }
                if let Some((_, description)) = self
                    .doc
                    .params
                    .iter_mut()
                    .rev()
                    .find(|(param, _)| param == name)
                {
                    if !description.is_empty() {
                        description.push(' ');
                    }
                    description.push_str(line);
                }
            }
            Section::Returns => {
                if line.is_empty() {
                    return;
                }
                let returns = self.doc.returns.get_or_insert_with(String::new);
                if !returns.is_empty() {
                    returns.push(' ');
                }
                returns.push_str(line);
            }
        }
    }

    fn finish(mut self) -> OridocEntry {
        trim_empty_lines(&mut self.doc.body);
        for (_, description) in &mut self.doc.params {
            *description = description.trim().to_string();
        }
        if self
            .doc
            .returns
            .as_ref()
            .is_some_and(|text| text.trim().is_empty())
        {
            self.doc.returns = None;
        }
        let symbol = qualified_symbol(&self.namespace, &self.kind, &self.target);
        OridocEntry {
            namespace: self.namespace,
            kind: self.kind,
            target: self.target,
            symbol,
            doc: self.doc,
            path: self.path,
            file_id: None,
            span: Span::new(self.span_start, self.span_end),
        }
    }
}

fn trim_empty_lines(lines: &mut Vec<String>) {
    while lines.first().is_some_and(|line| line.is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
}
