use base64::{engine::general_purpose::STANDARD, Engine};
use roxmltree::{Document, Node};
use serde::Serialize;
use std::{
    collections::HashMap,
    fs::File,
    io::Read,
    path::{Component, Path, PathBuf},
};
use zip::ZipArchive;

const MAX_PART: u64 = 16 * 1024 * 1024;
type Package = ZipArchive<File>;

#[derive(Serialize)]
pub(crate) struct Block {
    kind: String,
    text: String,
    image: Option<String>,
    children: Vec<Block>,
    style: Option<TextStyle>,
}
#[derive(Serialize)]
pub(crate) struct WordDocument {
    blocks: Vec<Block>,
    width: f64,
    height: f64,
    margins: [f64; 4],
}
#[derive(Clone, Default, Serialize)]
struct TextStyle {
    bold: bool,
    italic: bool,
    underline: bool,
    size_pt: Option<f64>,
    color: Option<String>,
    font: Option<String>,
    align: Option<String>,
    list_level: Option<u8>,
    marker: Option<String>,
    width_px: Option<f64>,
    height_px: Option<f64>,
}
#[derive(Serialize)]
struct SlideParagraph {
    align: Option<String>,
    marker: Option<String>,
    level: u8,
    runs: Vec<SlideRun>,
}
#[derive(Serialize)]
struct SlideRun {
    text: String,
    style: TextStyle,
}
#[derive(Serialize)]
pub(crate) struct SlideItem {
    kind: String,
    text: String,
    image: Option<String>,
    fill: Option<String>,
    outline: Option<String>,
    geometry: String,
    rotation: f64,
    flip_h: bool,
    flip_v: bool,
    paragraphs: Vec<SlideParagraph>,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}
#[derive(Serialize)]
pub(crate) struct Slide {
    width: f64,
    height: f64,
    background: Option<String>,
    items: Vec<SlideItem>,
}

fn package(path: &Path) -> Result<Package, String> {
    ZipArchive::new(File::open(path).map_err(|_| "The Office file could not be opened.")?)
        .map_err(|_| "The Office file is damaged, encrypted, or cannot be read.".into())
}
fn part(zip: &mut Package, path: &str) -> Result<Vec<u8>, String> {
    let mut f = zip
        .by_name(path)
        .map_err(|_| format!("A required document part is missing: {path}"))?;
    if f.size() > MAX_PART {
        return Err("A document part exceeds the 16 MB limit.".into());
    }
    let mut bytes = Vec::new();
    f.by_ref()
        .take(MAX_PART + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "A document part could not be read.")?;
    if bytes.len() as u64 > MAX_PART {
        return Err("A document part exceeds the 16 MB limit.".into());
    }
    Ok(bytes)
}
fn xml(zip: &mut Package, path: &str) -> Result<String, String> {
    String::from_utf8(part(zip, path)?).map_err(|_| "A document part is not valid UTF-8.".into())
}
fn resolve(base: &str, target: &str) -> Option<String> {
    let mut path = PathBuf::new();
    let joined = if target.starts_with('/') {
        PathBuf::from(target.trim_start_matches('/'))
    } else {
        Path::new(base).parent()?.join(target)
    };
    for component in joined.components() {
        match component {
            Component::Normal(c) => path.push(c),
            Component::ParentDir => {
                if !path.pop() {
                    return None;
                }
            }
            Component::CurDir => {}
            _ => return None,
        }
    }
    path.to_str().map(str::to_owned)
}
fn relations(zip: &mut Package, source: &str) -> HashMap<String, String> {
    let path = Path::new(source);
    let rel = format!(
        "{}/_rels/{}.rels",
        path.parent().unwrap_or(Path::new("")).display(),
        path.file_name().unwrap_or_default().to_string_lossy()
    );
    let Ok(contents) = xml(zip, &rel) else {
        return HashMap::new();
    };
    let Ok(doc) = Document::parse(&contents) else {
        return HashMap::new();
    };
    doc.descendants()
        .filter(|n| n.has_tag_name("Relationship") && n.attribute("TargetMode") != Some("External"))
        .filter_map(|n| {
            Some((
                n.attribute("Id")?.to_owned(),
                resolve(source, n.attribute("Target")?)?,
            ))
        })
        .collect()
}
fn image(zip: &mut Package, path: &str, budget: &mut usize) -> Option<String> {
    let mime = match Path::new(path)
        .extension()?
        .to_str()?
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => return None,
    };
    let bytes = part(zip, path).ok()?;
    if bytes.len() > *budget {
        return None;
    }
    *budget -= bytes.len();
    Some(format!("data:{mime};base64,{}", STANDARD.encode(bytes)))
}
fn image_for(
    node: Node<'_, '_>,
    rels: &HashMap<String, String>,
    zip: &mut Package,
    budget: &mut usize,
) -> Option<String> {
    let id = node
        .descendants()
        .find(|n| n.has_tag_name("blip"))?
        .attributes()
        .find(|a| a.name() == "embed")?
        .value();
    image(zip, rels.get(id)?, budget)
}
fn block(kind: &str, text: String, image: Option<String>, children: Vec<Block>) -> Block {
    Block {
        kind: kind.into(),
        text,
        image,
        children,
        style: None,
    }
}
fn attr<'a>(node: Node<'a, '_>, name: &str) -> Option<&'a str> {
    node.attributes()
        .find(|a| a.name() == name)
        .map(|a| a.value())
}
fn on(node: Node<'_, '_>) -> bool {
    !matches!(attr(node, "val"), Some("0" | "false" | "off" | "none"))
}
fn color(node: Node<'_, '_>, theme: &HashMap<String, String>) -> Option<String> {
    let color = node
        .descendants()
        .find(|n| n.has_tag_name("srgbClr") || n.has_tag_name("schemeClr"))?;
    let value = color.attribute("val")?;
    if color.has_tag_name("schemeClr") {
        let key = match value {
            "tx1" => "dk1",
            "bg1" => "lt1",
            "tx2" => "dk2",
            "bg2" => "lt2",
            other => other,
        };
        return theme.get(key).cloned();
    }
    value
        .len()
        .eq(&6)
        .then_some(value)
        .filter(|v| v.chars().all(|c| c.is_ascii_hexdigit()))
        .map(|v| format!("#{v}"))
}
fn related(zip: &mut Package, source: &str, kind: &str) -> Option<String> {
    let path = Path::new(source);
    let rel = format!(
        "{}/_rels/{}.rels",
        path.parent()?.display(),
        path.file_name()?.to_str()?
    );
    let contents = xml(zip, &rel).ok()?;
    let doc = Document::parse(&contents).ok()?;
    let target = doc
        .descendants()
        .find(|n| {
            n.has_tag_name("Relationship")
                && n.attribute("TargetMode") != Some("External")
                && n.attribute("Type")
                    .is_some_and(|t| t.rsplit('/').next() == Some(kind))
        })?
        .attribute("Target")?;
    resolve(source, target)
}
fn slide_theme(zip: &mut Package, slide: &str) -> HashMap<String, String> {
    let Some(layout) = related(zip, slide, "slideLayout") else {
        return HashMap::new();
    };
    let Some(master) = related(zip, &layout, "slideMaster") else {
        return HashMap::new();
    };
    let Some(theme) = related(zip, &master, "theme") else {
        return HashMap::new();
    };
    let Ok(contents) = xml(zip, &theme) else {
        return HashMap::new();
    };
    let Ok(doc) = Document::parse(&contents) else {
        return HashMap::new();
    };
    let Some(scheme) = doc.descendants().find(|n| n.has_tag_name("clrScheme")) else {
        return HashMap::new();
    };
    scheme
        .children()
        .filter(|n| n.is_element())
        .filter_map(|n| {
            let color = n
                .children()
                .find(|c| c.has_tag_name("srgbClr") || c.has_tag_name("sysClr"))?;
            let value = if color.has_tag_name("sysClr") {
                color.attribute("lastClr")?
            } else {
                color.attribute("val")?
            };
            (value.len() == 6 && value.chars().all(|c| c.is_ascii_hexdigit()))
                .then(|| (n.tag_name().name().to_owned(), format!("#{value}")))
        })
        .collect()
}
fn safe_font(value: &str) -> Option<String> {
    if !value.is_empty()
        && value.len() <= 80
        && value
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'))
    {
        Some(value.to_owned())
    } else {
        None
    }
}
fn word_style(props: Option<Node<'_, '_>>) -> TextStyle {
    let mut style = TextStyle::default();
    if let Some(props) = props {
        style.bold = props
            .children()
            .find(|n| n.has_tag_name("b"))
            .is_some_and(on);
        style.italic = props
            .children()
            .find(|n| n.has_tag_name("i"))
            .is_some_and(on);
        style.underline = props
            .children()
            .find(|n| n.has_tag_name("u"))
            .is_some_and(on);
        style.size_pt = props
            .children()
            .find(|n| n.has_tag_name("sz"))
            .and_then(|n| attr(n, "val"))
            .and_then(|v| v.parse::<f64>().ok())
            .map(|v| v / 2.0)
            .filter(|v| (4.0..=96.0).contains(v));
        style.color = props
            .children()
            .find(|n| n.has_tag_name("color"))
            .and_then(|n| attr(n, "val"))
            .filter(|v| v.len() == 6 && v.chars().all(|c| c.is_ascii_hexdigit()))
            .map(|v| format!("#{v}"));
        style.font = props
            .children()
            .find(|n| n.has_tag_name("rFonts"))
            .and_then(|n| attr(n, "ascii"))
            .and_then(safe_font);
    }
    style
}
#[derive(Default)]
struct Numbering {
    definitions: HashMap<(String, u8), (String, String, u32)>,
    counters: HashMap<String, [u32; 9]>,
}
fn numbering(zip: &mut Package) -> Numbering {
    let Ok(contents) = xml(zip, "word/numbering.xml") else {
        return Numbering::default();
    };
    let Ok(doc) = Document::parse(&contents) else {
        return Numbering::default();
    };
    let mut abstract_levels = HashMap::new();
    for abstract_num in doc.descendants().filter(|n| n.has_tag_name("abstractNum")) {
        let Some(id) = attr(abstract_num, "abstractNumId") else {
            continue;
        };
        for level in abstract_num.children().filter(|n| n.has_tag_name("lvl")) {
            let Some(index) = attr(level, "ilvl")
                .and_then(|v| v.parse::<u8>().ok())
                .filter(|v| *v < 9)
            else {
                continue;
            };
            let format = level
                .children()
                .find(|n| n.has_tag_name("numFmt"))
                .and_then(|n| attr(n, "val"))
                .unwrap_or("");
            let text = level
                .children()
                .find(|n| n.has_tag_name("lvlText"))
                .and_then(|n| attr(n, "val"))
                .unwrap_or("");
            let start = level
                .children()
                .find(|n| n.has_tag_name("start"))
                .and_then(|n| attr(n, "val"))
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(1);
            if matches!(format, "bullet" | "decimal")
                && text.len() <= 32
                && !text.chars().any(char::is_control)
            {
                abstract_levels.insert(
                    (id.to_owned(), index),
                    (format.to_owned(), text.to_owned(), start),
                );
            }
        }
    }
    let mut result = Numbering::default();
    for num in doc.descendants().filter(|n| n.has_tag_name("num")) {
        let Some(id) = attr(num, "numId") else {
            continue;
        };
        let Some(abstract_id) = num
            .children()
            .find(|n| n.has_tag_name("abstractNumId"))
            .and_then(|n| attr(n, "val"))
        else {
            continue;
        };
        for level in 0..9 {
            if let Some(definition) = abstract_levels.get(&(abstract_id.to_owned(), level)) {
                result
                    .definitions
                    .insert((id.to_owned(), level), definition.clone());
            }
        }
    }
    result
}
impl Numbering {
    fn marker(&mut self, id: &str, level: u8) -> Option<String> {
        let (format, text, start) = self.definitions.get(&(id.to_owned(), level))?;
        if format == "bullet" {
            return Some(text.clone());
        }
        let counters = self.counters.entry(id.to_owned()).or_insert([0; 9]);
        let index = level as usize;
        counters[index] = if counters[index] == 0 {
            *start
        } else {
            counters[index].saturating_add(1)
        };
        for value in &mut counters[index + 1..] {
            *value = 0;
        }
        let mut marker = text.clone();
        for (i, value) in counters.iter().enumerate() {
            marker = marker.replace(&format!("%{}", i + 1), &value.to_string());
        }
        Some(marker)
    }
}
fn paragraph(
    node: Node<'_, '_>,
    rels: &HashMap<String, String>,
    zip: &mut Package,
    budget: &mut usize,
    numbering: &mut Numbering,
) -> Block {
    let mut children = Vec::new();
    for run in node
        .descendants()
        .filter(|n| n.has_tag_name("r") && !n.ancestors().skip(1).any(|a| a.has_tag_name("r")))
    {
        for child in run.children().filter(|n| n.is_element()) {
            if child.has_tag_name("br") && attr(child, "type") == Some("page") {
                children.push(block("page_break", String::new(), None, vec![]));
            } else if child.has_tag_name("t")
                || child.has_tag_name("tab")
                || child.has_tag_name("br")
                || child.has_tag_name("cr")
            {
                let text = if child.has_tag_name("tab") {
                    "\t"
                } else if child.has_tag_name("br") || child.has_tag_name("cr") {
                    "\n"
                } else {
                    child.text().unwrap_or("")
                };
                let props = run.children().find(|n| n.has_tag_name("rPr"));
                let mut part = block("text", text.to_owned(), None, vec![]);
                part.style = Some(word_style(props));
                children.push(part);
            } else if child.has_tag_name("drawing") || child.has_tag_name("pict") {
                if let Some(img) = image_for(child, rels, zip, budget) {
                    let extent = child.descendants().find(|n| n.has_tag_name("extent"));
                    let dimension = |name| {
                        extent
                            .and_then(|n| n.attribute(name))
                            .and_then(|v| v.parse::<f64>().ok())
                            .map(|v| v / 9525.0)
                            .filter(|v| (1.0..=4000.0).contains(v))
                    };
                    let mut part = block("image", String::new(), Some(img), vec![]);
                    part.style = Some(TextStyle {
                        width_px: dimension("cx"),
                        height_px: dimension("cy"),
                        ..Default::default()
                    });
                    children.push(part);
                }
            }
        }
    }
    let style = node
        .children()
        .find(|n| n.has_tag_name("pPr"))
        .and_then(|n| n.children().find(|n| n.has_tag_name("pStyle")))
        .and_then(|n| n.attributes().find(|a| a.name() == "val"))
        .map(|a| a.value())
        .unwrap_or("");
    let kind = match style {
        "Heading1" => "h1",
        "Heading2" => "h2",
        "Heading3" => "h3",
        _ => "paragraph",
    };
    let props = node.children().find(|n| n.has_tag_name("pPr"));
    let mut formatting = TextStyle::default();
    formatting.align = props
        .and_then(|p| p.children().find(|n| n.has_tag_name("jc")))
        .and_then(|n| attr(n, "val"))
        .and_then(|v| match v {
            "left" | "start" => Some("left"),
            "right" | "end" => Some("right"),
            "center" => Some("center"),
            "both" => Some("justify"),
            _ => None,
        })
        .map(str::to_owned);
    formatting.list_level = props
        .and_then(|p| p.children().find(|n| n.has_tag_name("numPr")))
        .map(|p| {
            p.children()
                .find(|n| n.has_tag_name("ilvl"))
                .and_then(|n| attr(n, "val"))
                .and_then(|v| v.parse::<u8>().ok())
                .unwrap_or(0)
                .min(8)
        });
    if let Some(level) = formatting.list_level {
        let id = props
            .and_then(|p| p.children().find(|n| n.has_tag_name("numPr")))
            .and_then(|p| p.children().find(|n| n.has_tag_name("numId")))
            .and_then(|n| attr(n, "val"));
        formatting.marker = id.and_then(|id| numbering.marker(id, level));
    }
    let mut result = block(kind, String::new(), None, children);
    result.style = Some(formatting);
    result
}
pub(crate) fn docx(path: &Path) -> Result<WordDocument, String> {
    let mut zip = package(path)?;
    let rels = relations(&mut zip, "word/document.xml");
    let contents = xml(&mut zip, "word/document.xml")?;
    let doc = Document::parse(&contents).map_err(|_| "The DOCX document XML is damaged.")?;
    let body = doc
        .descendants()
        .find(|n| n.has_tag_name("body"))
        .ok_or("The DOCX document has no body.")?;
    let mut blocks = Vec::new();
    let mut numbering = numbering(&mut zip);
    let mut budget = 32 * 1024 * 1024;
    for node in body.children().filter(|n| n.is_element()) {
        if node.has_tag_name("p") {
            if node
                .children()
                .find(|n| n.has_tag_name("pPr"))
                .and_then(|p| p.children().find(|n| n.has_tag_name("pageBreakBefore")))
                .is_some_and(on)
            {
                blocks.push(block("page_break", String::new(), None, vec![]));
            }
            blocks.push(paragraph(
                node,
                &rels,
                &mut zip,
                &mut budget,
                &mut numbering,
            ));
        }
        if node.has_tag_name("tbl") {
            let rows = node
                .children()
                .filter(|n| n.has_tag_name("tr"))
                .map(|row| {
                    let cells = row
                        .children()
                        .filter(|n| n.has_tag_name("tc"))
                        .map(|cell| {
                            let paragraphs = cell
                                .children()
                                .filter(|n| n.has_tag_name("p"))
                                .map(|p| paragraph(p, &rels, &mut zip, &mut budget, &mut numbering))
                                .collect();
                            block("cell", String::new(), None, paragraphs)
                        })
                        .collect();
                    block("row", String::new(), None, cells)
                })
                .collect();
            blocks.push(block("table", String::new(), None, rows));
        }
    }
    let section = body.descendants().find(|n| n.has_tag_name("sectPr"));
    let size = section.and_then(|n| n.children().find(|c| c.has_tag_name("pgSz")));
    let margin = section.and_then(|n| n.children().find(|c| c.has_tag_name("pgMar")));
    let dimension = |node: Option<Node<'_, '_>>, name, default: f64| {
        node.and_then(|n| attr(n, name))
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| (0.0..=28800.0).contains(v))
            .map(|v| v / 15.0)
            .unwrap_or(default)
    };
    let width = dimension(size, "w", 816.0).max(200.0);
    let height = dimension(size, "h", 1056.0).max(200.0);
    let margins = [
        dimension(margin, "top", 96.0),
        dimension(margin, "right", 96.0),
        dimension(margin, "bottom", 96.0),
        dimension(margin, "left", 96.0),
    ];
    Ok(WordDocument {
        blocks,
        width,
        height,
        margins,
    })
}
fn presentation(zip: &mut Package) -> Result<(Vec<String>, f64, f64), String> {
    let source = "ppt/presentation.xml";
    let rels = relations(zip, source);
    let contents = xml(zip, source)?;
    let doc = Document::parse(&contents).map_err(|_| "The presentation XML is damaged.")?;
    let ids = doc
        .descendants()
        .filter(|n| n.has_tag_name("sldId"))
        .filter_map(|n| {
            n.attributes()
                .find(|a| a.name() == "id" && a.namespace().is_some())
                .and_then(|a| rels.get(a.value()))
                .cloned()
        })
        .collect::<Vec<_>>();
    if ids.is_empty() {
        return Err("The presentation contains no readable slides.".into());
    }
    let size = doc.descendants().find(|n| n.has_tag_name("sldSz"));
    let width = size
        .and_then(|n| n.attribute("cx"))
        .and_then(|v| v.parse().ok())
        .unwrap_or(12192000.0);
    let height = size
        .and_then(|n| n.attribute("cy"))
        .and_then(|v| v.parse().ok())
        .unwrap_or(6858000.0);
    if width <= 0.0 || height <= 0.0 || !f64::is_finite(width) || !f64::is_finite(height) {
        return Err("The presentation has invalid slide dimensions.".into());
    }
    Ok((ids, width, height))
}
fn slide_style(
    props: Option<Node<'_, '_>>,
    mut style: TextStyle,
    theme: &HashMap<String, String>,
) -> TextStyle {
    if let Some(p) = props {
        if let Some(value) = p.attribute("b") {
            style.bold = value == "1" || value == "true";
        }
        if let Some(value) = p.attribute("i") {
            style.italic = value == "1" || value == "true";
        }
        if let Some(value) = p.attribute("u") {
            style.underline = value != "none";
        }
        if let Some(value) = p
            .attribute("sz")
            .and_then(|v| v.parse::<f64>().ok())
            .map(|n| n / 100.0)
            .filter(|n| (4.0..=96.0).contains(n))
        {
            style.size_pt = Some(value);
        }
        if let Some(value) = p
            .children()
            .find(|n| n.has_tag_name("solidFill"))
            .and_then(|n| color(n, theme))
        {
            style.color = Some(value);
        }
        if let Some(value) = p
            .children()
            .find(|n| n.has_tag_name("latin"))
            .and_then(|n| n.attribute("typeface"))
            .and_then(safe_font)
        {
            style.font = Some(value);
        }
    }
    style
}
fn slide_text(node: Node<'_, '_>, theme: &HashMap<String, String>) -> Vec<SlideParagraph> {
    node.children()
        .find(|n| n.has_tag_name("txBody"))
        .map(|body| {
            let mut counters = [0u32; 9];
            body.children()
                .filter(|n| n.has_tag_name("p"))
                .map(|paragraph| {
                    let align = paragraph
                        .children()
                        .find(|n| n.has_tag_name("pPr"))
                        .and_then(|n| n.attribute("algn"))
                        .and_then(|a| match a {
                            "l" => Some("left"),
                            "ctr" => Some("center"),
                            "r" => Some("right"),
                            "just" => Some("justify"),
                            _ => None,
                        })
                        .map(str::to_owned);
                    let properties = paragraph.children().find(|n| n.has_tag_name("pPr"));
                    let level = properties
                        .and_then(|p| p.attribute("lvl"))
                        .and_then(|v| v.parse::<u8>().ok())
                        .unwrap_or(0)
                        .min(8);
                    let marker = properties
                        .and_then(|p| {
                            p.children()
                                .find(|n| n.has_tag_name("buChar") || n.has_tag_name("buAutoNum"))
                        })
                        .and_then(|n| {
                            if n.has_tag_name("buChar") {
                                return n
                                    .attribute("char")
                                    .filter(|v| v.len() <= 16 && !v.chars().any(char::is_control))
                                    .map(str::to_owned);
                            }
                            let format = n.attribute("type").unwrap_or("arabicPeriod");
                            if !matches!(format, "arabicPeriod" | "arabicParenR") {
                                return None;
                            }
                            let current = &mut counters[level as usize];
                            *current = if *current == 0 {
                                n.attribute("startAt")
                                    .and_then(|v| v.parse().ok())
                                    .unwrap_or(1)
                            } else {
                                current.saturating_add(1)
                            };
                            Some(format!(
                                "{}{}",
                                *current,
                                if format == "arabicParenR" { ")" } else { "." }
                            ))
                        });
                    let default_style = slide_style(
                        properties.and_then(|p| p.children().find(|n| n.has_tag_name("defRPr"))),
                        TextStyle::default(),
                        theme,
                    );
                    let runs = paragraph
                        .children()
                        .filter(|n| {
                            n.has_tag_name("r") || n.has_tag_name("fld") || n.has_tag_name("br")
                        })
                        .map(|run| {
                            let props = run.children().find(|n| n.has_tag_name("rPr"));
                            let style = slide_style(props, default_style.clone(), theme);
                            let text = if run.has_tag_name("br") {
                                "\n".into()
                            } else {
                                run.children()
                                    .find(|n| n.has_tag_name("t"))
                                    .and_then(|n| n.text())
                                    .unwrap_or("")
                                    .to_owned()
                            };
                            SlideRun { text, style }
                        })
                        .collect();
                    SlideParagraph {
                        align,
                        marker,
                        level,
                        runs,
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}
fn coordinates(node: Node<'_, '_>) -> (f64, f64, f64, f64) {
    let number = |node: Option<Node<'_, '_>>, name: &str| {
        node.and_then(|n| n.attribute(name))
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| v.is_finite())
            .unwrap_or(0.0)
    };
    let transform = node
        .children()
        .find(|n| n.has_tag_name("spPr"))
        .and_then(|p| p.children().find(|n| n.has_tag_name("xfrm")));
    let off = transform.and_then(|n| n.children().find(|n| n.has_tag_name("off")));
    let ext = transform.and_then(|n| n.children().find(|n| n.has_tag_name("ext")));
    let (mut x, mut y, mut width, mut height) = (
        number(off, "x"),
        number(off, "y"),
        number(ext, "cx"),
        number(ext, "cy"),
    );
    for group in node.ancestors().filter(|n| n.has_tag_name("grpSp")) {
        let transform = group
            .children()
            .find(|n| n.has_tag_name("grpSpPr"))
            .and_then(|p| p.children().find(|n| n.has_tag_name("xfrm")));
        let off = transform.and_then(|n| n.children().find(|n| n.has_tag_name("off")));
        let ext = transform.and_then(|n| n.children().find(|n| n.has_tag_name("ext")));
        let child_off = transform.and_then(|n| n.children().find(|n| n.has_tag_name("chOff")));
        let child_ext = transform.and_then(|n| n.children().find(|n| n.has_tag_name("chExt")));
        let (cx, cy) = (number(child_ext, "cx"), number(child_ext, "cy"));
        if cx > 0.0 && cy > 0.0 {
            let (sx, sy) = (number(ext, "cx") / cx, number(ext, "cy") / cy);
            x = number(off, "x") + (x - number(child_off, "x")) * sx;
            y = number(off, "y") + (y - number(child_off, "y")) * sy;
            width *= sx;
            height *= sy;
        }
    }
    (x, y, width, height)
}
fn placeholder(node: Node<'_, '_>) -> Option<u32> {
    node.children()
        .find(|n| n.has_tag_name("nvSpPr") || n.has_tag_name("nvPicPr"))?
        .descendants()
        .find(|n| n.has_tag_name("ph"))
        .and_then(|n| n.attribute("idx").unwrap_or("0").parse().ok())
}
pub(crate) struct Presentation {
    zip: Package,
    slides: Vec<String>,
    width: f64,
    height: f64,
}
impl Presentation {
    pub(crate) fn open(path: &Path) -> Result<Self, String> {
        let mut zip = package(path)?;
        let (slides, width, height) = presentation(&mut zip)?;
        Ok(Self {
            zip,
            slides,
            width,
            height,
        })
    }
    pub(crate) fn slide(&mut self, index: usize) -> Result<(usize, Slide), String> {
        let path = self
            .slides
            .get(index)
            .ok_or("The slide could not be found.")?;
        let layout = related(&mut self.zip, path, "slideLayout");
        let master = layout
            .as_deref()
            .and_then(|p| related(&mut self.zip, p, "slideMaster"));
        let theme = slide_theme(&mut self.zip, path);
        let contents = xml(&mut self.zip, path)?;
        // The slide is required; a damaged optional layout or master must not hide it.
        Document::parse(&contents).map_err(|_| "The slide XML is damaged.")?;
        let mut parts = Vec::new();
        for parent in [master.as_deref(), layout.as_deref()].into_iter().flatten() {
            if let Ok(content) = xml(&mut self.zip, parent) {
                if Document::parse(&content).is_ok() {
                    parts.push((parent.to_owned(), content));
                }
            }
        }
        parts.push((path.clone(), contents));
        let docs: Vec<_> = parts
            .iter()
            .filter_map(|(_, content)| Document::parse(content).ok())
            .collect();
        let layout_doc = layout
            .as_ref()
            .and_then(|p| parts.iter().position(|(path, _)| path == p))
            .map(|i| &docs[i]);
        let slide_doc = docs.last().unwrap();
        let show_master = !matches!(
            slide_doc.root_element().attribute("showMasterSp"),
            Some("0" | "false")
        );
        let layout_shows_master = layout_doc.is_none_or(|doc| {
            !matches!(
                doc.root_element().attribute("showMasterSp"),
                Some("0" | "false")
            )
        });
        let background = docs
            .iter()
            .rev()
            .find_map(|doc| {
                doc.descendants().find(|n| n.has_tag_name("bg")).map(|bg| {
                    bg.descendants()
                        .find(|n| n.has_tag_name("bgPr"))
                        .and_then(|n| n.children().find(|n| n.has_tag_name("solidFill")))
                        .and_then(|n| color(n, &theme))
                })
            })
            .flatten();
        let mut items = Vec::new();
        let mut budget = 32 * 1024 * 1024;
        for (layer, doc) in docs.iter().enumerate() {
            let is_slide = layer == docs.len() - 1;
            let is_master = master.as_ref().is_some_and(|p| parts[layer].0 == *p);
            if is_master && (!show_master || !layout_shows_master) {
                continue;
            }
            let rels = relations(&mut self.zip, &parts[layer].0);
            for node in doc
                .descendants()
                .filter(|n| n.has_tag_name("sp") || n.has_tag_name("pic"))
            {
                // Layout/master placeholders define slide content rather than standalone artwork.
                if !is_slide && placeholder(node).is_some() {
                    continue;
                }
                let properties = node.children().find(|n| n.has_tag_name("spPr"));
                let transform =
                    properties.and_then(|n| n.children().find(|n| n.has_tag_name("xfrm")));
                let rotation = transform
                    .and_then(|n| n.attribute("rot"))
                    .and_then(|v| v.parse::<f64>().ok())
                    .filter(|v| v.is_finite())
                    .map(|v| (v / 60000.0).rem_euclid(360.0))
                    .unwrap_or(0.0);
                let flip_h = transform
                    .and_then(|n| n.attribute("flipH"))
                    .is_some_and(|v| v == "1" || v == "true");
                let flip_v = transform
                    .and_then(|n| n.attribute("flipV"))
                    .is_some_and(|v| v == "1" || v == "true");
                let inherited = if transform.is_none() && layer == docs.len() - 1 {
                    placeholder(node).and_then(|key| {
                        layout_doc.and_then(|layout| {
                            layout.descendants().find(|candidate| {
                                candidate.has_tag_name("sp") && placeholder(*candidate) == Some(key)
                            })
                        })
                    })
                } else {
                    None
                };
                let (x, y, width, height) = coordinates(inherited.unwrap_or(node));
                let paragraphs = slide_text(node, &theme);
                let text = paragraphs
                    .iter()
                    .map(|p| p.runs.iter().map(|r| r.text.as_str()).collect::<String>())
                    .collect::<Vec<_>>()
                    .join("\n");
                let img = image_for(node, &rels, &mut self.zip, &mut budget);
                let fill = properties
                    .and_then(|p| p.children().find(|n| n.has_tag_name("solidFill")))
                    .and_then(|n| color(n, &theme));
                let outline = properties
                    .and_then(|p| p.children().find(|n| n.has_tag_name("ln")))
                    .and_then(|p| p.children().find(|n| n.has_tag_name("solidFill")))
                    .and_then(|n| color(n, &theme));
                let geometry = properties
                    .and_then(|p| p.children().find(|n| n.has_tag_name("prstGeom")))
                    .and_then(|n| n.attribute("prst"))
                    .filter(|v| matches!(*v, "rect" | "roundRect" | "ellipse" | "line"))
                    .unwrap_or("rect")
                    .to_owned();
                items.push(SlideItem {
                    fill,
                    outline,
                    geometry,
                    rotation,
                    flip_h,
                    flip_v,
                    paragraphs,
                    kind: if img.is_some() { "image" } else { "shape" }.into(),
                    text,
                    image: img,
                    x: x / self.width * 100.0,
                    y: y / self.height * 100.0,
                    width: width / self.width * 100.0,
                    height: height / self.height * 100.0,
                });
            }
        }
        Ok((
            self.slides.len(),
            Slide {
                width: self.width,
                height: self.height,
                background,
                items,
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    fn fixture(parts: &[(&str, &str)], extension: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "shoruipoppu-test-{}-{}.{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
            extension
        ));
        let mut writer = zip::ZipWriter::new(File::create(&path).unwrap());
        for (name, contents) in parts {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(contents.as_bytes()).unwrap();
        }
        writer.finish().unwrap();
        path
    }
    #[test]
    fn reads_docx_paragraphs_and_tables() {
        let path = fixture(
            &[(
                "word/document.xml",
                r#"<w:document xmlns:w="urn:w"><w:body><w:p><w:r><w:rPr><w:b/></w:rPr><w:t>Hello</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#,
            )],
            "docx",
        );
        let blocks = docx(&path).unwrap().blocks;
        std::fs::remove_file(path).unwrap();
        assert!(blocks[0].children[0].style.as_ref().unwrap().bold);
        assert_eq!(blocks[0].children[0].text, "Hello");
        assert_eq!(
            blocks[1].children[0].children[0].children[0].children[0].text,
            "Cell"
        );
    }
    #[test]
    fn reads_word_page_dimensions_margins_and_explicit_breaks() {
        let path = fixture(
            &[(
                "word/document.xml",
                r#"<w:document xmlns:w="urn:w"><w:body><w:p><w:r><w:t>Before</w:t><w:br w:type="page"/><w:t>After</w:t></w:r></w:p><w:p><w:pPr><w:pageBreakBefore/></w:pPr><w:r><w:t>Next</w:t></w:r></w:p><w:sectPr><w:pgSz w:w="12000" w:h="15000"/><w:pgMar w:top="900" w:right="1200" w:bottom="1500" w:left="1800"/></w:sectPr></w:body></w:document>"#,
            )],
            "docx",
        );
        let doc = docx(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(
            (doc.width, doc.height, doc.margins),
            (800.0, 1000.0, [60.0, 80.0, 100.0, 120.0])
        );
        assert_eq!(doc.blocks[0].children[1].kind, "page_break");
        assert_eq!(doc.blocks[1].kind, "page_break");
    }
    #[test]
    fn reads_slide_order_and_placement() {
        let path = fixture(
            &[
                (
                    "ppt/presentation.xml",
                    r#"<p:presentation xmlns:p="urn:p" xmlns:r="urn:r"><p:sldIdLst><p:sldId id="256" r:id="rId1"/></p:sldIdLst><p:sldSz cx="1000" cy="500"/></p:presentation>"#,
                ),
                (
                    "ppt/_rels/presentation.xml.rels",
                    r#"<Relationships><Relationship Id="rId1" Target="slides/slide1.xml"/></Relationships>"#,
                ),
                (
                    "ppt/slides/slide1.xml",
                    r#"<p:sld xmlns:p="urn:p" xmlns:a="urn:a"><p:sp><p:spPr><a:xfrm><a:off x="100" y="50"/><a:ext cx="500" cy="250"/></a:xfrm><a:solidFill><a:srgbClr val="AABBCC"/></a:solidFill></p:spPr><p:txBody><a:p><a:r><a:t>Hello slide</a:t></a:r></a:p></p:txBody></p:sp></p:sld>"#,
                ),
            ],
            "pptx",
        );
        let mut presentation = Presentation::open(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        let (count, slide) = presentation.slide(0).unwrap();
        assert_eq!(count, 1);
        assert_eq!(slide.items[0].text, "Hello slide");
        assert_eq!(slide.items[0].x, 10.0);
        assert_eq!(slide.items[0].fill.as_deref(), Some("#AABBCC"));
    }
    #[test]
    fn inherits_layout_and_master_artwork_background_and_placeholder_position() {
        let path = fixture(
            &[
                (
                    "ppt/presentation.xml",
                    r#"<p:presentation xmlns:p="urn:p" xmlns:r="urn:r"><p:sldIdLst><p:sldId r:id="rId1"/><p:sldId r:id="rId2"/></p:sldIdLst><p:sldSz cx="1000" cy="500"/></p:presentation>"#,
                ),
                (
                    "ppt/_rels/presentation.xml.rels",
                    r#"<Relationships><Relationship Id="rId1" Target="slides/slide1.xml"/><Relationship Id="rId2" Target="slides/slide2.xml"/></Relationships>"#,
                ),
                (
                    "ppt/slides/_rels/slide1.xml.rels",
                    r#"<Relationships><Relationship Type="x/slideLayout" Target="../slideLayouts/slideLayout1.xml"/></Relationships>"#,
                ),
                (
                    "ppt/slides/_rels/slide2.xml.rels",
                    r#"<Relationships><Relationship Type="x/slideLayout" Target="../slideLayouts/slideLayout1.xml"/></Relationships>"#,
                ),
                (
                    "ppt/slides/slide2.xml",
                    r#"<p:sld xmlns:p="urn:p" xmlns:a="urn:a" showMasterSp="0"><p:cSld><p:sp><p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="100" cy="100"/></a:xfrm></p:spPr><p:txBody><a:p><a:r><a:t>Second slide</a:t></a:r></a:p></p:txBody></p:sp></p:cSld></p:sld>"#,
                ),
                (
                    "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
                    r#"<Relationships><Relationship Type="x/slideMaster" Target="../slideMasters/slideMaster1.xml"/></Relationships>"#,
                ),
                (
                    "ppt/slideMasters/slideMaster1.xml",
                    r#"<p:sldMaster xmlns:p="urn:p" xmlns:a="urn:a"><p:cSld><p:bg><p:bgPr><a:solidFill><a:srgbClr val="AABBCC"/></a:solidFill></p:bgPr></p:bg><p:sp><p:spPr><a:xfrm><a:off x="10" y="10"/><a:ext cx="40" cy="40"/></a:xfrm></p:spPr><p:txBody><a:p><a:r><a:t>Master artwork</a:t></a:r></a:p></p:txBody></p:sp></p:cSld></p:sldMaster>"#,
                ),
                (
                    "ppt/slideLayouts/slideLayout1.xml",
                    r#"<p:sldLayout xmlns:p="urn:p" xmlns:a="urn:a"><p:cSld><p:sp><p:nvSpPr><p:nvPr><p:ph idx="3"/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x="100" y="50"/><a:ext cx="500" cy="250"/></a:xfrm></p:spPr><p:txBody><a:p><a:r><a:t>Placeholder example</a:t></a:r></a:p></p:txBody></p:sp><p:sp><p:spPr><a:xfrm><a:off x="20" y="20"/><a:ext cx="30" cy="30"/></a:xfrm></p:spPr><p:txBody><a:p><a:r><a:t>Layout artwork</a:t></a:r></a:p></p:txBody></p:sp></p:cSld></p:sldLayout>"#,
                ),
                (
                    "ppt/slides/slide1.xml",
                    r#"<p:sld xmlns:p="urn:p" xmlns:a="urn:a"><p:cSld><p:sp><p:nvSpPr><p:nvPr><p:ph idx="3"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:p><a:r><a:t>Actual title</a:t></a:r></a:p></p:txBody></p:sp></p:cSld></p:sld>"#,
                ),
            ],
            "pptx",
        );
        let mut presentation = Presentation::open(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        let (_, slide) = presentation.slide(0).unwrap();
        assert_eq!(slide.background.as_deref(), Some("#AABBCC"));
        assert_eq!(slide.items.len(), 3);
        assert_eq!(slide.items[0].text, "Master artwork");
        let (_, second) = presentation.slide(1).unwrap();
        assert_eq!(second.items.len(), 2);
        assert_eq!(second.items[0].text, "Layout artwork");
        assert_eq!(second.items[1].text, "Second slide");
        assert_eq!(second.background.as_deref(), Some("#AABBCC"));
        assert_eq!(slide.items[1].text, "Layout artwork");
        assert_eq!(slide.items[2].text, "Actual title");
        assert_eq!(
            (
                slide.items[2].x,
                slide.items[2].y,
                slide.items[2].width,
                slide.items[2].height
            ),
            (10.0, 10.0, 50.0, 50.0)
        );
    }
    #[test]
    fn preserves_word_run_styles_breaks_and_paragraph_alignment() {
        let path = fixture(
            &[(
                "word/document.xml",
                r#"<w:document xmlns:w="urn:w"><w:body><w:p><w:pPr><w:jc w:val="center"/><w:numPr><w:ilvl w:val="2"/></w:numPr></w:pPr><w:r><w:rPr><w:b w:val="0"/><w:i/><w:u w:val="single"/><w:sz w:val="28"/><w:color w:val="AA11CC"/><w:rFonts w:ascii="Arial"/></w:rPr><w:t>Hello</w:t><w:tab/><w:t>World</w:t><w:br/></w:r></w:p></w:body></w:document>"#,
            )],
            "docx",
        );
        let blocks = docx(&path).unwrap().blocks;
        std::fs::remove_file(path).unwrap();
        let p = blocks[0].style.as_ref().unwrap();
        assert_eq!(p.align.as_deref(), Some("center"));
        assert_eq!(p.list_level, Some(2));
        let style = blocks[0].children[0].style.as_ref().unwrap();
        assert!(!style.bold && style.italic && style.underline);
        assert_eq!(style.size_pt, Some(14.0));
        assert_eq!(style.color.as_deref(), Some("#AA11CC"));
        assert_eq!(style.font.as_deref(), Some("Arial"));
        assert_eq!(
            blocks[0]
                .children
                .iter()
                .map(|b| b.text.as_str())
                .collect::<String>(),
            "Hello\tWorld\n"
        );
    }

    #[test]
    fn preserves_slide_text_format_and_group_positions() {
        let contents = r#"<p:grpSp xmlns:p="urn:p" xmlns:a="urn:a"><p:grpSpPr><a:xfrm><a:off x="100" y="200"/><a:ext cx="400" cy="600"/><a:chOff x="0" y="0"/><a:chExt cx="200" cy="300"/></a:xfrm></p:grpSpPr><p:sp><p:spPr><a:xfrm><a:off x="20" y="30"/><a:ext cx="50" cy="60"/></a:xfrm></p:spPr><p:txBody><a:p><a:pPr algn="ctr"><a:defRPr sz="1800" b="1"/></a:pPr><a:r><a:rPr b="1" i="1" u="sng" sz="2400"><a:solidFill><a:schemeClr val="accent1"/></a:solidFill></a:rPr><a:t>Styled</a:t></a:r></a:p></p:txBody></p:sp></p:grpSp>"#;
        let doc = Document::parse(contents).unwrap();
        let node = doc.descendants().find(|n| n.has_tag_name("sp")).unwrap();
        assert_eq!(coordinates(node), (140.0, 260.0, 100.0, 120.0));
        let theme = HashMap::from([("accent1".to_owned(), "#123456".to_owned())]);
        let paragraphs = slide_text(node, &theme);
        assert_eq!(paragraphs[0].align.as_deref(), Some("center"));
        let run = &paragraphs[0].runs[0];
        assert_eq!(run.text, "Styled");
        assert!(run.style.bold && run.style.italic && run.style.underline);
        let defaults = slide_style(
            None,
            slide_style(
                Some(
                    node.descendants()
                        .find(|n| n.has_tag_name("defRPr"))
                        .unwrap(),
                ),
                TextStyle::default(),
                &theme,
            ),
            &theme,
        );
        assert!(defaults.bold);
        assert_eq!(defaults.size_pt, Some(18.0));
        assert_eq!(run.style.size_pt, Some(24.0));
        assert_eq!(run.style.color.as_deref(), Some("#123456"));
        assert!(safe_font("Arial; color:red").is_none());
    }

    #[test]
    fn renders_word_numbering_and_slide_bullets() {
        let path = fixture(
            &[
                (
                    "word/document.xml",
                    r#"<w:document xmlns:w="urn:w"><w:body><w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="9"/></w:numPr></w:pPr><w:r><w:t>First</w:t></w:r></w:p><w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="9"/></w:numPr></w:pPr><w:r><w:t>Second</w:t></w:r></w:p></w:body></w:document>"#,
                ),
                (
                    "word/numbering.xml",
                    r#"<w:numbering xmlns:w="urn:w"><w:abstractNum w:abstractNumId="4"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/></w:lvl></w:abstractNum><w:num w:numId="9"><w:abstractNumId w:val="4"/></w:num></w:numbering>"#,
                ),
            ],
            "docx",
        );
        let blocks = docx(&path).unwrap().blocks;
        std::fs::remove_file(path).unwrap();
        assert_eq!(
            blocks[0].style.as_ref().unwrap().marker.as_deref(),
            Some("1.")
        );
        assert_eq!(
            blocks[1].style.as_ref().unwrap().marker.as_deref(),
            Some("2.")
        );
        let slide = r#"<p:sp xmlns:p="urn:p" xmlns:a="urn:a"><p:txBody><a:p><a:pPr lvl="1"><a:buChar char="•"/></a:pPr><a:r><a:t>Item</a:t></a:r></a:p></p:txBody></p:sp>"#;
        let doc = Document::parse(slide).unwrap();
        let p = slide_text(doc.root_element(), &HashMap::new());
        assert_eq!(p[0].marker.as_deref(), Some("•"));
        assert_eq!(p[0].level, 1);
    }

    #[test]
    fn reads_theme_colors_through_slide_relationships() {
        let path = fixture(
            &[
                (
                    "ppt/slides/_rels/slide1.xml.rels",
                    r#"<Relationships><Relationship Id="r1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/></Relationships>"#,
                ),
                (
                    "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
                    r#"<Relationships><Relationship Id="r1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/></Relationships>"#,
                ),
                (
                    "ppt/slideMasters/_rels/slideMaster1.xml.rels",
                    r#"<Relationships><Relationship Id="r1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/></Relationships>"#,
                ),
                (
                    "ppt/theme/theme1.xml",
                    r#"<a:theme xmlns:a="urn:a"><a:clrScheme><a:accent1><a:srgbClr val="123456"/></a:accent1><a:dk1><a:sysClr lastClr="111111"/></a:dk1></a:clrScheme></a:theme>"#,
                ),
            ],
            "pptx",
        );
        let mut zip = package(&path).unwrap();
        let theme = slide_theme(&mut zip, "ppt/slides/slide1.xml");
        drop(zip);
        std::fs::remove_file(path).unwrap();
        assert_eq!(theme.get("accent1").map(String::as_str), Some("#123456"));
        assert_eq!(theme.get("dk1").map(String::as_str), Some("#111111"));
    }

    #[test]
    fn rejects_relationships_outside_the_package() {
        assert_eq!(
            resolve("word/document.xml", "../media/picture.png").as_deref(),
            Some("media/picture.png")
        );
        assert!(resolve("word/document.xml", "../../secret").is_none());
    }
}
