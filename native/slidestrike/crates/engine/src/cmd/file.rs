//! File menu: new, open, save, export, close, properties.

use deckcraft_model::{Presentation, defaults};
use serde_json::{Value, json};

use super::*;
use crate::{DocState, EngineError, Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(noundo "file.new", "New Presentation", ["File"], Some("Cmd+N"), "{theme?: name, size?: [w,h] pt, blank?: bool}", always, new),
        cmd!(noundo "file.open", "Open…", ["File"], Some("Cmd+O"), "{path}", always, open),
        cmd!(noundo "file.importLegacy", "Import Older Strike Deck", ["File"], None, "{path: .sstrike, allowFontSubstitution?: bool}; uses the pinned OFL Source Serif 4 original, then an exact installed face if needed; refuses a missing font unless substitution is explicit; opens an unsaved deck and requires Save As", always, import_legacy),
        cmd!(noundo "file.openBytes", "Open Data", [], None, "{name, data: base64}", always, open_bytes),
        cmd!(noundo "file.save", "Save", ["File"], Some("Cmd+S"), "{path?, format?: deckcraft|pptx}", has_doc, save),
        cmd!(noundo "file.saveAs", "Save As…", ["File"], Some("Cmd+Shift+S"), "{path, format?}", has_doc, save_as),
        cmd!(noundo "file.saveTemplate", "Save as Template…", ["File"], None, "{path}", has_doc, save_as),
        cmd!(query "file.saveBytes", "Save to Data", [], None, "{format?: deckcraft|pptx} → {data: base64}", has_doc, save_bytes_cmd),
        cmd!(noundo "file.export", "Export…", ["File"], None, "{path, format: png|jpeg|pptx|deckcraft|outline|pdf, slide?: index, all?: bool, scale?: px per pt; pdf: layout?: slides|notes|handouts, perPage?: 1|2|3|4|6|9, dpi?, slides?: [index], includeHidden?, textLayer?, frame?}", has_doc, export),
        cmd!(query "file.render", "Render Slide", [], None, "{slide?: index, scale?, edit?: bool} → {png: base64, width, height}", has_doc, render),
        cmd!(noundo "file.close", "Close", ["File"], Some("Cmd+W"), "{}", has_doc, close),
        cmd!(
            "file.properties",
            "Properties",
            ["File"],
            None,
            "{title?, subject?, author?, keywords?, comments?, category?, company?} → properties",
            has_doc,
            properties
        ),
        cmd!(noundo "file.recovery.save", "Save AutoRecover Information", [], None, "{} → {written}", has_doc, recovery_save),
        cmd!(query "file.recovery.list", "Recovered Presentations", [], None, "{} → [{uid, path, title, saved}]", always, recovery_list),
        cmd!(noundo "file.recovery.open", "Open Recovered Presentations", [], None, "{} → {opened: [document index]}", always, recovery_open),
        cmd!(noundo "file.recovery.discard", "Discard Recovered Presentations", [], None, "{uid?: one entry, else all}", always, recovery_discard),
        cmd!(noundo "file.revert", "Revert", [], None, "{}", has_doc, revert),
        cmd!(noundo "window.next", "Next Window", ["Window"], Some("Cmd+`"), "{index?}", has_doc, next_window),
    ]
}

fn theme_named(name: &str) -> Option<deckcraft_model::Theme> {
    deckcraft_model::theme::builtin_themes()
        .into_iter()
        .find(|t| t.name.eq_ignore_ascii_case(name))
}

fn new(s: &mut Session, p: &Value) -> Result<Value> {
    let theme = str_param(p, "theme")
        .and_then(theme_named)
        .unwrap_or_default();
    let size = p
        .get("size")
        .and_then(Value::as_array)
        .and_then(|a| {
            Some(deckcraft_geom::Size::new(
                a.first()?.as_f64()?,
                a.get(1)?.as_f64()?,
            ))
        })
        .filter(|s| s.width >= 1.0 && s.height >= 1.0 && s.width < 5000.0 && s.height < 5000.0)
        .unwrap_or(defaults::WIDE);
    let doc = defaults::blank_presentation(size, theme, !bool_or(p, "blank", false));
    let title = s.next_untitled();
    let i = s.add_document(DocState::new(doc, None, title));
    Ok(json!({"document": i}))
}

/// Recognise and read presentation bytes (native, PPTX).
pub fn open_presentation(name: &str, bytes: &[u8]) -> Result<Presentation> {
    if deckcraft_format::sniff(bytes) {
        return deckcraft_format::load(bytes).map_err(|e| EngineError::Other(e.to_string()));
    }
    if deckcraft_pptx::sniff(bytes) {
        let mut p = deckcraft_pptx::import(bytes).map_err(|e| EngineError::Other(e.to_string()))?;
        deckcraft_format::repair(&mut p);
        return Ok(p);
    }
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".txt") || lower.ends_with(".md") {
        let text = String::from_utf8_lossy(bytes);
        let mut p = defaults::blank_presentation(defaults::WIDE, Default::default(), false);
        deckcraft_format::outline_to_slides(&mut p, &text);
        if p.slides.is_empty() {
            p = defaults::new_presentation(None);
        }
        return Ok(p);
    }
    Err(EngineError::Other(format!(
        "{name}: not a presentation SlideStrike can read"
    )))
}

pub fn add_opened(s: &mut Session, name: &str, path: Option<String>, p: Presentation) -> usize {
    let title = std::path::Path::new(name)
        .file_stem()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| name.to_string());
    s.add_document(DocState::new(p, path, title))
}

fn import_legacy(s: &mut Session, p: &Value) -> Result<Value> {
    let path = str_param(p, "path").ok_or_else(|| bad("file.importLegacy", "missing `path`"))?;
    let source = std::path::Path::new(path);
    if !source
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("sstrike"))
    {
        return Err(bad("file.importLegacy", "choose a .sstrike file"));
    }
    let allow_font_substitution =
        p.get("allowFontSubstitution").and_then(Value::as_bool) == Some(true);
    let converted = crate::legacy::convert_path(source, allow_font_substitution)?;
    let doc = converted.doc;
    let slides = doc.slides.len();
    let title_inferred = doc.slides.iter().any(|slide| !slide.title().is_empty());
    let (font_resolution, font_substitution) = match converted.font {
        crate::legacy::LegacyFont::NoText => (Value::Null, Value::Null),
        crate::legacy::LegacyFont::VerifiedBundledOriginal => (
            json!({"requested": "Source Serif 4", "resolved": "Source Serif 4", "source": "pinned-ofl-original", "portable": true}),
            Value::Null,
        ),
        crate::legacy::LegacyFont::LocalOriginal => (
            json!({"requested": "Source Serif 4", "resolved": "Source Serif 4", "source": "local-font-database", "portable": false}),
            Value::Null,
        ),
        crate::legacy::LegacyFont::Substituted => (
            json!({"requested": "Source Serif 4", "resolved": "Ubuntu", "source": "bundled", "portable": true}),
            json!({"from": "Source Serif 4", "requested": "Ubuntu", "exact": false}),
        ),
    };
    let index = add_opened(s, path, None, doc);
    Ok(
        json!({"document": index, "source": path, "slides": slides, "saveAsRequired": true, "historyReset": true, "identifiersRegenerated": true, "titleInferred": title_inferred, "fontResolution": font_resolution, "fontSubstitution": font_substitution}),
    )
}

#[cfg(not(target_arch = "wasm32"))]
fn open(s: &mut Session, p: &Value) -> Result<Value> {
    let path = str_param(p, "path").ok_or_else(|| bad("file.open", "missing `path`"))?;
    let bytes = std::fs::read(path).map_err(|e| EngineError::Other(format!("{path}: {e}")))?;
    let doc = open_presentation(path, &bytes)?;
    let native = deckcraft_format::sniff(&bytes) || deckcraft_pptx::sniff(&bytes);
    let i = add_opened(s, path, native.then(|| path.to_string()), doc);
    Ok(json!({"document": i, "slides": s.doc()?.doc.slides.len()}))
}
#[cfg(target_arch = "wasm32")]
fn open(_s: &mut Session, _p: &Value) -> Result<Value> {
    Err(EngineError::Other(
        "use file.openBytes in the browser".into(),
    ))
}

fn open_bytes(s: &mut Session, p: &Value) -> Result<Value> {
    let name = str_param(p, "name").unwrap_or("Presentation");
    let data = str_param(p, "data")
        .and_then(base64_decode)
        .ok_or_else(|| bad("file.openBytes", "missing or invalid base64 `data`"))?;
    let doc = open_presentation(name, &data)?;
    let i = add_opened(s, name, None, doc);
    Ok(json!({"document": i}))
}

/// Encode the presentation in `format` (`deckcraft`, `pptx`, `outline`).
pub fn save_bytes(doc: &Presentation, format: &str) -> Result<Vec<u8>> {
    match format {
        "pptx" | "potx" | "ppsx" => {
            deckcraft_pptx::export(doc).map_err(|e| EngineError::Other(e.to_string()))
        }
        "outline" | "txt" => Ok(deckcraft_format::slides_to_outline(doc).into_bytes()),
        "pdf" => pdf_export_checked(doc, &Default::default()),
        _ => deckcraft_format::save(doc).map_err(|e| EngineError::Other(e.to_string())),
    }
}

/// PDF options from command params: `{layout: slides|notes|handouts, perPage, dpi, slides: [i],
/// includeHidden, textLayer, frame}`.
pub fn pdf_options(p: &Value) -> deckcraft_pdf::PdfOptions {
    let mut o = deckcraft_pdf::PdfOptions {
        layout: deckcraft_pdf::PageLayout::Slides,
        ..Default::default()
    };
    o.layout = match str_param(p, "layout").unwrap_or("slides") {
        "notes" => deckcraft_pdf::PageLayout::Notes,
        "handouts" => deckcraft_pdf::PageLayout::Handouts {
            per_page: usize_param(p, "perPage").unwrap_or(6).min(9) as u8,
        },
        _ => deckcraft_pdf::PageLayout::Slides,
    };
    o.dpi = f64_or(p, "dpi", o.dpi).clamp(36.0, 600.0);
    o.slides = p.get("slides").and_then(Value::as_array).map(|a| {
        a.iter()
            .filter_map(|v| v.as_u64().map(|x| x as usize))
            .collect()
    });
    o.include_hidden = bool_or(p, "includeHidden", false);
    o.text_layer = bool_or(p, "textLayer", true);
    o.frame_slides = bool_or(p, "frame", true);
    o
}

/// PDF bytes for `doc` with [`pdf_options`] from `p`.
pub fn pdf_bytes(doc: &Presentation, p: &Value) -> Result<Vec<u8>> {
    pdf_export_checked(doc, &pdf_options(p))
}

/// The PDF text layer embeds font subsets. The pinned Source Serif 4 original is OFL-1.1 and
/// permits this; an arbitrary installed font with the same family name has unknown provenance.
fn pdf_export_checked(doc: &Presentation, opts: &deckcraft_pdf::PdfOptions) -> Result<Vec<u8>> {
    if opts.text_layer
        && doc_uses_source_serif(doc)
        && !deckcraft_fonts::FontDb::global().verified_source_serif_4()
    {
        return Err(EngineError::Other(
            "PDF text layer would embed an unverified Source Serif 4 font. Export with textLayer=false for a raster-only PDF without searchable text".into(),
        ));
    }
    deckcraft_pdf::export(doc, opts).map_err(|e| EngineError::Other(e.to_string()))
}

fn doc_uses_source_serif(doc: &Presentation) -> bool {
    doc.slides
        .iter()
        .flat_map(|slide| &slide.shapes)
        .filter_map(|shape| shape.text.as_ref())
        .flat_map(|body| &body.paragraphs)
        .flat_map(|paragraph| &paragraph.runs)
        .any(|run| {
            run.props
                .font
                .as_deref()
                .is_some_and(|font| font.eq_ignore_ascii_case("Source Serif 4"))
        })
}

pub fn format_for_path(path: &str) -> &'static str {
    let l = path.to_ascii_lowercase();
    if l.ends_with(".pptx") || l.ends_with(".potx") || l.ends_with(".ppsx") {
        "pptx"
    } else if l.ends_with(".png") {
        "png"
    } else if l.ends_with(".jpg") || l.ends_with(".jpeg") {
        "jpeg"
    } else if l.ends_with(".pdf") {
        "pdf"
    } else if l.ends_with(".txt") || l.ends_with(".md") {
        "outline"
    } else {
        "deckcraft"
    }
}

fn mark_saved(s: &mut Session, path: Option<String>) -> Result<()> {
    let st = s.doc_mut()?;
    st.saved_doc = st.doc.clone();
    if path.is_some() {
        st.path = path;
    }
    st.revision += 1;
    let uid = st.uid;
    if let Some(dir) = &s.recovery_dir {
        crate::recovery::discard(dir, uid);
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn write_file(path: &str, bytes: &[u8]) -> Result<()> {
    // A legacy deck must remain readable by the original SlideStrike editor. Native and
    // exported formats must never replace it, even when a caller supplies a format override.
    if std::path::Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("sstrike"))
    {
        return Err(EngineError::Other(
            "choose a new .deckcraft or .pptx path; .sstrike is reserved for older Strike decks"
                .into(),
        ));
    }
    // Write to a temporary sibling then rename, so a failed save never truncates the old file.
    let tmp = format!("{path}.saving");
    std::fs::write(&tmp, bytes).map_err(|e| EngineError::Other(format!("{path}: {e}")))?;
    std::fs::rename(&tmp, path).map_err(|e| EngineError::Other(format!("{path}: {e}")))
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod legacy_destination_tests {
    use super::write_file;

    #[test]
    fn native_save_cannot_replace_an_older_deck_even_with_a_format_override() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("slidestrike-original-{unique}.SSTRIKE"));
        std::fs::write(&path, b"original deck bytes").unwrap();
        let error = write_file(path.to_str().unwrap(), b"native bytes")
            .unwrap_err()
            .to_string();
        assert!(error.contains(".sstrike is reserved"));
        assert_eq!(std::fs::read(&path).unwrap(), b"original deck bytes");
        std::fs::remove_file(path).unwrap();
    }
}
#[cfg(target_arch = "wasm32")]
fn write_file(_path: &str, _bytes: &[u8]) -> Result<()> {
    Err(EngineError::Other(
        "saving to a path is not available in the browser".into(),
    ))
}

fn save(s: &mut Session, p: &Value) -> Result<Value> {
    let path = str_param(p, "path")
        .map(String::from)
        .or_else(|| s.active().and_then(|d| d.path.clone()));
    let Some(path) = path else {
        s.ui_requests.push(crate::UiRequest::PickFile {
            purpose: "saveAs".into(),
        });
        return Ok(json!({"needsPath": true}));
    };
    let format = str_param(p, "format").unwrap_or(format_for_path(&path));
    let bytes = save_bytes(&s.doc()?.doc, format)?;
    write_file(&path, &bytes)?;
    mark_saved(s, Some(path.clone()))?;
    Ok(json!({"path": path, "bytes": bytes.len()}))
}

fn save_as(s: &mut Session, p: &Value) -> Result<Value> {
    if str_param(p, "path").is_none() {
        s.ui_requests.push(crate::UiRequest::PickFile {
            purpose: "saveAs".into(),
        });
        return Ok(json!({"needsPath": true}));
    }
    save(s, p)
}

fn save_bytes_cmd(s: &mut Session, p: &Value) -> Result<Value> {
    let format = str_param(p, "format").unwrap_or("deckcraft");
    let bytes = save_bytes(&s.doc()?.doc, format)?;
    Ok(json!({"data": base64_encode(&bytes), "bytes": bytes.len()}))
}

/// Render slide `index` (current when absent) to PNG bytes.
pub fn render_png(doc: &Presentation, index: usize, scale: f64, edit: bool) -> (Vec<u8>, u32, u32) {
    let img = deckcraft_render::render_slide(
        doc,
        index,
        &deckcraft_render::RenderOpts {
            scale: scale.clamp(0.01, 16.0),
            edit,
            ..Default::default()
        },
    );
    (img.to_png(), img.width, img.height)
}

fn render(s: &mut Session, p: &Value) -> Result<Value> {
    let st = s.doc()?;
    let i = usize_param(p, "slide").unwrap_or(st.selection.slide);
    if i >= st.doc.slides.len() {
        return Err(bad("file.render", format!("no slide {i}")));
    }
    let (png, w, h) = render_png(
        &st.doc,
        i,
        f64_or(p, "scale", 1.0),
        bool_or(p, "edit", false),
    );
    Ok(json!({"png": base64_encode(&png), "width": w, "height": h}))
}

fn export(s: &mut Session, p: &Value) -> Result<Value> {
    let path = str_param(p, "path")
        .ok_or_else(|| bad("file.export", "missing `path`"))?
        .to_string();
    let format = str_param(p, "format")
        .unwrap_or(format_for_path(&path))
        .to_string();
    let st = s.doc()?;
    let scale = f64_or(p, "scale", 2.0);
    match format.as_str() {
        "png" | "jpeg" | "jpg" => {
            let slides: Vec<usize> = match usize_param(p, "slide") {
                Some(i) => vec![i],
                None if bool_or(p, "all", false) => (0..st.doc.slides.len()).collect(),
                None => vec![st.selection.slide],
            };
            let mut written = vec![];
            for (k, i) in slides.iter().enumerate() {
                if *i >= st.doc.slides.len() {
                    return Err(bad("file.export", format!("no slide {i}")));
                }
                let img = deckcraft_render::render_slide(
                    &st.doc,
                    *i,
                    &deckcraft_render::RenderOpts {
                        scale: scale.clamp(0.01, 16.0),
                        ..Default::default()
                    },
                );
                let bytes = if format == "png" {
                    img.to_png()
                } else {
                    img.to_jpeg(92)
                };
                let out = if slides.len() == 1 {
                    path.clone()
                } else {
                    let pth = std::path::Path::new(&path);
                    let stem = pth
                        .file_stem()
                        .map(|x| x.to_string_lossy().to_string())
                        .unwrap_or_else(|| "Slide".into());
                    let ext = pth
                        .extension()
                        .map(|x| x.to_string_lossy().to_string())
                        .unwrap_or_else(|| format.clone());
                    pth.with_file_name(format!("{stem}{}.{ext}", k + 1))
                        .to_string_lossy()
                        .to_string()
                };
                write_file(&out, &bytes)?;
                written.push(out);
            }
            Ok(json!({"files": written}))
        }
        "pdf" => {
            let bytes = pdf_bytes(&st.doc, p)?;
            write_file(&path, &bytes)?;
            Ok(json!({"path": path, "bytes": bytes.len()}))
        }
        other => {
            let bytes = save_bytes(&st.doc, other)?;
            write_file(&path, &bytes)?;
            Ok(json!({"path": path, "bytes": bytes.len()}))
        }
    }
}

fn close(s: &mut Session, _p: &Value) -> Result<Value> {
    if let Some(i) = s.active_index() {
        s.close_document(i);
    }
    ok()
}

fn properties(s: &mut Session, p: &Value) -> Result<Value> {
    let fields = [
        "title", "subject", "author", "keywords", "comments", "category", "company",
    ];
    if fields.iter().any(|f| p.get(*f).is_some()) {
        s.edit(|doc, _| {
            let pr = &mut doc.props;
            for f in fields {
                if let Some(v) = str_param(p, f) {
                    let slot = match f {
                        "title" => &mut pr.title,
                        "subject" => &mut pr.subject,
                        "author" => &mut pr.author,
                        "keywords" => &mut pr.keywords,
                        "comments" => &mut pr.comments,
                        "category" => &mut pr.category,
                        _ => &mut pr.company,
                    };
                    *slot = v.to_string();
                }
            }
            Ok(())
        })?;
    }
    Ok(serde_json::to_value(&s.doc()?.doc.props).unwrap_or_default())
}

fn revert(s: &mut Session, _p: &Value) -> Result<Value> {
    let st = s.doc_mut()?;
    st.doc = st.saved_doc.clone();
    st.history = crate::History {
        limit: st.history.limit,
        ..Default::default()
    };
    st.revision += 1;
    ok()
}

fn next_window(s: &mut Session, p: &Value) -> Result<Value> {
    let n = s.documents().len();
    if n == 0 {
        return ok();
    }
    let i = usize_param(p, "index").unwrap_or_else(|| (s.active_index().unwrap_or(0) + 1) % n);
    s.set_active(i.min(n - 1));
    ok()
}

fn recovery_dir(s: &Session) -> Result<std::path::PathBuf> {
    s.recovery_dir
        .clone()
        .ok_or_else(|| EngineError::Other("no recovery folder is set".into()))
}

fn recovery_save(s: &mut Session, _p: &Value) -> Result<Value> {
    let dir = recovery_dir(s)?;
    Ok(json!({"written": crate::recovery::save(s, &dir)?}))
}

fn recovery_list(s: &mut Session, _p: &Value) -> Result<Value> {
    let dir = recovery_dir(s)?;
    Ok(Value::Array(
        crate::recovery::list(&dir)
            .into_iter()
            .map(|(uid, mut m)| {
                if let Some(o) = m.as_object_mut() {
                    o.insert("uid".into(), json!(uid));
                }
                m
            })
            .collect(),
    ))
}

fn recovery_open(s: &mut Session, _p: &Value) -> Result<Value> {
    let dir = recovery_dir(s)?;
    Ok(json!({"opened": crate::recovery::open(s, &dir)?}))
}

fn recovery_discard(s: &mut Session, p: &Value) -> Result<Value> {
    let dir = recovery_dir(s)?;
    match p.get("uid").and_then(Value::as_u64) {
        Some(uid) => crate::recovery::discard(&dir, uid),
        None => {
            for (uid, _) in crate::recovery::list(&dir) {
                crate::recovery::discard(&dir, uid);
            }
        }
    }
    ok()
}

#[cfg(test)]
mod local_font_pdf_tests {
    use super::*;

    #[test]
    fn verified_ofl_source_serif_allows_searchable_pdf() {
        let mut doc = defaults::blank_presentation(defaults::WIDE, Default::default(), false);
        let layout = defaults::layout_of_kind(&doc, deckcraft_model::LayoutType::Title).unwrap();
        let mut slide = defaults::new_slide(&mut doc, layout);
        slide.shapes.clear();
        let mut body = deckcraft_model::TextBody::from_text("Local font");
        body.paragraphs[0].runs[0].props.font = Some("Source Serif 4".into());
        slide.shapes.push(deckcraft_model::Shape {
            id: deckcraft_model::ShapeId(doc.alloc_id()),
            xfrm: Some(deckcraft_geom::Xfrm::new(50.0, 50.0, 400.0, 100.0)),
            text_box: true,
            text: Some(body),
            ..Default::default()
        });
        doc.slides.push(std::sync::Arc::new(slide));
        assert!(deckcraft_fonts::FontDb::global().verified_source_serif_4());
        let searchable = pdf_export_checked(&doc, &Default::default()).unwrap();
        assert!(searchable.starts_with(b"%PDF"));
        let pdf = pdf_export_checked(
            &doc,
            &deckcraft_pdf::PdfOptions {
                text_layer: false,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(pdf.starts_with(b"%PDF"));
    }
}
