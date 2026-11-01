use serde::{Deserialize, Serialize};
use sim::GameData;
pub fn serialize<S: serde::Serializer>(data: &GameData, s: S) -> Result<S::Ok, S::Error> {
    (
        data,
        data.fighters.iter().map(|f| &f.id).collect::<Vec<_>>(),
        data.stages.iter().map(|s| &s.id).collect::<Vec<_>>(),
    )
        .serialize(s)
}
// Replay-only presentation migration; supplied values remain intact except unused select-screen looks.
static RENDER: std::sync::LazyLock<serde_json::Value> = std::sync::LazyLock::new(|| {
    let g: serde_json::Value = serde_json::from_str(include_str!("../../../../assets/global.json")).unwrap();
    g["render"].clone()
});
fn presentation(v: &mut serde_json::Value) {
    let Some(render) = v.pointer_mut("/global/render") else { return };
    if let Some(r) = render.as_object_mut() {
        r.insert("select".into(), RENDER["select"].clone());
    }
    for (path, fields) in [
        ("", &["interpolate"][..]),
        ("/hud", &["look"][..]),
        ("/camera", &["referencePlayWidth", "referencePlayHeight", "minScaleStart", "minScaleFloor"][..]),
    ] {
        if let Some(object) = render.pointer_mut(path).and_then(|v| v.as_object_mut()) {
            let bundled = RENDER.pointer(path).unwrap();
            for field in fields {
                object.entry(*field).or_insert_with(|| bundled[*field].clone());
            }
        }
    }
}
pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<GameData, D::Error> {
    let (mut v, fighters, stages): (serde_json::Value, Vec<String>, Vec<String>) = Deserialize::deserialize(d)?;
    presentation(&mut v);
    let mut data: GameData = serde_json::from_value(v).map_err(serde::de::Error::custom)?;
    if data.fighters.len() != fighters.len() || data.stages.len() != stages.len() {
        return Err(serde::de::Error::custom("invalid content ids"));
    }
    for (f, id) in data.fighters.iter_mut().zip(fighters) {
        f.id = id;
    }
    for (s, id) in data.stages.iter_mut().zip(stages) {
        s.id = id;
    }
    let errors = sim::data::validate(&data);
    if !errors.is_empty() {
        return Err(serde::de::Error::custom(format!(
            "invalid replay content: {}",
            errors.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
        )));
    }
    Ok(data)
}
