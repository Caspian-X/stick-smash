//! Startup synth bank → in-memory WAV assets → Bevy audio. No sim RNG or device assumptions.

use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::TAU;

use bevy::audio::{AudioSinkPlayback, Volume};
use bevy::prelude::*;
use sim::Fx;
use sim::data::audio::{Audio, Sound, Wave};

use crate::input::{Devices, Nav};
use crate::play::{Match, Mode};
use crate::select::Select;
use crate::{Data, Screen, input, settings};

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Bank>()
            .add_systems(Startup, synthesize)
            .add_systems(PreUpdate, navigation.after(input::poll))
            .add_systems(PostUpdate, (selection, playback).chain());
    }
}

#[derive(Resource, Default)]
struct Bank {
    clips: BTreeMap<String, Vec<Handle<AudioSource>>>,
    pending: BTreeMap<String, (Option<f64>, Option<f64>)>,
    last: BTreeMap<String, f64>,
    unknown: BTreeSet<String>,
    rng: u32,
}

impl Bank {
    fn queue(&mut self, name: &str, arg: Option<f64>, x: Option<f64>) {
        let old = self.pending.entry(name.into()).or_insert((arg, x));
        if arg.unwrap_or(0.) > old.0.unwrap_or(0.) {
            *old = (arg, x);
        }
    }
}

#[derive(Component)]
struct Voice {
    gain: f32,
    expires: f64,
    priority: u32,
}

fn synthesize(
    mut commands: Commands,
    data: Res<Data>,
    mut bank: ResMut<Bank>,
    mut assets: ResMut<Assets<AudioSource>>,
) {
    let a = &data.0.global.audio;
    commands.spawn(SpatialListener::new(a.ear_gap as f32));
    for (name, sound) in &a.sounds {
        let clips = (0..a.strength_levels)
            .map(|i| {
                let strength = i as f64 / a.strength_levels.saturating_sub(1).max(1) as f64;
                assets.add(AudioSource { bytes: wav(a, sound, strength).into() })
            })
            .collect();
        bank.clips.insert(name.clone(), clips);
    }
}

/// Swept oscillator + low-pass noise layers with a short attack and exponential release.
fn wav(a: &Audio, sound: &Sound, strength: f64) -> Vec<u8> {
    let stretch = 1. + strength * (sound.strength_duration - 1.);
    let duration = sound.layers.iter().map(|l| l.duration * stretch).fold(0., f64::max);
    let mut samples = vec![0.; (duration * f64::from(a.sample_rate)).ceil() as usize];
    let mut rng = 0x79ab_cdef_u32;
    for l in &sound.layers {
        let duration = l.duration * stretch;
        let mut phase = 0.;
        let mut low = 0.;
        for (i, sample) in samples.iter_mut().enumerate() {
            let t = i as f64 / f64::from(a.sample_rate);
            if t >= duration {
                break;
            }
            let u = t / duration;
            let hz = l.from * (l.to / l.from).powf(u);
            phase = (phase + hz / f64::from(a.sample_rate)).fract();
            rng ^= rng << 13;
            rng ^= rng >> 17;
            rng ^= rng << 5;
            let noise = f64::from(rng) / f64::from(u32::MAX) * 2. - 1.;
            low += (noise - low) * (1. - (-TAU * hz / f64::from(a.sample_rate)).exp());
            let wave = match l.wave {
                Wave::Sine => (phase * TAU).sin(),
                Wave::Triangle => 1. - 4. * (phase - 0.5).abs(),
                Wave::Square => {
                    if phase < 0.5 {
                        1.
                    } else {
                        -1.
                    }
                }
                Wave::Saw => phase * 2. - 1.,
                Wave::Noise => low,
            };
            let envelope = (t / l.attack.max(f64::EPSILON)).min(1.) * (-l.decay * u).exp() * (1. - u);
            *sample += wave * envelope * l.gain;
        }
    }
    let len = (samples.len() * 2) as u32;
    let mut bytes = Vec::with_capacity(44 + len as usize);
    bytes.extend(b"RIFF");
    bytes.extend((36 + len).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16_u32.to_le_bytes());
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend(a.sample_rate.to_le_bytes());
    bytes.extend((a.sample_rate * 2).to_le_bytes());
    bytes.extend(2_u16.to_le_bytes());
    bytes.extend(16_u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(len.to_le_bytes());
    for sample in samples {
        bytes.extend(((sample.tanh() * f64::from(i16::MAX)) as i16).to_le_bytes());
    }
    bytes
}

fn navigation(screen: Res<State<Screen>>, m: Option<Res<Match>>, devs: Res<Devices>, mut bank: ResMut<Bank>) {
    if *screen.get() == Screen::Match && m.as_ref().is_none_or(|m| m.paused.is_none()) {
        return;
    }
    for (_, nav) in devs.navs() {
        let name = match nav {
            Nav::Confirm | Nav::Start => "confirm",
            Nav::Back => "back",
            _ => "menu",
        };
        bank.queue(name, None, None);
    }
}

fn selection(
    screen: Res<State<Screen>>,
    sel: Option<Res<Select>>,
    mut prev: Local<Vec<(crate::input::Dev, bool)>>,
    mut bank: ResMut<Bank>,
) {
    if *screen.get() != Screen::Select {
        prev.clear();
        return;
    }
    let Some(sel) = sel else { return };
    let now: Vec<_> = sel.slots.iter().filter_map(|p| p.dev.map(|d| (d, p.ready))).collect();
    for &(dev, ready) in &now {
        match prev.iter().find(|(d, _)| *d == dev) {
            None => {
                bank.pending.remove("confirm");
                bank.queue("join", None, None);
            }
            Some((_, false)) if ready => {
                bank.pending.remove("confirm");
                bank.queue("ready", None, None);
            }
            _ => {}
        }
    }
    *prev = now;
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn playback(
    mut commands: Commands,
    mut bank: ResMut<Bank>,
    data: Res<Data>,
    saved: Res<settings::Saved>,
    time: Res<Time<Real>>,
    frame: Res<render::Framing>,
    m: Option<ResMut<Match>>,
    mut voices: Query<(Entity, &Voice, Option<&mut AudioSink>, Option<&mut SpatialAudioSink>)>,
) {
    let a = &data.0.global.audio;
    let volume = saved.volume.as_ref().unwrap_or(&a.defaults);
    let gain = (volume.master.clamp(0., 1.) * volume.sfx.clamp(0., 1.)) as f32;
    let now = time.elapsed_secs_f64();
    let mut active = Vec::new();
    for (e, v, sink, spatial) in &mut voices {
        if now >= v.expires {
            commands.entity(e).try_despawn();
        } else {
            active.push((e, v.priority));
            if let Some(mut s) = sink {
                s.set_volume(Volume::Linear(v.gain * gain));
            }
            if let Some(mut s) = spatial {
                s.set_volume(Volume::Linear(v.gain * gain));
            }
        }
    }
    if let Some(mut m) = m {
        let audible = m.paused.is_none() && matches!(m.mode, Mode::Versus | Mode::Training);
        for fx in std::mem::take(&mut m.sounds) {
            if audible && let Fx::Sfx { name, arg, x } = fx {
                bank.queue(&name, arg, x);
            }
        }
    }
    let mut pending: Vec<_> = std::mem::take(&mut bank.pending).into_iter().collect();
    pending.sort_by_key(|(n, _)| std::cmp::Reverse(a.sounds.get(n).map_or(0, |s| s.priority)));
    let mut count = 0;
    for (name, (arg, x)) in pending {
        let Some(sound) = a.sounds.get(&name) else {
            if bank.unknown.insert(name.clone()) {
                warn!("unknown sound: {name}");
            }
            continue;
        };
        if gain == 0. || count >= a.max_per_frame {
            continue;
        }
        if bank.last.get(&name).is_some_and(|t| now - t < a.cooldown.max(sound.cooldown)) {
            continue;
        }
        if active.len() >= a.max_voices {
            let Some((i, &(e, priority))) = active.iter().enumerate().min_by_key(|(_, (_, p))| *p) else { continue };
            if priority >= sound.priority {
                continue;
            }
            commands.entity(e).try_despawn();
            active.swap_remove(i);
        }
        let strength = arg.map_or(1., |k| (k / a.strength_max).clamp(0., 1.));
        let index = (strength * a.strength_levels.saturating_sub(1) as f64).round() as usize;
        let Some(clip) = bank.clips.get(&name).and_then(|v| v.get(index)).cloned() else { continue };
        bank.rng = bank.rng.wrapping_mul(747_796_405).wrapping_add(1);
        let pitch = 1. + (f64::from(bank.rng) / f64::from(u32::MAX) * 2. - 1.) * a.pitch_variation;
        let local_gain = (sound.volume * (sound.strength_gain + strength * (1. - sound.strength_gain))) as f32;
        let pan = x.map_or(0., |x| {
            let [left, _, right, _] = frame.view;
            if right <= left {
                return 0.;
            }
            ((x - left) / (right - left) * 2. - 1.).clamp(-1., 1.) * a.pan
        });
        let duration = sound.layers.iter().map(|l| l.duration).fold(0., f64::max)
            * (1. + strength * (sound.strength_duration - 1.))
            / pitch;
        let e = commands
            .spawn((
                AudioPlayer::new(clip),
                PlaybackSettings {
                    volume: Volume::Linear(local_gain * gain),
                    speed: pitch as f32,
                    spatial: x.is_some(),
                    ..PlaybackSettings::DESPAWN
                },
                Transform::from_xyz(pan as f32, 0., a.depth as f32),
                // Also expires queued players when Bevy cannot open an output device.
                Voice { gain: local_gain, expires: now + duration, priority: sound.priority },
            ))
            .id();
        active.push((e, sound.priority));
        bank.last.insert(name, now);
        count += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> sim::GameData {
        let (data, errors) = crate::load_data(&[std::path::PathBuf::from("../../assets")]);
        assert!(errors.is_empty(), "{errors:?}");
        data
    }

    #[test]
    fn bank_covers_sim_content_and_ui() {
        let d = data();
        let sounds = &d.global.audio.sounds;
        for name in [
            "jump",
            "djump",
            "swing",
            "smash",
            "charge",
            "hit",
            "shoot",
            "slash",
            "counter",
            "whack",
            "toss",
            "bonk",
            "jet",
            "quake",
            "ko",
            "land",
            "grab",
            "pummel",
            "throw",
            "shieldHit",
            "shieldBreak",
            "tech",
            "roll",
            "menu",
            "beep",
            "go",
            "confirm",
            "back",
            "join",
            "ready",
        ] {
            assert!(sounds.contains_key(name), "missing {name}");
        }
        fn walk(v: &serde_json::Value, sounds: &BTreeMap<String, Sound>) {
            match v {
                serde_json::Value::Object(o) => {
                    for (k, v) in o {
                        if matches!(k.as_str(), "sfx" | "spawnSfx" | "fullSfx")
                            && let Some(name) = v.as_str()
                        {
                            assert!(sounds.contains_key(name), "unmapped content sound: {name}");
                        }
                        walk(v, sounds);
                    }
                }
                serde_json::Value::Array(a) => {
                    for v in a {
                        walk(v, sounds);
                    }
                }
                _ => {}
            }
        }
        for fighter in &d.fighters {
            walk(&serde_json::to_value(fighter).unwrap(), sounds);
        }
    }

    #[test]
    fn synth_produces_bounded_click_free_pcm_and_scales_hits() {
        let d = data();
        let a = &d.global.audio;
        for sound in a.sounds.values() {
            for strength in [0., 1.] {
                let bytes = wav(a, sound, strength);
                assert_eq!(&bytes[..4], b"RIFF");
                assert_eq!(u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as usize, bytes.len() - 44);
                let pcm: Vec<_> = bytes[44..].as_chunks::<2>().0.iter().map(|b| i16::from_le_bytes(*b)).collect();
                assert_eq!(pcm[0], 0);
                assert!(pcm.last().unwrap().abs() < 64, "release must approach silence");
                assert!(pcm.iter().any(|x| x.abs() > 100), "sound must be audible");
            }
        }
        assert!(wav(a, &a.sounds["hit"], 1.).len() > wav(a, &a.sounds["hit"], 0.).len());
    }

    #[test]
    fn fast_ticks_coalesce_to_strongest_hit_with_its_position() {
        let mut bank = Bank::default();
        bank.queue("hit", Some(8.), Some(-20.));
        bank.queue("hit", Some(30.), Some(40.));
        bank.queue("hit", Some(4.), Some(5.));
        assert_eq!(bank.pending.len(), 1);
        assert_eq!(bank.pending["hit"], (Some(30.), Some(40.)));
    }

    #[test]
    fn unavailable_output_expires_queued_voices_and_mute_discards_events() {
        let mut data = data();
        data.global.audio.max_voices = 1;
        let mut app = App::new();
        app.insert_resource(Data(std::sync::Arc::new(data)))
            .init_resource::<Bank>()
            .init_resource::<Assets<AudioSource>>()
            .init_resource::<Time<Real>>()
            .init_resource::<render::Framing>()
            .init_resource::<settings::Saved>()
            .add_systems(Startup, synthesize)
            .add_systems(Update, playback);
        // No AudioPlugin/output/sinks: queued entities must remain bounded and expire anyway.
        app.world_mut().resource_mut::<Bank>().queue("jet", None, None);
        app.update();
        app.world_mut().resource_mut::<Bank>().queue("ko", None, Some(0.));
        app.update();
        let count = |app: &mut App| app.world_mut().query_filtered::<Entity, With<Voice>>().iter(app.world()).count();
        assert_eq!(count(&mut app), 1);
        let voice = app.world_mut().query::<&Voice>().single(app.world()).unwrap();
        assert_eq!(voice.priority, 100, "KO must replace a movement voice");
        app.world_mut().resource_mut::<Time<Real>>().advance_by(std::time::Duration::from_secs(10));
        app.update();
        assert_eq!(count(&mut app), 0);
        let mut volume = app.world().resource::<Data>().0.global.audio.defaults.clone();
        volume.master = 0.;
        app.world_mut().resource_mut::<settings::Saved>().volume = Some(volume);
        app.world_mut().resource_mut::<Bank>().queue("hit", Some(10.), None);
        app.update();
        assert_eq!(count(&mut app), 0);
        assert!(app.world().resource::<Bank>().pending.is_empty());
    }

    #[test]
    fn old_settings_use_content_defaults_and_volumes_roundtrip() {
        let mut saved: settings::Saved = serde_json::from_str(r#"{"fullscreen":true}"#).unwrap();
        assert!(saved.volume.is_none());
        saved.volume = Some(data().global.audio.defaults);
        let json = serde_json::to_string(&saved).unwrap();
        assert_eq!(serde_json::from_str::<settings::Saved>(&json).unwrap(), saved);
    }
}
