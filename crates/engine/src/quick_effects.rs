//! Quick Effects Console metadata and bounded search. Effects always come from the live registry.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Preferences {
    pub enabled: bool,
    pub max_results: u32,
    pub favorites_first: bool,
    pub show_recent: bool,
    pub animations: bool,
    pub opacity: f64,
    pub width: u32,
    pub preset_folder: String,
    pub screenshot_folder: String,
    pub screenshot_scale: f64,
    pub favorites: Vec<String>,
    pub recent: Vec<String>,
    pub usage: BTreeMap<String, u32>,
    pub aliases: BTreeMap<String, String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            enabled: true,
            max_results: 8,
            favorites_first: true,
            show_recent: true,
            animations: true,
            opacity: 0.96,
            width: 640,
            preset_folder: String::new(),
            screenshot_folder: String::new(),
            screenshot_scale: 1.0,
            favorites: vec![],
            recent: vec![],
            usage: BTreeMap::new(),
            aliases: BTreeMap::new(),
            extra: BTreeMap::new(),
        }
    }
}

impl Preferences {
    pub fn normalize(&mut self) {
        for path in [&mut self.preset_folder, &mut self.screenshot_folder] {
            if path.len() > 4096 || path.contains('\0') {
                path.clear();
            }
        }
        self.max_results = self.max_results.clamp(4, 30);
        self.width = self.width.clamp(360, 900);
        self.opacity = if self.opacity.is_finite() { self.opacity.clamp(0.75, 1.0) } else { 0.96 };
        self.screenshot_scale = if self.screenshot_scale.is_finite() { self.screenshot_scale.clamp(0.25, 1.0) } else { 1.0 };
        let valid = |s: &str| !s.is_empty() && s.len() <= 4096 && !s.contains('\0');
        for (list, limit) in [(&mut self.favorites, 256), (&mut self.recent, 40)] {
            let mut seen = std::collections::BTreeSet::new();
            list.retain(|key| valid(key) && seen.insert(key.clone()));
            list.truncate(limit);
        }
        self.aliases.retain(|key, value| valid(key) && value.len() <= 512 && !value.contains('\0'));
        self.aliases = std::mem::take(&mut self.aliases).into_iter().take(256).collect();
        self.usage.retain(|key, _| valid(key));
        self.usage = std::mem::take(&mut self.usage).into_iter().take(512).collect();
    }

    pub fn record(&mut self, key: &str) {
        self.recent.retain(|entry| entry != key);
        self.recent.insert(0, key.into());
        if self.usage.len() >= 512
            && !self.usage.contains_key(key)
            && let Some(oldest) = self.usage.iter().filter(|(key, _)| !self.recent.contains(key)).min_by_key(|(_, count)| *count).map(|(key, _)| key.clone())
        {
            self.usage.remove(&oldest);
        }
        let count = self.usage.entry(key.into()).or_default();
        *count = count.saturating_add(1);
        self.normalize();
    }

    pub fn favorite(&mut self, key: &str) -> bool {
        let was = self.favorites.iter().any(|entry| entry == key);
        self.favorites.retain(|entry| entry != key);
        if !was {
            if self.favorites.len() >= 256 {
                return false;
            }
            self.favorites.push(key.into());
        }
        self.normalize();
        !was
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Effect,
    TextPreset,
    BuiltinPreset,
    NativePreset,
    FfxPreset,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub key: String,
    pub id: String,
    pub name: String,
    pub category: String,
    pub kind: Kind,
    pub gpu: bool,
}

impl Entry {
    pub fn command(&self) -> (&'static str, Value) {
        match self.kind {
            Kind::Effect => ("effect.apply", json!({"effect":self.id})),
            Kind::TextPreset => ("layer.applyTextPreset", json!({"preset":self.id})),
            Kind::BuiltinPreset => ("anim.applyBuiltinPreset", json!({"preset":self.id})),
            Kind::NativePreset => ("anim.applyPreset", json!({"path":self.id})),
            Kind::FfxPreset => ("anim.applyAfterEffectsPreset", json!({"path":self.id})),
        }
    }

    pub fn file(path: &str) -> Option<Self> {
        let file = std::path::Path::new(path);
        let kind = match file.extension()?.to_str()?.to_ascii_lowercase().as_str() {
            "ffx" => Kind::FfxPreset,
            "ecpreset" => Kind::NativePreset,
            _ => return None,
        };
        if path.len() > 4090 {
            return None;
        }
        Some(Self {
            key: format!("file:{path}"),
            id: path.into(),
            name: file.file_stem()?.to_string_lossy().into_owned(),
            category: if kind == Kind::FfxPreset { "FFX · experimental".into() } else { "User Presets".into() },
            kind,
            gpu: false,
        })
    }
}

/// Rebuilt when the registered ids change; no second list of effect definitions.
pub fn catalog() -> Vec<Entry> {
    let mut entries: Vec<_> = effectcraft_effects::all()
        .into_iter()
        .map(|effect| Entry {
            key: format!("effect:{}", effect.id),
            id: effect.id.into(),
            name: effect.name.into(),
            category: effect.category.into(),
            kind: Kind::Effect,
            gpu: effect.gpu,
        })
        .collect();
    entries.extend(crate::text_presets().into_iter().map(|(id, name)| Entry {
        key: format!("text:{id}"),
        id,
        name,
        category: "Text Presets".into(),
        kind: Kind::TextPreset,
        gpu: false,
    }));
    entries.extend(crate::animation_presets().into_iter().map(|(id, name, category)| Entry {
        key: format!("builtin:{id}"),
        id,
        name,
        category,
        kind: Kind::BuiltinPreset,
        gpu: false,
    }));
    entries
}

/// Accent-insensitive, Unicode-safe tokens; punctuation and repeated whitespace are equivalent.
pub fn fold(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            c if c.is_alphanumeric() => c,
            _ => ' ',
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn near(a: &str, b: &str) -> bool {
    let a: Vec<_> = a.chars().take(64).collect();
    let b: Vec<_> = b.chars().take(64).collect();
    if a.len() < 4 || a.len().abs_diff(b.len()) > 1 {
        return false;
    }
    let (mut i, mut j, mut edits) = (0_usize, 0_usize, 0);
    while let (Some(ac), Some(bc)) = (a.get(i), b.get(j)) {
        if ac == bc {
            i += 1;
            j += 1;
            continue;
        }
        edits += 1;
        if edits > 1 {
            return false;
        }
        match a.len().cmp(&b.len()) {
            std::cmp::Ordering::Less => j += 1,
            std::cmp::Ordering::Greater => i += 1,
            std::cmp::Ordering::Equal => {
                i += 1;
                j += 1;
            }
        }
    }
    edits + a.len().saturating_sub(i) + b.len().saturating_sub(j) <= 1
}

/// `names` may include localized display names; stable ids remain searchable.
pub fn search(entries: &[Entry], names: &[String], query: &str, prefs: &Preferences, filter: &str) -> Vec<usize> {
    let query = fold(&query.chars().take(128).collect::<String>());
    let compact_query = query.replace(' ', "");
    let mut ranked = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let favorite = prefs.favorites.contains(&entry.key);
        if (filter == "favorites" && !favorite)
            || (filter == "effects" && entry.kind != Kind::Effect)
            || (filter == "presets" && entry.kind == Kind::Effect)
            || (filter == "recent" && !prefs.recent.contains(&entry.key))
        {
            continue;
        }
        let name = fold(&entry.name);
        let local = fold(names.get(index).map_or(&entry.name, String::as_str));
        let alias = fold(prefs.aliases.get(&entry.key).map_or("", String::as_str));
        let hay = fold(&format!("{name} {local} {} {} {alias}", entry.category, entry.id));
        let mut score = if query.is_empty() {
            0
        } else if name == query || local == query || alias == query {
            10_000
        } else if name.replace(' ', "") == compact_query || local.replace(' ', "") == compact_query {
            9_800
        } else if name.starts_with(&query)
            || local.starts_with(&query)
            || name.split_whitespace().any(|word| word == query)
            || local.split_whitespace().any(|word| word == query)
        {
            8_000
        } else if hay.contains(&query) {
            6_000
        } else {
            let words: Vec<_> = hay.split_whitespace().collect();
            if !query.split_whitespace().all(|token| words.iter().any(|word| word.starts_with(token) || near(token, word))) {
                continue;
            }
            3_000
        };
        if !query.is_empty() && entry.kind == Kind::Effect {
            score += 100;
        }
        if favorite && prefs.favorites_first {
            score += if query.is_empty() { 20_000 } else { 400 };
        }
        if prefs.show_recent
            && let Some(position) = prefs.recent.iter().position(|key| key == &entry.key)
        {
            score += 200_i64.saturating_sub(position as i64 * 4);
        }
        score += i64::from(prefs.usage.get(&entry.key).copied().unwrap_or(0).min(100));
        ranked.push((index, score, name));
    }
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.2.cmp(&b.2)).then(a.0.cmp(&b.0)));
    ranked.into_iter().take(prefs.max_results.clamp(4, 30) as usize).map(|x| x.0).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn searches_live_registry_ids_categories_aliases_typos_and_ranks_exact_names() {
        let entries = catalog();
        let names: Vec<_> = entries.iter().map(|e| e.name.clone()).collect();
        let mut prefs = Preferences::default();
        for query in ["warp", "  MOTION   TILE ", "motiontile", "ec.distort.warp", "gausian blur"] {
            assert!(!search(&entries, &names, query, &prefs, "all").is_empty(), "{query}");
        }
        assert!(search(&entries, &names, "an effect that does not exist", &prefs, "all").is_empty());
        assert_eq!(entries[search(&entries, &names, "blur", &prefs, "all")[0]].kind, Kind::Effect, "a native blur ranks ahead of text-only presets");
        let warp = entries.iter().find(|e| e.id == "ec.distort.warp").unwrap();
        prefs.aliases.insert(warp.key.clone(), "deformação favorita".into());
        assert_eq!(entries[search(&entries, &names, "deformacao", &prefs, "all")[0]].key, warp.key);
        prefs.record(&warp.key);
        prefs.favorite(&warp.key);
        assert_eq!(entries[search(&entries, &names, "", &prefs, "favorites")[0]].key, warp.key);
        assert_eq!(entries[search(&entries, &names, "", &prefs, "recent")[0]].key, warp.key);
        assert!(search(&entries, &names, "warp", &prefs, "presets").is_empty());
    }
    #[test]
    fn preferences_round_trip_bound_history_and_keep_unknown_fields() {
        let mut p = Preferences::default();
        p.extra.insert("futureOption".into(), json!(true));
        p.record("effect:ec.blur.gaussian");
        p.record("effect:ec.blur.gaussian");
        p.favorite("effect:ec.blur.gaussian");
        let restored: Preferences = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(restored, p);
        assert_eq!(p.usage["effect:ec.blur.gaussian"], 2);
        assert_eq!(p.recent.len(), 1);
        for n in 0..100 {
            p.record(&format!("effect:{n}"));
        }
        assert_eq!(p.recent.len(), 40);
        assert!(!p.favorite("effect:ec.blur.gaussian"));
    }
}
