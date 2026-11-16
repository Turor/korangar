//! Map weather driven by `ZC_NOTIFY_EFFECT2` (`0x01F3`) effect ids.
//!
//! Hercules `clif_weather_check` sends these on map load: snow 162, clouds 233,
//! clouds2 516, fog 515, fireworks 297 and 301, sakura 163, leaves 333. Rain
//! is 161 (no Hercules mapflag; roBrowser still plays it). `299` is
//! `EF_THROWITEM2`, sent beside fireworks, and is not weather.
//!
//! Particles are CPU motes drawn with the existing rectangle path. Logic mode
//! only keeps [`WeatherSet`]. Caps stay under the official rain emitter so a
//! frame does not need a compute pass.

use std::f32::consts::TAU;

use cgmath::{EuclideanSpace, Point3, Vector3};
#[cfg(feature = "debug")]
use korangar_debug::logging::print_debug;
use rand_aes::tls::rand_f32;

use super::Camera;
use crate::graphics::{Color, ScreenPosition, ScreenSize};
use crate::loaders::GAT_TILE_SIZE;
use crate::renderer::GameInterfaceRenderer;

/// Scatter around the player, in cells. Official snow uses ~60; this stays
/// nearer the camera.
const SCATTER_CELLS: f32 = 28.0;
/// Snow falls 0.1 cells per 25 ms rag tick.
const SNOW_FALL: f32 = 0.1 * GAT_TILE_SIZE * 40.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WeatherKind {
    Rain,
    Snow,
    Sakura,
    Clouds,
    Clouds2,
    Fog,
    Fireworks,
    Leaves,
}

/// `Some` when `effect_id` starts or refreshes a weather profile.
pub fn kind_from_effect_id(effect_id: u32) -> Option<WeatherKind> {
    Some(match effect_id {
        161 => WeatherKind::Rain,
        162 => WeatherKind::Snow,
        163 => WeatherKind::Sakura,
        229 | 230 | 233 => WeatherKind::Clouds,
        515 => WeatherKind::Fog,
        516 | 592 | 697 | 698 => WeatherKind::Clouds2,
        297 | 301 => WeatherKind::Fireworks,
        333 => WeatherKind::Leaves,
        _ => return None,
    })
}

/// Server weather set. `show` is the Game settings toggle.
#[derive(Clone, Debug)]
pub struct WeatherSet {
    active: Vec<WeatherKind>,
    show: bool,
}

impl Default for WeatherSet {
    fn default() -> Self {
        Self {
            active: Vec::new(),
            show: true,
        }
    }
}

impl WeatherSet {
    /// Records a weather effect. Returns true when the id is weather, so the
    /// caller skips the skill-effect path. A repeat id does not log again.
    pub fn note_effect(&mut self, effect_id: u32) -> bool {
        let Some(kind) = kind_from_effect_id(effect_id) else {
            return false;
        };
        if !self.active.contains(&kind) {
            self.active.push(kind);
            if self.show {
                log_weather(true, &[kind]);
            }
        }
        true
    }

    pub fn clear(&mut self) {
        if self.show && !self.active.is_empty() {
            log_weather(false, &self.active);
        }
        self.active.clear();
    }

    /// Shows or hides the set the server already sent. Logs only when the
    /// toggle changes and something is active.
    pub fn set_show(&mut self, show: bool) {
        if self.show == show {
            return;
        }
        self.show = show;
        if !self.active.is_empty() {
            log_weather(show, &self.active);
        }
    }

    pub fn active(&self) -> &[WeatherKind] {
        &self.active
    }

    pub fn visible(&self) -> &[WeatherKind] {
        if self.show { &self.active } else { &[] }
    }
}

fn log_weather(starting: bool, kinds: &[WeatherKind]) {
    #[cfg(feature = "debug")]
    if !kinds.is_empty() {
        let action = if starting { "start" } else { "stop" };
        print_debug!("weather {action}: {kinds:?}");
    }
    #[cfg(not(feature = "debug"))]
    let _ = (starting, kinds);
}

struct Mote {
    offset: Vector3<f32>,
    velocity: Vector3<f32>,
    size: ScreenSize,
    color: Color,
    kind: WeatherKind,
    phase: f32,
    life: f32,
}

/// CPU weather field. Empty until [`WeatherField::sync`] sees visible kinds.
pub struct WeatherField {
    motes: Vec<Mote>,
    center: Point3<f32>,
    fog: bool,
    time: f32,
}

impl Default for WeatherField {
    fn default() -> Self {
        Self {
            motes: Vec::new(),
            center: Point3::origin(),
            fog: false,
            time: 0.0,
        }
    }
}

impl WeatherField {
    pub fn clear(&mut self) {
        self.motes.clear();
        self.fog = false;
    }

    #[cfg(test)]
    fn mote_count(&self) -> usize {
        self.motes.len()
    }

    #[cfg(test)]
    fn fog_active(&self) -> bool {
        self.fog
    }

    #[cfg(test)]
    fn offsets_finite(&self) -> bool {
        self.motes
            .iter()
            .all(|mote| mote.offset.x.is_finite() && mote.offset.y.is_finite() && mote.offset.z.is_finite())
    }

    /// Fills each kind up to its cap and steps the motes. Safe with no map and
    /// no camera: this only moves numbers.
    pub fn sync(&mut self, kinds: &[WeatherKind], center: Point3<f32>, delta_time: f32) {
        self.center = if center.x.is_finite() && center.y.is_finite() && center.z.is_finite() {
            center
        } else {
            Point3::origin()
        };
        self.fog = kinds.contains(&WeatherKind::Fog);
        self.motes.retain(|mote| kinds.contains(&mote.kind));

        for kind in kinds {
            if *kind == WeatherKind::Fog {
                continue;
            }
            let cap = cap_for(*kind);
            let have = self.motes.iter().filter(|mote| mote.kind == *kind).count();
            for _ in have..cap {
                self.motes.push(spawn(*kind));
            }
        }

        let dt = if delta_time.is_finite() { delta_time.clamp(0.0, 0.25) } else { 0.0 };
        self.time = (self.time + dt).rem_euclid(10_000.0);
        let time = self.time;
        for mote in &mut self.motes {
            step_mote(mote, dt, time);
        }
    }

    pub fn render(&self, renderer: &GameInterfaceRenderer, camera: &dyn Camera, window_size: ScreenSize) {
        if window_size.width <= 1.0 || window_size.height <= 1.0 {
            return;
        }
        if self.fog {
            renderer.render_rectangle(
                ScreenPosition { left: 0.0, top: 0.0 },
                window_size,
                Color::rgba(0.75, 0.78, 0.82, 0.22),
            );
        }
        for mote in &self.motes {
            let world = Point3::new(
                self.center.x + mote.offset.x,
                self.center.y + mote.offset.y,
                self.center.z + mote.offset.z,
            );
            let clip = camera.view_projection_matrix() * world.to_homogeneous();
            if clip.w <= 0.1 || !clip.w.is_finite() {
                continue;
            }
            let screen = camera.clip_to_screen_space(clip);
            if !screen.x.is_finite() || !screen.y.is_finite() {
                continue;
            }
            let left = screen.x * window_size.width;
            let top = screen.y * window_size.height;
            if left < -80.0 || top < -80.0 || left > window_size.width + 80.0 || top > window_size.height + 80.0 {
                continue;
            }
            renderer.render_rectangle(ScreenPosition { left, top }, mote.size, mote.color);
        }
    }
}

fn cap_for(kind: WeatherKind) -> usize {
    match kind {
        WeatherKind::Rain => 180,
        WeatherKind::Snow => 160,
        WeatherKind::Sakura => 90,
        WeatherKind::Leaves => 70,
        WeatherKind::Clouds | WeatherKind::Clouds2 => 24,
        WeatherKind::Fireworks => 36,
        WeatherKind::Fog => 0,
    }
}

fn scatter() -> f32 {
    (rand_f32() * 2.0 - 1.0) * SCATTER_CELLS * GAT_TILE_SIZE
}

fn unit() -> f32 {
    rand_f32()
}

fn spawn(kind: WeatherKind) -> Mote {
    let spread = scatter();
    let depth = scatter();
    let (offset, velocity, size, color, life) = match kind {
        WeatherKind::Snow => (
            Vector3::new(spread, 8.0 + unit() * 40.0, depth),
            Vector3::new((unit() - 0.5) * 4.0, -SNOW_FALL, (unit() - 0.5) * 2.0),
            ScreenSize {
                width: 3.0 + unit() * 2.0,
                height: 3.0 + unit() * 2.0,
            },
            Color::rgba(0.95, 0.97, 1.0, 0.55 + unit() * 0.4),
            6.0 + unit() * 4.0,
        ),
        WeatherKind::Rain => (
            Vector3::new(spread, 20.0 + unit() * 50.0, depth),
            Vector3::new(-12.0, -SNOW_FALL * 5.5, 0.0),
            ScreenSize {
                width: 1.5,
                height: 10.0 + unit() * 6.0,
            },
            Color::rgba(0.65, 0.78, 0.95, 0.35 + unit() * 0.25),
            1.4 + unit(),
        ),
        WeatherKind::Sakura => (
            Vector3::new(spread, 16.0 + unit() * 36.0, depth),
            Vector3::new(6.0 + unit() * 4.0, -SNOW_FALL * 0.55, (unit() - 0.5) * 3.0),
            ScreenSize {
                width: 5.0 + unit() * 2.0,
                height: 4.0,
            },
            Color::rgba(1.0, 0.72, 0.8, 0.7 + unit() * 0.25),
            8.0 + unit() * 4.0,
        ),
        WeatherKind::Leaves => (
            Vector3::new(spread, 14.0 + unit() * 30.0, depth),
            Vector3::new(3.0 + unit() * 3.0, -SNOW_FALL * 0.35, (unit() - 0.5) * 2.0),
            ScreenSize {
                width: 6.0 + unit() * 2.0,
                height: 4.5,
            },
            Color::rgba(0.85, 0.45 + unit() * 0.2, 0.15, 0.75),
            9.0 + unit() * 4.0,
        ),
        WeatherKind::Clouds => (
            Vector3::new(spread, 28.0 + unit() * 18.0, depth * 0.4),
            Vector3::new(3.0 + unit() * 2.0, 0.0, 0.4),
            ScreenSize {
                width: 46.0 + unit() * 20.0,
                height: 16.0 + unit() * 8.0,
            },
            Color::rgba(1.0, 1.0, 1.0, 0.16 + unit() * 0.1),
            30.0,
        ),
        WeatherKind::Clouds2 => (
            Vector3::new(spread, 22.0 + unit() * 16.0, depth * 0.3),
            Vector3::new(8.0 + unit() * 4.0, 0.0, 0.2),
            ScreenSize {
                width: 54.0 + unit() * 24.0,
                height: 18.0,
            },
            Color::rgba(0.9, 0.86, 0.78, 0.28 + unit() * 0.15),
            30.0,
        ),
        WeatherKind::Fireworks => (
            Vector3::new(spread * 0.5, 4.0 + unit() * 6.0, depth * 0.5),
            Vector3::new((unit() - 0.5) * 10.0, 28.0 + unit() * 24.0, (unit() - 0.5) * 10.0),
            ScreenSize {
                width: 3.0 + unit() * 2.0,
                height: 3.0 + unit() * 2.0,
            },
            Color::rgba(unit(), 0.4 + unit() * 0.6, 0.3 + unit() * 0.7, 0.9),
            0.8 + unit() * 0.8,
        ),
        WeatherKind::Fog => (
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 0.0),
            ScreenSize { width: 1.0, height: 1.0 },
            Color::TRANSPARENT,
            1.0,
        ),
    };
    Mote {
        offset,
        velocity,
        size,
        color,
        kind,
        phase: unit() * TAU,
        life,
    }
}

fn step_mote(mote: &mut Mote, dt: f32, time: f32) {
    let limit = SCATTER_CELLS * GAT_TILE_SIZE;
    match mote.kind {
        WeatherKind::Snow | WeatherKind::Rain | WeatherKind::Sakura | WeatherKind::Leaves => {
            let sway = match mote.kind {
                WeatherKind::Sakura | WeatherKind::Leaves => 10.0,
                WeatherKind::Snow => 4.0,
                _ => 0.0,
            };
            mote.offset.x += (mote.velocity.x + (time + mote.phase).sin() * sway) * dt;
            mote.offset.y += mote.velocity.y * dt;
            mote.offset.z += mote.velocity.z * dt;
            mote.life -= dt;
            if mote.offset.y < 0.0 || mote.life <= 0.0 || mote.offset.x.abs() > limit || mote.offset.z.abs() > limit {
                let fresh = spawn(mote.kind);
                mote.offset = fresh.offset;
                mote.life = fresh.life;
                mote.phase = fresh.phase;
            }
        }
        WeatherKind::Clouds | WeatherKind::Clouds2 => {
            mote.offset.x += mote.velocity.x * dt;
            mote.offset.y += (time * 0.3 + mote.phase).sin() * 1.5 * dt;
            if mote.offset.x > limit {
                mote.offset.x = -limit;
                mote.offset.z = scatter() * 0.4;
            }
        }
        WeatherKind::Fireworks => {
            mote.velocity.y -= 40.0 * dt;
            mote.offset += mote.velocity * dt;
            mote.life -= dt;
            if mote.life <= 0.0 || mote.offset.y < 0.0 {
                let fresh = spawn(WeatherKind::Fireworks);
                *mote = fresh;
            }
        }
        WeatherKind::Fog => {}
    }
    mote.offset.x = finite_or_zero(mote.offset.x);
    mote.offset.y = finite_or_zero(mote.offset.y);
    mote.offset.z = finite_or_zero(mote.offset.z);
}

fn finite_or_zero(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use cgmath::Point3;
    use ragnarok_packets::EffectId;

    use super::{WeatherField, WeatherKind, WeatherSet, kind_from_effect_id};
    use crate::world::effects::effect_id_value;

    #[test]
    fn effect_ids_map_to_weather_kinds() {
        assert_eq!(kind_from_effect_id(effect_id_value(EffectId::Rain)), Some(WeatherKind::Rain));
        assert_eq!(kind_from_effect_id(effect_id_value(EffectId::Snow)), Some(WeatherKind::Snow));
        assert_eq!(
            kind_from_effect_id(effect_id_value(EffectId::Sakura)),
            Some(WeatherKind::Sakura)
        );
        assert_eq!(
            kind_from_effect_id(effect_id_value(EffectId::Cloud3)),
            Some(WeatherKind::Clouds)
        );
        assert_eq!(
            kind_from_effect_id(effect_id_value(EffectId::Cloud5)),
            Some(WeatherKind::Clouds2)
        );
        assert_eq!(kind_from_effect_id(effect_id_value(EffectId::Cloud4)), Some(WeatherKind::Fog));
        assert_eq!(
            kind_from_effect_id(effect_id_value(EffectId::Pokjuk)),
            Some(WeatherKind::Fireworks)
        );
        assert_eq!(
            kind_from_effect_id(effect_id_value(EffectId::PokjukSound)),
            Some(WeatherKind::Fireworks)
        );
        assert_eq!(kind_from_effect_id(effect_id_value(EffectId::Maple)), Some(WeatherKind::Leaves));
        assert_eq!(kind_from_effect_id(162), Some(WeatherKind::Snow));
        assert_eq!(kind_from_effect_id(299), None);
        assert_eq!(kind_from_effect_id(effect_id_value(EffectId::Throwitem2)), None);
        assert_eq!(kind_from_effect_id(165), None);
    }

    #[test]
    fn show_toggle_hides_without_forgetting_the_server_set() {
        let mut set = WeatherSet::default();
        assert!(set.note_effect(162));
        assert!(set.note_effect(162));
        assert_eq!(set.active(), &[WeatherKind::Snow]);
        assert_eq!(set.visible(), &[WeatherKind::Snow]);
        assert!(!set.note_effect(299));
        assert!(set.note_effect(161));
        assert_eq!(set.active(), &[WeatherKind::Snow, WeatherKind::Rain]);

        set.set_show(false);
        assert!(set.visible().is_empty());
        assert_eq!(set.active(), &[WeatherKind::Snow, WeatherKind::Rain]);
        set.set_show(false);
        set.set_show(true);
        assert_eq!(set.visible(), &[WeatherKind::Snow, WeatherKind::Rain]);

        set.clear();
        assert!(set.active().is_empty());
        assert!(set.visible().is_empty());
    }

    #[test]
    fn snow_field_stays_finite_without_a_camera() {
        let mut field = WeatherField::default();
        let center = Point3::new(100.0, 0.0, 40.0);
        field.sync(&[WeatherKind::Snow], center, 0.016);
        assert!(field.mote_count() > 0);
        assert!(!field.fog_active());
        for _ in 0..240 {
            field.sync(&[WeatherKind::Snow, WeatherKind::Fog], center, 0.05);
        }
        assert!(field.offsets_finite());
        assert!(field.fog_active());
        assert!(field.mote_count() <= 160);
        field.sync(&[], center, 0.016);
        assert_eq!(field.mote_count(), 0);
        assert!(!field.fog_active());
        field.clear();
        assert_eq!(field.mote_count(), 0);
    }
}
