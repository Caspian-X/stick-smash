//! The editor's model, independent of Bevy and egui: the content being edited, per-package documents (fighters and
//! stages) with undo, validation through the real loader, and saving that patches only the changed paths into the
//! package's own JSON, so `extends` and untouched values stay intact (see docs/DESIGN.md "Editor").

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;
use sim::GameData;
use sim::data::fighter::Fighter;
use sim::data::json::{merge, to_compact_string};
use sim::data::stage::Stage;
use sim::data::visual::{AnimKey, Pose, PoseRef};
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
pub fn stage_path(id: &str) -> String {
    format!("stages/{id}/stage.json")
}

/// A kind of package the editor edits: where it lives, how it's checked, and where the workspace keeps its docs.
pub trait Content: Clone + PartialEq + Serialize + std::fmt::Debug + Sized {
    const KIND: Kind;
    fn path(id: &str) -> String;
    /// Cross-reference checks (the loader's), on the typed value.
    fn check(path: &str, v: &Self) -> Vec<ContentError>;
    fn find<'a>(d: &'a GameData, id: &str) -> Option<&'a Self>;
    fn name(&self) -> &str;
    fn docs(ws: &Workspace) -> &BTreeMap<String, Doc<Self>>;
    fn docs_mut(ws: &mut Workspace) -> &mut BTreeMap<String, Doc<Self>>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    Fighter,
    Stage,
}

impl Kind {
    pub fn path(self, id: &str) -> String {
        match self {
            Kind::Fighter => fighter_path(id),
            Kind::Stage => stage_path(id),
        }
    }
    pub fn noun(self) -> &'static str {
        match self {
            Kind::Fighter => "fighter",
            Kind::Stage => "stage",
        }
    }
}

impl Content for Fighter {
    const KIND: Kind = Kind::Fighter;
    fn path(id: &str) -> String {
        fighter_path(id)
    }
    fn check(path: &str, v: &Self) -> Vec<ContentError> {
        sim::data::load::check_fighter(path, v).err().into_iter().collect()
    }
    fn find<'a>(d: &'a GameData, id: &str) -> Option<&'a Self> {
        d.fighter(id)
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn docs(ws: &Workspace) -> &BTreeMap<String, Doc<Self>> {
        &ws.docs
    }
    fn docs_mut(ws: &mut Workspace) -> &mut BTreeMap<String, Doc<Self>> {
        &mut ws.docs
    }
}

impl Content for Stage {
    const KIND: Kind = Kind::Stage;
    fn path(id: &str) -> String {
        stage_path(id)
    }
    fn check(path: &str, v: &Self) -> Vec<ContentError> {
        sim::data::load::check_stage(path, v).err().into_iter().collect()
    }
    fn find<'a>(d: &'a GameData, id: &str) -> Option<&'a Self> {
        d.stage(id)
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn docs(ws: &Workspace) -> &BTreeMap<String, Doc<Self>> {
        &ws.stages
    }
    fn docs_mut(ws: &mut Workspace) -> &mut BTreeMap<String, Doc<Self>> {
        &mut ws.stages
    }
}

/// Fighter package ids in the files (abstract ones too), for `extends`.
pub fn package_ids(files: &Files) -> Vec<String> {
    files
        .0
        .keys()
        .filter_map(|p| p.strip_prefix("fighters/")?.strip_suffix("/fighter.json").map(String::from))
        .collect()
}

/// `base`, or `base2`, `base3`… whichever isn't taken.
pub fn unique_name(base: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(base) {
        return base.into();
    }
    (2..).map(|n| format!("{base}{n}")).find(|n| !taken(n)).expect("some name is free")
}

/// Calls `fx` on every pose key in the fighter: move phases and state animations.
fn each_anim_key(f: &mut Fighter, mut fx: impl FnMut(&mut AnimKey)) {
    for slot in crate::SLOTS {
        if let Some(mv) = crate::move_mut(&mut f.moves, slot) {
            mv.phases.iter_mut().flat_map(|p| p.anim.iter_mut()).for_each(&mut fx);
        }
    }
    f.anims.values_mut().flat_map(|a| a.keys.iter_mut()).for_each(fx);
}

/// How many pose keys and poses refer to the named pose.
pub fn pose_uses(f: &Fighter, name: &str) -> usize {
    let named = |k: &AnimKey| match &k.pose {
        PoseRef::Named(n) => n == name,
        PoseRef::Inline(p) => p.from.as_deref() == Some(name),
    };
    f.moves.iter().flat_map(|(_, m)| m.phases.iter()).flat_map(|p| p.anim.iter()).filter(|k| named(k)).count()
        + f.anims.values().flat_map(|a| a.keys.iter()).filter(|k| named(k)).count()
        + f.poses.values().filter(|p| p.from.as_deref() == Some(name)).count()
}

/// Like `pose_uses`, but `stand` also counts the poses and inline keys without a `from` (they fill in from it).
pub fn pose_users(f: &Fighter, name: &str) -> usize {
    if name != "stand" {
        return pose_uses(f, name);
    }
    let inline = |k: &AnimKey| matches!(&k.pose, PoseRef::Inline(p) if p.from.is_none());
    pose_uses(f, name)
        + f.moves.iter().flat_map(|(_, m)| m.phases.iter()).flat_map(|p| p.anim.iter()).filter(|k| inline(k)).count()
        + f.anims.values().flat_map(|a| a.keys.iter()).filter(|k| inline(k)).count()
        + f.poses.iter().filter(|(n, p)| *n != name && p.from.is_none()).count()
}

/// "Used by 3 keys and poses" / "Not used by any key or pose".
pub fn uses_text(name: &str, n: usize) -> String {
    match (n, name == "stand") {
        (0, _) => "Not used by any key or pose".into(),
        (1, _) => "Used by 1 key or pose".into(),
        (n, false) => format!("Used by {n} keys and poses"),
        (n, true) => format!("The base pose: {n} keys and poses build on it"),
    }
}

/// Renames a pose and every reference to it. Refuses taken or empty names.
pub fn rename_pose(f: &mut Fighter, old: &str, new: &str) -> bool {
    if new.is_empty() || f.poses.contains_key(new) {
        return false;
    }
    let Some(p) = f.poses.remove(old) else { return false };
    f.poses.insert(new.into(), p);
    let fix = |n: &mut Option<String>| {
        if n.as_deref() == Some(old) {
            *n = Some(new.into());
        }
    };
    for p in f.poses.values_mut() {
        fix(&mut p.from);
    }
    each_anim_key(f, |k| match &mut k.pose {
        PoseRef::Named(n) if n == old => *n = new.into(),
        PoseRef::Inline(p) => fix(&mut p.from),
        PoseRef::Named(_) => {}
    });
    true
}

/// Adds a pose (a copy of `from`'s joints, or one that inherits `stand`) under a free name; returns the name.
pub fn new_pose(f: &mut Fighter, base: &str, from: Option<&Pose>) -> String {
    let name = unique_name(base, |n| f.poses.contains_key(n));
    f.poses.insert(
        name.clone(),
        from.cloned().unwrap_or_else(|| Pose { from: Some("stand".into()), ..Default::default() }),
    );
    name
}

/// Copies move `from` over slot `to`.
pub fn copy_move(f: &mut Fighter, from: &str, to: &str) -> bool {
    let Some(mv) = crate::move_ref(&f.moves, from).cloned() else { return false };
    if to == "sspec" && f.moves.sspec.is_none() {
        f.moves.sspec = Some(mv);
        return true;
    }
    crate::move_mut(&mut f.moves, to).map(|m| *m = mv).is_some()
}

/// One package (fighter or stage) being edited.
#[derive(Clone, Debug)]
pub struct Doc<T: Content = Fighter> {
    pub id: String,
    pub path: String,
    /// The package's own JSON as last loaded or saved: what gets patched and written.
    pub raw: Value,
    /// Merged parent JSON (`None` without `extends`): decides between removing a key and writing `null`.
    pub parent: Option<Value>,
    /// The merged, typed value being edited.
    pub item: T,
    /// `item` as last loaded or saved.
    pub saved: T,
    /// Problems in `item` (cross-reference checks); the preview keeps the last good data meanwhile.
    pub errors: Vec<ContentError>,
    /// Created in the editor and never written: unsaved until the first save.
    pub fresh: bool,
    undo: Vec<T>,
    redo: Vec<T>,
    /// The last undo step is still growing (a drag or typing in progress).
    open: bool,
}

const UNDO_LIMIT: usize = 300;

impl<T: Content> Doc<T> {
    pub fn open(files: &Files, id: &str, item: &T) -> Self {
        let path = T::path(id);
        let raw = files.raw(&path).unwrap_or_else(|| Value::Object(Default::default()));
        let parent = raw.get("extends").and_then(Value::as_str).and_then(|p| files.resolved(p));
        Doc {
            id: id.into(),
            path,
            raw,
            parent,
            item: item.clone(),
            saved: item.clone(),
            errors: vec![],
            fresh: false,
            undo: vec![],
            redo: vec![],
            open: false,
        }
    }

    pub fn dirty(&self) -> bool {
        self.fresh || self.item != self.saved
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Record that `item` changed from `before`. With `ongoing`, later changes join this step until
    /// [`Doc::close_step`] (one undo step per drag or text edit).
    pub fn changed(&mut self, before: T, ongoing: bool) {
        // A discrete `edit` inside the frame already recorded this step.
        if !(self.open && ongoing) && self.undo.last() != Some(&before) {
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
    pub fn edit(&mut self, f: impl FnOnce(&mut T)) {
        let before = self.item.clone();
        f(&mut self.item);
        if self.item != before {
            self.open = false;
            self.changed(before, false);
        }
    }

    pub fn undo(&mut self) {
        if let Some(f) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.item, f));
            self.open = false;
            self.check();
        }
    }
    pub fn redo(&mut self) {
        if let Some(f) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.item, f));
            self.open = false;
            self.check();
        }
    }
    pub fn revert(&mut self) {
        let saved = self.saved.clone();
        self.edit(|f| *f = saved);
    }

    /// Applies `f` to every version of the value (current, saved, undo history), e.g. a new id.
    pub fn rewrite(&mut self, f: impl Fn(&mut T)) {
        f(&mut self.item);
        f(&mut self.saved);
        self.undo.iter_mut().chain(self.redo.iter_mut()).for_each(f);
    }

    /// Cheap live validation of the merged value.
    pub fn check(&mut self) {
        self.errors = T::check(&self.path, &self.item);
    }

    /// The own document with the edits since the last save patched in.
    pub fn patched(&self) -> Value {
        let old = serde_json::to_value(&self.saved).expect("content serializes");
        let new = serde_json::to_value(&self.item).expect("content serializes");
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
            _ => drop(m.insert(k.clone(), tidy(nv))),
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

/// `nv` with whole floats written as integers (`290`, not `290.0`), the way hand-written content has them.
fn tidy(v: &Value) -> Value {
    match v {
        Value::Number(n) if n.is_f64() => match n.as_f64() {
            Some(f) if f.fract() == 0. && f.abs() < 9e15 => Value::from(f as i64),
            _ => v.clone(),
        },
        Value::Array(a) => Value::Array(a.iter().map(tidy).collect()),
        Value::Object(m) => Value::Object(m.iter().map(|(k, x)| (k.clone(), tidy(x))).collect()),
        _ => v.clone(),
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
    pub docs: BTreeMap<String, Doc<Fighter>>,
    pub stages: BTreeMap<String, Doc<Stage>>,
    /// Bumped on every save.
    pub saves: u32,
}

/// Package ids: lowercase letters, digits, `-` and `_`.
fn valid_id(id: &str) -> Result<(), String> {
    let ok = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_';
    if id.is_empty() || !id.chars().all(ok) {
        return Err("Use lowercase letters, digits, - and _ for the id".into());
    }
    Ok(())
}

/// `My Stage!` → `my-stage`.
pub fn slug(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || *c == ' ')
        .collect::<String>()
        .trim()
        .replace(' ', "-")
}

impl Workspace {
    pub fn new(files: Files) -> Result<(Self, Vec<ContentError>), Vec<ContentError>> {
        let (data, errors) = load(files.sources())?;
        let ws = Workspace { files, data: Arc::new(data), docs: BTreeMap::new(), stages: BTreeMap::new(), saves: 0 };
        Ok((ws, errors))
    }

    /// The document for package `id`, opened on first use.
    pub fn open<T: Content>(&mut self, id: &str) -> Option<&mut Doc<T>> {
        if !T::docs(self).contains_key(id) {
            let item = T::find(&self.data, id)?;
            let doc = Doc::open(&self.files, id, item);
            T::docs_mut(self).insert(id.into(), doc);
        }
        T::docs_mut(self).get_mut(id)
    }
    pub fn doc(&mut self, id: &str) -> Option<&mut Doc<Fighter>> {
        self.open(id)
    }
    pub fn stage(&mut self, id: &str) -> Option<&mut Doc<Stage>> {
        self.open(id)
    }

    /// Loaded content with every open document's current value swapped in (unsaved edits and new packages
    /// included), skipping documents that don't validate.
    pub fn live_data(&self) -> GameData {
        let mut d = (*self.data).clone();
        for doc in self.docs.values().filter(|doc| doc.errors.is_empty()) {
            match d.fighters.iter_mut().find(|f| f.id == doc.id) {
                Some(f) => *f = doc.item.clone(),
                None => d.fighters.push(doc.item.clone()),
            }
        }
        for doc in self.stages.values().filter(|doc| doc.errors.is_empty()) {
            match d.stages.iter_mut().find(|s| s.id == doc.id) {
                Some(s) => *s = doc.item.clone(),
                None => d.stages.push(doc.item.clone()),
            }
        }
        d
    }

    /// Loaded packages of a kind plus new unsaved ones: id, name, dirty.
    fn listing<T: Content>(&self, loaded: Vec<(&str, &T)>) -> Vec<(String, String, bool)> {
        let docs = T::docs(self);
        let mut v: Vec<_> = loaded
            .into_iter()
            .map(|(id, x)| match docs.get(id) {
                Some(d) => (id.to_string(), d.item.name().to_string(), d.dirty()),
                None => (id.to_string(), x.name().to_string(), false),
            })
            .collect();
        v.extend(docs.values().filter(|d| d.fresh).map(|d| (d.id.clone(), d.item.name().to_string(), true)));
        v
    }
    /// Playable fighters, including new unsaved ones: id, name, dirty.
    pub fn fighters(&self) -> Vec<(String, String, bool)> {
        self.listing(self.data.fighters.iter().map(|f| (f.id.as_str(), f)).collect())
    }
    pub fn stage_list(&self) -> Vec<(String, String, bool)> {
        self.listing(self.data.stages.iter().map(|s| (s.id.as_str(), s)).collect())
    }

    /// Names of packages with unsaved changes.
    pub fn unsaved(&self) -> Vec<String> {
        let f = self.docs.values().filter(|d| d.dirty()).map(|d| d.item.name.clone());
        f.chain(self.stages.values().filter(|d| d.dirty()).map(|d| format!("{} (stage)", d.item.name))).collect()
    }
    /// Every document with unsaved changes.
    pub fn dirty(&self) -> Vec<(Kind, String)> {
        let f = self.docs.values().filter(|d| d.dirty()).map(|d| (Kind::Fighter, d.id.clone()));
        f.chain(self.stages.values().filter(|d| d.dirty()).map(|d| (Kind::Stage, d.id.clone()))).collect()
    }

    fn free_id(&self, kind: Kind, id: &str) -> Result<(), String> {
        valid_id(id)?;
        let taken = match kind {
            Kind::Fighter => self.docs.contains_key(id),
            Kind::Stage => self.stages.contains_key(id),
        };
        if self.files.0.contains_key(&kind.path(id)) || taken {
            return Err(format!("A {} called {id} already exists", kind.noun()));
        }
        Ok(())
    }

    /// Loads all content with `raw` as package `id` of kind `T` and opens it as a fresh (unsaved) document.
    fn add_fresh<T: Content>(&mut self, id: &str, raw: &Value) -> Result<(), String> {
        let path = T::path(id);
        let mut files = self.files.clone();
        files.0.insert(path.clone(), to_compact_string(raw));
        let (data, errors) =
            load(files.sources()).map_err(|e| e.iter().map(ToString::to_string).collect::<Vec<_>>().join("; "))?;
        if let Some(e) = errors.iter().find(|e| e.file == path) {
            return Err(e.to_string());
        }
        let item = T::find(&data, id).ok_or(format!("The new {} didn't load", T::KIND.noun()))?;
        let mut doc = Doc::open(&files, id, item);
        doc.fresh = true;
        T::docs_mut(self).insert(id.into(), doc);
        Ok(())
    }

    /// A new fighter package `id` that inherits everything from `extends`. Lives in memory until saved.
    pub fn new_fighter(&mut self, id: &str, name: &str, extends: &str) -> Result<(), String> {
        self.free_id(Kind::Fighter, id)?;
        let parent = self.files.raw(&fighter_path(extends)).ok_or(format!("No fighter {extends}"))?;
        let version = parent.get("schemaVersion").cloned().unwrap_or(1.into());
        let raw = serde_json::json!({
            "$schema": "../../schema/fighter.schema.json",
            "schemaVersion": version,
            "extends": extends,
            "name": if name.is_empty() { id } else { name },
        });
        self.add_fresh::<Fighter>(id, &raw)
    }

    /// A new stage package `id`: a copy of stage `from` as it is now (unsaved edits included). Stages don't inherit,
    /// so the copy is a whole file. Lives in memory until saved.
    pub fn new_stage(&mut self, id: &str, name: &str, from: &str) -> Result<(), String> {
        self.free_id(Kind::Stage, id)?;
        let mut raw = match self.stages.get(from) {
            Some(d) => d.patched(),
            None => self.files.raw(&stage_path(from)).ok_or(format!("No stage {from}"))?,
        };
        let m = raw.as_object_mut().ok_or(format!("Stage {from} isn't an object"))?;
        m.insert("name".into(), Value::String(if name.is_empty() { id.into() } else { name.into() }));
        self.add_fresh::<Stage>(id, &raw)
    }

    /// Renames a stage (undoable). A new stage that was never saved can also take a new id (its folder); saved
    /// stages keep theirs, since the old file would stay behind.
    pub fn rename_stage(&mut self, id: &str, new_id: &str, name: &str) -> Result<(), String> {
        let fresh = self.stage(id).ok_or(format!("No stage {id}"))?.fresh;
        if new_id != id {
            if !fresh {
                return Err("Only new, unsaved stages can change their id (duplicate it instead)".into());
            }
            self.free_id(Kind::Stage, new_id)?;
            let mut doc = self.stages.remove(id).expect("checked");
            doc.id = new_id.into();
            doc.path = stage_path(new_id);
            doc.rewrite(|s| s.id = new_id.into());
            doc.check();
            self.stages.insert(new_id.into(), doc);
        }
        let name = if name.is_empty() { new_id } else { name };
        self.stages.get_mut(new_id).expect("open").edit(|s| s.name = name.into());
        Ok(())
    }

    /// Runs the real loader over all content with the patched document in place. Refuses on any error in this
    /// package, or if reloading wouldn't reproduce the edited value.
    pub fn prepare_save<T: Content>(&self, id: &str) -> Saved {
        let Some(doc) = T::docs(self).get(id) else { return Saved::Refused(vec![]) };
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
                match T::find(&data, id) {
                    Some(f) if *f == doc.item => Saved::Ok { text, data: Box::new(data) },
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
    pub fn saved<T: Content>(&mut self, id: &str, text: String, data: Box<GameData>) {
        let Some(doc) = T::docs_mut(self).get_mut(id) else { return };
        let path = doc.path.clone();
        self.files.0.insert(path.clone(), text);
        let raw = self.files.raw(&path).unwrap_or(Value::Null);
        let doc = T::docs_mut(self).get_mut(id).expect("open");
        doc.raw = raw;
        doc.saved = doc.item.clone();
        doc.fresh = false;
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
        let Saved::Ok { text, data } = ws.prepare_save::<Fighter>(&id) else { panic!("save refused") };
        ws.saved::<Fighter>(&id, text, data);
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
        let Saved::Refused(errors) = ws.prepare_save::<Fighter>(&id) else { panic!("saved invalid data") };
        assert!(errors.iter().any(|e| e.path.contains("transitions")), "{errors:?}");
    }

    #[test]
    fn new_fighter_saves_as_a_thin_package() {
        let (mut ws, _) = Workspace::new(files()).unwrap();
        assert!(ws.new_fighter("Bad Id", "", "base").is_err());
        assert!(ws.new_fighter("brawler", "", "base").is_err());
        ws.new_fighter("ninja", "Ninja", "brawler").unwrap();
        assert!(ws.fighters().iter().any(|(id, name, dirty)| id == "ninja" && name == "Ninja" && *dirty));
        assert!(ws.live_data().fighter("ninja").is_some());
        let doc = ws.doc("ninja").unwrap();
        doc.edit(|f| f.stats.weight = 0.8);
        let Saved::Ok { text, data } = ws.prepare_save::<Fighter>("ninja") else { panic!("save refused") };
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["extends"], "brawler");
        assert_eq!(v["stats"], serde_json::json!({ "weight": 0.8 }));
        assert!(v.get("moves").is_none());
        ws.saved::<Fighter>("ninja", text, data);
        assert!(!ws.doc("ninja").unwrap().dirty());
        assert_eq!(ws.data.fighter("ninja").unwrap().stats.weight, 0.8);
    }

    #[test]
    fn stand_counts_the_poses_that_build_on_it() {
        let (ws, _) = Workspace::new(files()).unwrap();
        for f in &ws.data.fighters {
            assert!(pose_users(f, "stand") > 0, "{}", f.id);
            assert!(pose_users(f, "stand") >= pose_uses(f, "stand"));
        }
        assert_eq!(uses_text("crouch", 1), "Used by 1 key or pose");
        assert_eq!(uses_text("crouch", 3), "Used by 3 keys and poses");
    }

    #[test]
    fn renaming_a_pose_follows_references_and_saves() {
        let (mut ws, _) = Workspace::new(files()).unwrap();
        let id =
            ws.data.fighters.iter().find(|f| f.extends.is_some() && pose_uses(f, "crouch") > 0).unwrap().id.clone();
        let doc = ws.doc(&id).unwrap();
        let name = "crouch".to_string();
        let uses = pose_uses(&doc.item, &name);
        assert!(uses > 0);
        doc.edit(|f| assert!(rename_pose(f, &name, "renamed")));
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
        assert_eq!(pose_uses(&doc.item, "renamed"), uses);
        assert_eq!(pose_uses(&doc.item, &name), 0);
        doc.edit(|f| assert!(!rename_pose(f, "renamed", "stand")));
        let made = {
            let mut f = doc.item.clone();
            new_pose(&mut f, "renamed", None)
        };
        assert_eq!(made, "renamed2");
        let Saved::Ok { text, data } = ws.prepare_save::<Fighter>(&id) else { panic!("save refused") };
        ws.saved::<Fighter>(&id, text, data);
        assert!(ws.data.fighter(&id).unwrap().poses.contains_key("renamed"));
    }

    #[test]
    fn copying_a_move_fills_the_slot() {
        let (mut ws, _) = Workspace::new(files()).unwrap();
        let id = ws.data.fighters[0].id.clone();
        let doc = ws.doc(&id).unwrap();
        doc.edit(|f| {
            f.moves.sspec = None;
            assert!(copy_move(f, "nspec", "sspec"));
            assert!(copy_move(f, "jab", "ftilt"));
            assert!(!copy_move(f, "jab", "nope"));
        });
        assert_eq!(doc.item.moves.sspec.as_ref(), Some(&doc.item.moves.nspec));
        assert_eq!(doc.item.moves.ftilt, doc.item.moves.jab);
        assert!(doc.errors.is_empty(), "{:?}", doc.errors);
    }

    #[test]
    fn undo_coalesces_ongoing_changes() {
        let (mut ws, _) = Workspace::new(files()).unwrap();
        let id = ws.data.fighters[0].id.clone();
        let doc = ws.doc(&id).unwrap();
        let w0 = doc.item.stats.weight;
        for k in 1..=5 {
            let before = doc.item.clone();
            doc.item.stats.weight = w0 + f64::from(k);
            doc.changed(before, true);
        }
        doc.close_step();
        doc.edit(|f| f.stats.gravity += 0.1);
        doc.undo();
        assert_eq!(doc.item.stats.weight, w0 + 5.);
        doc.undo();
        assert_eq!(doc.item.stats.weight, w0);
        assert!(!doc.dirty());
        doc.redo();
        assert_eq!(doc.item.stats.weight, w0 + 5.);
    }

    #[test]
    fn stage_edits_patch_only_changed_paths() {
        let (mut ws, _) = Workspace::new(files()).unwrap();
        let before_text = ws.files.0[&stage_path("arena")].clone();
        let doc = ws.stage("arena").unwrap();
        let before = doc.raw.clone();
        doc.edit(|s| {
            s.platforms[0].y -= 20.;
            s.ground.left -= 10.;
            s.art.edge = sim::data::Color::Rgb(0x112233);
        });
        assert!(doc.dirty() && doc.errors.is_empty());
        let patched = doc.patched();
        assert_eq!(patched["$schema"], before["$schema"]);
        assert_eq!(patched["ground"]["left"], 290.);
        assert_eq!(patched["ground"]["right"], before["ground"]["right"]);
        assert_eq!(patched["art"]["edge"], "#112233");
        for k in ["spawns", "respawns", "blast", "camera", "ledge"] {
            assert_eq!(patched[k], before[k], "{k} changed");
        }
        // Key order and untouched formatting survive: only the edited lines differ.
        let Saved::Ok { text, data } = ws.prepare_save::<Stage>("arena") else { panic!("save refused") };
        let changed = before_text.lines().zip(text.lines()).filter(|(a, b)| a != b).count();
        let diff: Vec<_> = before_text.lines().zip(text.lines()).filter(|(a, b)| a != b).collect();
        assert!(changed <= 3, "{diff:#?}");
        ws.saved::<Stage>("arena", text, data);
        assert!(!ws.stage("arena").unwrap().dirty());
        assert_eq!(ws.data.stage("arena").unwrap().ground.left, 290.);
        // Invalid geometry is caught live and refused.
        let doc = ws.stage("arena").unwrap();
        doc.edit(|s| s.platforms[0].x2 = s.platforms[0].x1 - 1.);
        assert_eq!(doc.errors[0].path, "platforms[0]");
        assert!(matches!(ws.prepare_save::<Stage>("arena"), Saved::Refused(_)));
    }

    #[test]
    fn new_and_duplicated_stages_are_whole_copies() {
        let (mut ws, _) = Workspace::new(files()).unwrap();
        assert!(ws.new_stage("Bad Id", "", "arena").is_err());
        assert!(ws.new_stage("arena", "", "arena").is_err());
        assert!(ws.new_stage("x", "", "nowhere").is_err());
        // Duplicating carries unsaved edits along.
        ws.stage("arena").unwrap().edit(|s| s.ground.top = 500.);
        ws.new_stage("sky", "Sky", "arena").unwrap();
        assert_eq!(slug("Sky Fortress!"), "sky-fortress");
        let doc = ws.stage("sky").unwrap();
        assert!(doc.fresh && doc.dirty());
        assert_eq!((doc.item.name.as_str(), doc.item.id.as_str(), doc.item.ground.top), ("Sky", "sky", 500.));
        assert!(ws.stage_list().iter().any(|(id, name, dirty)| id == "sky" && name == "Sky" && *dirty));
        assert!(ws.live_data().stage("sky").is_some());
        assert_eq!(ws.dirty().len(), 2);
        let Saved::Ok { text, data } = ws.prepare_save::<Stage>("sky") else { panic!("save refused") };
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["name"], "Sky");
        assert_eq!(v["$schema"], "../../schema/stage.schema.json");
        assert!(v.get("spawns").is_some_and(Value::is_array));
        ws.saved::<Stage>("sky", text, data);
        assert!(!ws.stage("sky").unwrap().dirty());
        assert!(ws.data.stage("sky").is_some());
    }

    #[test]
    fn renaming_a_stage_changes_the_id_only_while_new() {
        let (mut ws, _) = Workspace::new(files()).unwrap();
        ws.rename_stage("arena", "arena", "Big Arena").unwrap();
        assert_eq!(ws.stage("arena").unwrap().item.name, "Big Arena");
        ws.stage("arena").unwrap().undo();
        assert_eq!(ws.stage("arena").unwrap().item.name, "Arena");
        assert!(ws.rename_stage("arena", "arena2", "Arena").is_err(), "saved stages keep their folder");
        ws.new_stage("draft", "Draft", "arena").unwrap();
        ws.stage("draft").unwrap().edit(|s| s.ground.top -= 5.);
        ws.rename_stage("draft", "final", "Final").unwrap();
        assert!(!ws.stages.contains_key("draft"));
        let doc = ws.stage("final").unwrap();
        assert_eq!((doc.item.id.as_str(), doc.path.as_str()), ("final", "stages/final/stage.json"));
        doc.undo();
        doc.undo();
        assert_eq!(doc.item.id, "final", "history follows the new id");
        assert!(ws.live_data().stage("final").is_some());
        let Saved::Ok { .. } = ws.prepare_save::<Stage>("final") else { panic!("save refused") };
    }
}
