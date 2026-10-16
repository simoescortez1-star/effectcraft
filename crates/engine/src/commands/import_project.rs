//! File ▸ Import ▸ EffectCraft Project: merge another `.ecproj` (or its XML copy) into the
//! current project, as After Effects imports an `.aep` into the open project. The imported
//! items land in a folder named after the file; every id is shifted past the current project's
//! so nothing collides, and the whole import is one undo step.

use effectcraft_color::Label;
use effectcraft_project::{ItemId, ItemKind, LayerSource, Project, essential};
use serde_json::{Value, json};

use super::{CommandSpec, always, bad, str_p};
use crate::{EngineError, Result, Session, cmd};

const C: &str = "file.importProject";

/// Whether `path` names a project EffectCraft imports as a project (not as footage).
pub(crate) fn is_project_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".ecproj") || lower.ends_with(".ecprojx") || lower.ends_with(".aep") || lower.ends_with(".aepx")
}

/// Read a project file (JSON or its XML copy), as File ▸ Open does.
fn read_project(s: &Session, path: &str) -> Result<(Project, Option<String>, Option<String>)> {
    if path.to_ascii_lowercase().ends_with(".aepx") {
        return Err(bad(C, "AEPX XML decoding is not supported by the configured AEP bridge; no project imported"));
    }
    if path.to_ascii_lowercase().ends_with(".aep") {
        let converter = s.ae_converter.as_ref().ok_or_else(|| bad(C,
            "AEP import requires an external converter. Build the pinned aep-reader adapter (docs/after-effects-compatibility.md) and set EFFECTCRAFT_AEP_CONVERTER to its absolute executable path before starting EffectCraft"))?;
        let bytes = s.services.read_file_bounded(path, effectcraft_ae_bridge::MAX_SOURCE_BYTES).map_err(|e| bad(C, format!("cannot read {path}: {e}")))?;
        let converted = converter.convert(&bytes).map_err(|e| bad(C, e))?;
        if converted.project_json.len() > effectcraft_ae_bridge::MAX_PROJECT_BYTES {
            return Err(bad(C, "converted AEP exceeds 128 MiB"));
        }
        effectcraft_ae_bridge::warnings(&converted.report_json).map_err(|e| bad(C, e))?;
        let (mut project, repairs) = Project::from_json_repaired(&converted.project_json).map_err(|e| bad(C, format!("invalid converted AEP: {e}")))?;
        let mut report: Value = serde_json::from_str(&converted.report_json).map_err(|e| bad(C, e.to_string()))?;
        let object = report.as_object_mut().ok_or_else(|| bad(C, "AEP conversion report is not an object"))?;
        object.insert("originalSource".into(), json!(path));
        if let Some(warnings) = report.get_mut("report").and_then(|r| r.get_mut("warnings")).and_then(Value::as_array_mut) {
            for repair in repairs {
                warnings.push(json!({"context": "EffectCraft validation", "message": repair}));
            }
        }
        let report = serde_json::to_string(&report).map_err(|e| bad(C, e.to_string()))?;
        effectcraft_ae_bridge::warnings(&report).map_err(|e| bad(C, e))?;
        // Expressions stay inspectable, but importing a foreign project cannot run code.
        for item in project.items.values_mut() {
            if let ItemKind::Comp(c) = &mut item.kind {
                for layer in &mut std::sync::Arc::make_mut(c).layers {
                    layer.props.walk_mut(&mut |p| {
                        if let Some(e) = &mut p.expr {
                            e.enabled = false;
                        }
                    });
                }
            }
        }
        return Ok((project, None, Some(report)));
    }
    let bytes = s.services.read_file(path).map_err(|e| EngineError::Other(format!("cannot read {path}: {e}")))?;
    let text = String::from_utf8(bytes).map_err(|_| bad(C, format!("{path} is not a text project file")))?;
    let text = if crate::xml_project::is_xml(&text) { crate::xml_project::from_xml(&text).map_err(EngineError::Other)? } else { text };
    let project = Project::from_json(&text).map_err(|e| bad(C, format!("{path} is not a project EffectCraft can open: {e}")))?;
    Ok((project, effectcraft_project::saved_by(&text), None))
}

/// Footage paths relative to the imported project's folder become absolute when the file is
/// there; returns the footage paths that are missing.
fn resolve_footage(s: &Session, project: &mut Project, dir: &std::path::Path) -> Vec<String> {
    let services = s.services.clone();
    let resolve = |raw: &mut String| -> bool {
        if raw.is_empty() || services.exists(raw) {
            return true;
        }
        if std::path::Path::new(raw.as_str()).is_absolute() {
            return false;
        }
        let joined = dir.join(raw.as_str()).to_string_lossy().to_string();
        if services.exists(&joined) {
            *raw = joined;
            true
        } else {
            false
        }
    };
    let mut missing = vec![];
    for it in project.items.values_mut() {
        if let ItemKind::Footage(f) = &mut it.kind {
            if f.path.is_empty() && f.missing {
                missing.push(format!("{} (missing source; use Replace Footage; original reference is in the import report)", it.name));
                continue;
            }
            if f.sequence.is_empty() {
                f.missing = !resolve(&mut f.path);
                if f.missing {
                    missing.push(f.path.clone());
                }
            } else {
                let mut all = true;
                for frame in &mut f.sequence {
                    all &= resolve(frame);
                }
                f.missing = !all;
                if !all {
                    missing.push(f.path.clone());
                }
            }
        }
    }
    missing
}

/// The comps no other comp of `project` nests: the ones to open or place.
fn root_comps(project: &Project) -> Vec<ItemId> {
    let nested: std::collections::HashSet<ItemId> =
        project.comps().flat_map(|(_, c)| c.layers.iter().filter_map(|l| if let LayerSource::Comp { item } = l.source { Some(item) } else { None })).collect();
    project.comps().map(|(id, _)| *id).filter(|id| !nested.contains(id)).collect()
}

fn import_project(s: &mut Session, p: &Value) -> Result<Value> {
    let path = str_p(p, "path").ok_or_else(|| bad(C, "missing `path` (.ecproj)"))?.to_string();
    let (mut sub, saved_by, conversion_report) = read_project(s, &path)?;
    let mut warnings = match conversion_report.as_deref() {
        Some(report) => effectcraft_ae_bridge::warnings(report).map_err(|e| bad(C, e))?,
        None => vec![],
    };
    if conversion_report.is_some() {
        warnings.push("Experimental AEP import: expressions disabled; unsupported data remains in the original AEP. The conversion report is preserved in the import folder comment.".into());
    }
    crate::upgrade_effects(&mut sub);
    let file = std::path::Path::new(&path);
    let dir = file.parent().map(|d| d.to_path_buf()).unwrap_or_default();
    let missing = resolve_footage(s, &mut sub, &dir);
    // The imported project's own render queue stays behind (as for templates).
    let base = s.project.next_id;
    if sub.next_id.checked_add(base).is_none() {
        return Err(bad(C, format!("{path} has ids out of range")));
    }
    essential::offset_ids(&mut sub, base);
    let roots: Vec<ItemId> = root_comps(&sub);
    let folder_name = file.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Imported Project".into());
    let (folder, items) = s.edit("Import Project", None, |proj, st| {
        let folder = proj.add_item(&folder_name, Label::Yellow, None, ItemKind::Folder);
        if let Some(report) = &conversion_report
            && let Some(item) = proj.item_mut(folder)
        {
            item.comment = report.clone();
        }
        let mut items = vec![];
        for (id, mut it) in std::mem::take(&mut sub.items) {
            if it.parent.is_none() {
                it.parent = Some(folder);
            }
            items.push(id.0);
            proj.items.insert(id, it);
        }
        proj.next_id = proj.next_id.max(sub.next_id);
        proj.fix_next_id();
        st.project_selection = vec![folder];
        Ok((folder, items))
    })?;
    let comps: Vec<Value> = s
        .project
        .comps()
        .filter(|(id, _)| items.contains(&id.0))
        .map(|(id, _)| json!({"id": id.0, "name": s.project.item(*id).map(|i| i.name.clone()).unwrap_or_default()}))
        .collect();
    let primary = if let [one] = roots.as_slice() { Some(one.0) } else { None };
    if let Some(v) = saved_by.as_deref().filter(|v| effectcraft_project::is_newer_version(v, effectcraft_project::APP_VERSION)) {
        warnings.push(format!("saved by EffectCraft {v}, newer than this version ({})", effectcraft_project::APP_VERSION));
    }
    if !missing.is_empty() {
        warnings.push(format!("{} footage file{} missing", missing.len(), if missing.len() == 1 { " is" } else { "s are" }));
    }
    s.toast(if warnings.is_empty() {
        format!("Imported {folder_name}")
    } else if conversion_report.is_some() {
        format!("Imported {folder_name}: {} warnings; inspect the import report", warnings.len())
    } else {
        format!("Imported {folder_name}: {}", warnings.join("; "))
    });
    if conversion_report.is_some() {
        s.events.push(crate::Event::Frontend {
            command: "app.showReport".into(),
            params: json!({"title": "After Effects Import Report", "text": warnings.join("\n"),"summary":format!("Imported {} composition(s). Missing media: {}.\n{}\n\nThe full conversion report is saved in the import folder comment.",comps.len(),missing.len(),effectcraft_ae_bridge::warning_summary(&warnings))}),
        });
    }
    Ok(json!({
        "folder": folder.0,
        "items": items,
        "comps": comps,
        "rootComps": roots.iter().map(|id| id.0).collect::<Vec<_>>(),
        "primaryComp": primary,
        "missingMedia": missing,
        "warnings": warnings,
        "conversionReport": conversion_report.and_then(|r| serde_json::from_str::<Value>(&r).ok()),
    }))
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "file.importAfterEffects",
            "After Effects Project...",
            ["File", "Import"],
            None,
            "{path (.aep)} — external conversion then atomic project merge; returns warnings and conversionReport",
            always,
            |s, p| {
                let path = str_p(p, "path").ok_or_else(|| bad("file.importAfterEffects", "missing path"))?;
                if !path.to_ascii_lowercase().ends_with(".aep") && !path.to_ascii_lowercase().ends_with(".aepx") {
                    return Err(bad("file.importAfterEffects", "expected an .aep or .aepx project"));
                }
                import_project(s, p)
            }
        ),
        cmd!(
            "file.importProject",
            "EffectCraft Project...",
            ["File", "Import"],
            None,
            "{path (.ecproj or .ecprojx)} — merges the project into a folder named after the file; returns {folder, items, comps, rootComps, primaryComp (the only root comp, else null), missingMedia, warnings}",
            always,
            import_project
        ),
    ]
}
