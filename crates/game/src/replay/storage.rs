//! Replay files, bounded library headers and automatic-save retention.
use super::{model::*, player::Playback};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
};

const HEADER_LIMIT: u64 = 16 * 1024;
#[derive(Serialize, Deserialize)]
pub(super) struct Header {
    version: u32,
    sim_version: String,
    created: u64,
    stage: String,
    fighters: Vec<String>,
}
impl Header {
    pub(super) fn read(reader: &mut impl BufRead) -> Result<Self, String> {
        let mut line = String::new();
        reader.take(HEADER_LIMIT).read_line(&mut line).map_err(|e| e.to_string())?;
        if !line.ends_with('\n') {
            return Err(different_version());
        }
        let h: Self = serde_json::from_str(&line).map_err(|_| different_version())?;
        Ok(h)
    }
    pub(super) fn compatible(&self) -> Result<(), String> {
        if self.version != FORMAT_VERSION || self.sim_version != SIM_VERSION {
            return Err(different_version());
        }
        Ok(())
    }
    pub(super) fn label(&self) -> String {
        format!(
            "{} · {} · {}{}",
            date(self.created),
            self.stage,
            self.fighters.join(" / "),
            if self.version == FORMAT_VERSION && self.sim_version == SIM_VERSION {
                ""
            } else {
                " · recorded with a different version"
            }
        )
    }
}
impl Replay {
    pub(super) fn header(&self) -> Header {
        Header {
            version: self.version,
            sim_version: self.sim_version.clone(),
            created: self.created,
            stage: self.config.stage.clone(),
            fighters: self.config.players.iter().map(|p| p.fighter.clone()).collect(),
        }
    }
    pub(super) fn write(&self, path: &Path) -> Result<(), String> {
        let tmp = path.with_extension("tmp");
        let mut file = fs::File::create(&tmp).map_err(|e| e.to_string())?;
        serde_json::to_writer(&mut file, &self.header()).map_err(|e| e.to_string())?;
        file.write_all(b"\n").map_err(|e| e.to_string())?;
        serde_json::to_writer(&mut file, self).map_err(|e| e.to_string())?;
        file.flush().map_err(|e| e.to_string())?;
        fs::rename(tmp, path).map_err(|e| e.to_string())
    }
    pub(super) fn read(path: &Path) -> Result<Self, String> {
        let mut reader = BufReader::new(fs::File::open(path).map_err(|e| e.to_string())?);
        let header = Header::read(&mut reader)?;
        header.compatible()?;
        let mut r: Self = serde_json::from_reader(reader).map_err(|e| e.to_string())?;
        r.validate()?;
        r.saved_path = Some(path.into());
        Ok(r)
    }
    pub(super) fn refresh_saved_path(&mut self) {
        if self.saved_path.as_ref().is_some_and(|p| !p.is_file()) {
            self.saved_path = None;
        }
    }
    pub fn save(&mut self, automatic: bool) -> Result<PathBuf, String> {
        let dir = directory().ok_or("platform data directory unavailable")?;
        self.save_in(&dir, automatic)
    }
    pub(super) fn save_in(&mut self, dir: &Path, automatic: bool) -> Result<PathBuf, String> {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let name =
            format!("{}-{}-{}.replay.json", if automatic { "auto" } else { "saved" }, self.created, self.config.seed);
        let path = dir.join(name);
        self.write(&path)?;
        self.saved_path = Some(path.clone());
        if automatic {
            let mut files: Vec<_> = paths(dir)
                .into_iter()
                .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("auto-")))
                .collect();
            files.sort();
            let remove = files.len().saturating_sub(self.data.global.replay.auto_save_count);
            for p in files.into_iter().take(remove) {
                if let Err(e) = fs::remove_file(&p) {
                    bevy::log::warn!("replay cleanup {}: {e}", p.display());
                }
            }
        }
        Ok(path)
    }
}
pub(super) fn directory() -> Option<PathBuf> {
    crate::settings::replays_dir()
}
pub(super) fn paths(dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<_> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(".replay.json"))
        .collect();
    paths.sort();
    paths.reverse();
    paths
}

pub fn verify_directory(dir: &Path) -> Result<(), String> {
    let files = paths(dir);
    if files.is_empty() {
        return Err(format!("no .replay.json files in {}", dir.display()));
    }
    for file in files {
        let mut p = Playback::new(Replay::read(&file)?)?;
        let mut w = p.snapshots[0].1.clone();
        p.seek(p.replay.ticks, &mut w);
        if let Some(e) = p.warning {
            return Err(format!("{}: {e}", file.display()));
        }
        if !w.over {
            return Err(format!("{}: unfinished match", file.display()));
        }
        println!("{}: verified {} ticks, winner {:?}", file.display(), p.position, w.winner);
    }
    Ok(())
}

// Gregorian date from days since Unix epoch, independent of local timezone.
fn date(seconds: u64) -> String {
    let z = (seconds / 86400) as i64 + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    format!(
        "{:04}-{month:02}-{day:02} {:02}:{:02} UTC",
        y + i64::from(month <= 2),
        seconds / 3600 % 24,
        seconds / 60 % 60
    )
}

pub(super) fn entries() -> Vec<(PathBuf, String)> {
    directory()
        .map(|d| paths(&d))
        .unwrap_or_default()
        .into_iter()
        .map(|path| {
            let label = match fs::File::open(&path)
                .map_err(|e| e.to_string())
                .and_then(|f| Header::read(&mut BufReader::new(f)))
            {
                Ok(h) => h.label(),
                Err(e) => format!("Unreadable: {e}"),
            };
            (path, label)
        })
        .collect()
}
pub(super) fn delete(path: &Path) -> Result<(), String> {
    fs::remove_file(path).map_err(|e| format!("Delete failed: {e}"))
}
