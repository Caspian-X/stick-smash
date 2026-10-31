//! The editor's model, independent of Bevy and egui: the content being edited, per-fighter documents with undo,
//! validation through the real loader, and saving that keeps `extends` intact (see docs/DESIGN.md "Editor").

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::Value;
use sim::GameData;
use sim::data::fighter::Fighter;
use sim::data::json::{merge, to_compact_string};
use sim::data::{ContentError, Source, load};

/// Every content file the game loaded, by path relative to a content root (`fighters/<id>/fighter.json`). User
/// files have replaced built-in ones with the same path, like the loader does.
#[derive(Clone, Debug, Default)]
pub struct Files(pub BTreeMap<String, String>);

impl Files {
    pub fn sources(&self) -> Vec<Source> {
        self.0.iter().map(|(path, text)| Source { path: path.clone(), text: text.clone() }).collect()
    }
    /// A package's own JSON (not merged).
    pub fn raw(&self, path: &str) -> Option<Value> {
        serde_json::from_str(self.0.get(path)?).ok()
    }
    /// The fighter package's JSON with its `extends` chain merged in, like the loader does.
    pub fn resolved(&self, id: &str) -> Option<Value> {
        self.resolve(id, &mut vec![])
    }
    fn resolve(&self, id: &str, seen: &mut Vec<String>) -> Option<Value> {
        if seen.iter().any(|s| s == id) {
            return None;
        }
        seen.push(id.into());
        let v = self.raw(&fighter_path(id))?;
        let Some(parent) = v.get("extends").and_then(Value::as_str) else { return Some(v) };
        let mut base = self.resolve(parent, seen)?;
        if let Some(m) = base.as_object_mut() {
            m.shift_remove("abstract");
        }
        merge(&mut base, &v);
        Some(base)
    }
}

pub fn fighter_path(id: &str) -> String {
    format!("fighters/{id}/fighter.json")
}

/// One fighter being edited.
#[derive(Clone, Debug)]
pub struct Doc {
    pub id: String,
    pub path: String,
    /// The package's own JSON as last loaded or saved: what gets patched and written.
    pub raw: Value,
    /// Merged parent JSON (`None` without `extends`): decides between removing a key and writing `null`.
    pub parent: Option<Value>,
    /// The merged, typed fighter being edited.
    pub fighter: Fighter,
    /// `fighter` as last loaded or saved.
    pub saved: Fighter,
    /// Problems in `fighter` (cross-reference checks); the preview keeps the last good data meanwhile.
    pub errors: Vec<ContentError>,
    undo: Vec<Fighter>,
    redo: Vec<Fighter>,
    /// The last undo step is still growing (a drag or typing in progress).
    open: bool,
}

const UNDO_LIMIT: usize = 300;

impl Doc {
    pub fn open(files: &Files, fighter: &Fighter) -> Self {
        let path = fighter_path(&fighter.id);
        let raw = files.raw(&path).unwrap_or_else(|| Value::Object(Default::default()));
        let parent = raw.get("extends").and_then(Value::as_str).and_then(|p| files.resolved(p));
        Doc {
            id: fighter.id.clone(),
            path,
            raw,
            parent,
            fighter: fighter.clone(),
            saved: fighter.clone(),
            errors: vec![],
            undo: vec![],
            redo: vec![],
            open: false,
        }
    }

    pub fn dirty(&self) -> bool {
        self.fighter != self.saved
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Record that `fighter` changed from `before`. With `ongoing`, later changes join this step until
    /// [`Doc::close_step`] (one undo step per drag or text edit).
    pub fn changed(&mut self, before: Fighter, ongoing: bool) {
        if !(self.open && ongoing) {
            self.undo.push(before);
            if self.undo.len() > UNDO_LIMIT {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.open = ongoing;
        self.check();
    }
    pub fn close_step(&mut self) {
        self.open = false;
    }

    /// A discrete edit (button, shortcut): one undo step if it changed anything.
    pub fn edit(&mut self, f: impl FnOnce(&mut Fighter)) {
        let before = self.fighter.clone();
        f(&mut self.fighter);
        if self.fighter != before {
            self.open = false;
            self.changed(before, false);
        }
    }

    pub fn undo(&mut self) {
        if let Some(f) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.fighter, f));
            self.open = false;
            self.check();
        }
    }
    pub fn redo(&mut self) {
        if let Some(f) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.fighter, f));
            self.open = false;
            self.check();
        }
    }
    pub fn revert(&mut self) {
        let saved = self.saved.clone();
        self.edit(|f| *f = saved);
    }

    /// Cheap live validation of the merged fighter.
    pub fn check(&mut self) {
        self.errors = sim::data::load::check_fighter(&self.path, &self.fighter).err().into_iter().collect();
    }

    /// The own document with the edits since the last save patched in.
    pub fn patched(&self) -> Value {
        let old = serde_json::to_value(&self.saved).expect("fighter serializes");
        let new = serde_json::to_value(&self.fighter).expect("fighter serializes");
        let mut raw = self.raw.clone();
        patch(&mut raw, self.parent.as_ref(), &old, &new);
        raw
    }
}

/// Writes the differences between `old` and `new` (both merged documents, serialized the same way) into `own`.
/// Objects recurse key by key; anything else is replaced whole (arrays too, matching the merge rule). A key gone
/// from `new` is removed from `own`, or set to `null` when the parent still provides it.
pub fn patch(own: &mut Value, parent: Option<&Value>, old: &Value, new: &Value) {
    let (Value::Object(o), Value::Object(n)) = (old, new) else {
        *own = new.clone();
        return;
    };
    if !own.is_object() {
        *own = Value::Object(Default::default());
    }
    let m = own.as_object_mut().expect("object");
    for (k, nv) in n {
        let ov = o.get(k);
        if ov == Some(nv) {
            continue;
        }
        let pv = parent.and_then(|p| p.get(k));
        match (ov, m.get_mut(k)) {
            (Some(ov @ Value::Object(_)), Some(child @ Value::Object(_))) if nv.is_object() => patch(child, pv, ov, nv),
            (Some(ov @ Value::Object(_)), None) if nv.is_object() && pv.is_some() => {
                let mut child = Value::Object(Default::default());
                patch(&mut child, pv, ov, nv);
                m.insert(k.clone(), child);
            }
            _ => drop(m.insert(k.clone(), nv.clone())),
        }
    }
    for k in o.keys().filter(|k| !n.contains_key(*k)) {
        if parent.is_some_and(|p| p.get(k).is_some()) {
            m.insert(k.clone(), Value::Null);
        } else {
            m.shift_remove(k);
        }
    }
}

/// Outcome of a save attempt.
#[derive(Debug)]
pub enum Saved {
    /// Write `text` to the package path; `data` is the content reloaded with it.
    Ok {
        text: String,
        data: Box<GameData>,
    },
    Refused(Vec<ContentError>),
}

/// Everything the editor has open.
pub struct Workspace {
    pub files: Files,
    /// Content as last loaded or saved (what the game should play after leaving the editor).
    pub data: Arc<GameData>,
    pub docs: BTreeMap<String, Doc>,
    /// Bumped on every save.
    pub saves: u32,
}

impl Workspace {
    pub fn new(files: Files) -> Result<(Self, Vec<ContentError>), Vec<ContentError>> {
        let (data, errors) = load(files.sources())?;
        Ok((Workspace { files, data: Arc::new(data), docs: BTreeMap::new(), saves: 0 }, errors))
    }

    pub fn doc(&mut self, id: &str) -> Option<&mut Doc> {
        if !self.docs.contains_key(id) {
            let f = self.data.fighter(id)?;
            let doc = Doc::open(&self.files, f);
            self.docs.insert(id.into(), doc);
        }
        self.docs.get_mut(id)
    }

    /// Loaded content with every open document's current fighter swapped in (unsaved edits included), skipping
    /// documents that don't validate.
    pub fn live_data(&self) -> GameData {
        let mut d = (*self.data).clone();
        for doc in self.docs.values().filter(|doc| doc.errors.is_empty()) {
            if let Some(f) = d.fighters.iter_mut().find(|f| f.id == doc.id) {
                *f = doc.fighter.clone();
            }
        }
        d
    }

    /// Runs the real loader over all content with the patched document in place. Refuses on any error in this
    /// package, or if reloading wouldn't reproduce the edited fighter.
    pub fn prepare_save(&self, id: &str) -> Saved {
        let Some(doc) = self.docs.get(id) else { return Saved::Refused(vec![]) };
        let text = to_compact_string(&doc.patched());
        let mut files = self.files.clone();
        files.0.insert(doc.path.clone(), text.clone());
        let mine = |e: &ContentError| e.file == doc.path || e.file.is_empty();
        match load(files.sources()) {
            Err(errors) => Saved::Refused(errors.into_iter().filter(mine).collect()),
            Ok((data, errors)) => {
                let errors: Vec<_> = errors.into_iter().filter(mine).collect();
                if !errors.is_empty() {
                    return Saved::Refused(errors);
                }
                match data.fighter(id) {
                    Some(f) if *f == doc.fighter => Saved::Ok { text, data: Box::new(data) },
                    _ => Saved::Refused(vec![ContentError {
                        file: doc.path.clone(),
                        path: String::new(),
                        msg: "the saved file wouldn't reload as edited (please report this)".into(),
                    }]),
                }
            }
        }
    }

    /// After the file was written: it's now the package's document and the new baseline.
    pub fn saved(&mut self, id: &str, text: String, data: Box<GameData>) {
        let Some(doc) = self.docs.get_mut(id) else { return };
        self.files.0.insert(doc.path.clone(), text);
        doc.raw = self.files.raw(&doc.path).unwrap_or(Value::Null);
        doc.saved = doc.fighter.clone();
        self.data = Arc::new(*data);
        // Children of this package see the new parent.
        let files = &self.files;
        for d in self.docs.values_mut() {
            d.parent = d.raw.get("extends").and_then(Value::as_str).and_then(|p| files.resolved(p));
        }
        self.saves += 1;
    }
}

/// The built-in assets, for tests.
#[cfg(test)]
pub(crate) fn tests_files() -> Files {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    crate::read_roots(&[root])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn files() -> Files {
        tests_files()
    }

    #[test]
    fn patch_writes_only_changes() {
        let parent = json!({ "stats": { "weight": 1, "gravity": 0.5 }, "moves": { "jab": { "name": "Jab" } } });
        let mut own = json!({ "extends": "base", "name": "X" });
        let old =
            json!({ "name": "X", "stats": { "weight": 1, "gravity": 0.5 }, "moves": { "jab": { "name": "Jab" } } });
        let new = json!({ "name": "X", "stats": { "weight": 2, "gravity": 0.5 }, "moves": {} });
        patch(&mut own, Some(&parent), &old, &new);
        assert_eq!(own, json!({ "extends": "base", "name": "X", "stats": { "weight": 2 }, "moves": { "jab": null } }));
    }

    #[test]
    fn edits_save_as_thin_overrides_and_reload_identically() {
        let (mut ws, errors) = Workspace::new(files()).unwrap();
        assert!(errors.is_empty(), "{errors:?}");
        let id = ws.data.fighters.iter().find(|f| f.extends.is_some()).unwrap().id.clone();
        let doc = ws.doc(&id).unwrap();
        let before = doc.raw.clone();
        doc.edit(|f| {
            f.stats.weight *= 1.25;
            f.moves.jab.phases[0].hitboxes[0].damage += 1.;
            f.moves.jab.phases[0].hitboxes[0].angle = 77.;
        });
        assert!(doc.dirty());
        let patched = doc.patched();
        assert_eq!(patched["extends"], before["extends"]);
        assert_eq!(patched["$schema"], before["$schema"]);
        // Untouched sections stay exactly as written.
        for (k, v) in before.as_object().unwrap() {
            if k != "stats" && k != "moves" {
                assert_eq!(&patched[k], v, "{k} changed");
            }
        }
        let Saved::Ok { text, data } = ws.prepare_save(&id) else { panic!("save refused") };
        ws.saved(&id, text, data);
        let doc = ws.doc(&id).unwrap();
        assert!(!doc.dirty());
        assert_eq!(ws.data.fighter(&id).unwrap().moves.jab.phases[0].hitboxes[0].angle, 77.);
    }

    #[test]
    fn invalid_edits_are_refused_with_a_path() {
        let (mut ws, _) = Workspace::new(files()).unwrap();
        let id = ws.data.fighters[0].id.clone();
        let doc = ws.doc(&id).unwrap();
        doc.edit(|f| {
            f.moves.jab.phases[0].transitions.push(sim::data::Transition {
                on: sim::data::Trigger::Timeout,
                cond: None,
                from: None,
                to: "nowhere".into(),
                lag: None,
            })
        });
        assert_eq!(doc.errors.len(), 1);
        assert!(doc.errors[0].path.starts_with("moves.jab.phases[0].transitions"), "{:?}", doc.errors);
        let Saved::Refused(errors) = ws.prepare_save(&id) else { panic!("saved invalid data") };
        assert!(errors.iter().any(|e| e.path.contains("transitions")), "{errors:?}");
    }

    #[test]
    fn undo_coalesces_ongoing_changes() {
        let (mut ws, _) = Workspace::new(files()).unwrap();
        let id = ws.data.fighters[0].id.clone();
        let doc = ws.doc(&id).unwrap();
        let w0 = doc.fighter.stats.weight;
        for k in 1..=5 {
            let before = doc.fighter.clone();
            doc.fighter.stats.weight = w0 + f64::from(k);
            doc.changed(before, true);
        }
        doc.close_step();
        doc.edit(|f| f.stats.gravity += 0.1);
        doc.undo();
        assert_eq!(doc.fighter.stats.weight, w0 + 5.);
        doc.undo();
        assert_eq!(doc.fighter.stats.weight, w0);
        assert!(!doc.dirty());
        doc.redo();
        assert_eq!(doc.fighter.stats.weight, w0 + 5.);
    }
}
