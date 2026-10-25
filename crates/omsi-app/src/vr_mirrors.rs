//! Budgeted headset mirror refresh. Newly visible mirrors get a current image;
//! hidden mirrors share one background update per second in total.

use crate::{Player, camera_util};
use glam::{DVec3, Mat4};
use omsi_render::{Camera, Scene, TextureId};

pub(crate) struct Plan {
    pub cameras: Vec<(omsi_vehicle::Camera, (DVec3, f32, f32, f32))>,
    pub visible: Vec<usize>,
}

impl Plan {
    pub(crate) fn new(
        p: &Player,
        scene: &Scene,
        textures: &[Option<TextureId>],
        views: Option<&[(Camera, Mat4); 2]>,
    ) -> Self {
        let cameras = camera_util::mirror_cameras(p, camera_util::driver_eye(p));
        let instances = std::iter::once(&p.render)
            .chain(p.trailer_renders.iter())
            .flat_map(|r| r.instances.iter().chain(r.far.iter()))
            .copied();
        let surfaces = visible_surfaces(scene, instances, textures, views);
        let visible = cameras
            .iter()
            .enumerate()
            .filter_map(|(i, (camera, _))| {
                // A CCTV camera need not be anywhere near the screen showing it.
                // Missing/unknown texture users stay live rather than freezing a feed.
                (camera.fixed || surfaces.get(i).copied().unwrap_or(true)).then_some(i)
            })
            .collect();
        Self { cameras, visible }
    }
}

fn visible_surfaces(
    scene: &Scene,
    instances: impl Iterator<Item = usize>,
    textures: &[Option<TextureId>],
    views: Option<&[(Camera, Mat4); 2]>,
) -> Vec<bool> {
    let Some(views) = views else {
        return vec![true; textures.len()];
    };
    let mut known = vec![false; textures.len()];
    let mut visible = vec![false; textures.len()];
    for id in instances {
        let Some(instance) = scene.instances.get(id) else {
            continue;
        };
        let Some(mesh) = scene.meshes.get(instance.mesh) else {
            continue;
        };
        let centre = instance.origin
            + instance
                .transform
                .transform_point3(mesh.bounds_center)
                .as_dvec3();
        let scale = instance
            .transform
            .x_axis
            .truncate()
            .length()
            .max(instance.transform.y_axis.truncate().length())
            .max(instance.transform.z_axis.truncate().length());
        let radius = mesh.bounds_radius * scale;
        let seen = instance.visible
            && !instance.mirror_only
            && (!centre.is_finite()
                || !radius.is_finite()
                || views.iter().any(|(camera, projection)| {
                    camera_util::mirror_in_projection(centre, radius, camera, *projection)
                }));
        for (i, texture) in textures.iter().enumerate() {
            let Some(texture) = texture else { continue };
            if instance.materials.iter().any(|&m| {
                scene
                    .materials
                    .get(m)
                    .is_some_and(|m| m.uses_texture(*texture))
            }) {
                known[i] = true;
                visible[i] |= seen;
            }
        }
    }
    visible
        .iter()
        .zip(known)
        .map(|(&v, known)| v || !known)
        .collect()
}

#[derive(Default)]
pub(crate) struct Schedule {
    bus: Option<u64>,
    visible: Vec<usize>,
    count: usize,
    turn: usize,
    hidden_turn: usize,
    background: f32,
}

impl Schedule {
    pub(crate) fn select(
        &mut self,
        bus: u64,
        count: usize,
        visible: &[usize],
        updates: usize,
        dt: f32,
        rate: f32,
    ) -> Vec<usize> {
        let visible: Vec<usize> = if rate < 0.0 {
            // -1 is the explicit every-camera override, including hidden feeds.
            (0..count).collect()
        } else {
            visible.iter().copied().filter(|i| *i < count).collect()
        };
        let reset = self.bus != Some(bus) || self.count != count;
        let mut due: Vec<usize> = if reset {
            (0..count).collect()
        } else {
            visible
                .iter()
                .copied()
                .filter(|i| *i < count && !self.visible.contains(i))
                .collect()
        };
        self.bus = Some(bus);
        self.count = count;
        self.visible = visible;
        // Initial images and newly visible mirrors take priority over regular
        // turns; they also count against this frame's available redraw budget.
        if rate != 0.0 {
            self.background = (self.background + dt.clamp(0.0, 0.25)).min(1.0);
            if self.background >= 1.0 && due.len() < updates && count > self.visible.len() {
                for _ in 0..count {
                    let i = self.hidden_turn % count;
                    self.hidden_turn = self.hidden_turn.wrapping_add(1);
                    if !self.visible.contains(&i) {
                        if !due.contains(&i) {
                            due.push(i);
                        }
                        self.background = 0.0;
                        break;
                    }
                }
            }
            for _ in 0..self.visible.len() {
                if due.len() >= updates.min(count) {
                    break;
                }
                let i = self.visible[self.turn % self.visible.len()];
                self.turn = self.turn.wrapping_add(1);
                if !due.contains(&i) {
                    due.push(i);
                }
            }
        } else if !reset {
            // A zero refresh rate explicitly freezes existing images.
            due.clear();
        }
        due
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initializes_all_then_prioritizes_newly_visible_mirrors() {
        let mut s = Schedule::default();
        assert_eq!(s.select(1, 4, &[0], 0, 0.0, 1.0), vec![0, 1, 2, 3]);
        assert!(s.select(1, 4, &[0], 0, 0.0, 1.0).is_empty());
        assert_eq!(s.select(1, 4, &[0, 3], 0, 0.0, 1.0), vec![3]);
        assert_eq!(s.select(2, 4, &[0], 0, 0.0, 1.0), vec![0, 1, 2, 3]);
    }

    #[test]
    fn hidden_mirrors_share_one_background_turn_per_second() {
        let mut s = Schedule::default();
        s.select(1, 3, &[0], 0, 0.0, 1.0);
        for _ in 0..3 {
            assert!(s.select(1, 3, &[0], 0, 0.25, 1.0).is_empty());
        }
        assert_eq!(s.select(1, 3, &[0], 1, 0.25, 1.0), vec![1]);
        for _ in 0..3 {
            assert!(s.select(1, 3, &[0], 0, 0.25, 1.0).is_empty());
        }
        assert_eq!(s.select(1, 3, &[0], 1, 0.25, 1.0), vec![2]);
    }

    #[test]
    fn negative_rate_keeps_every_camera_updating_even_when_hidden() {
        let mut s = Schedule::default();
        s.select(1, 3, &[0], 3, 0.0, -1.0);
        for _ in 0..3 {
            let mut due = s.select(1, 3, &[0], 3, 0.0, -1.0);
            due.sort_unstable();
            assert_eq!(due, vec![0, 1, 2]);
        }
        assert_eq!(s.select(1, 3, &[0], 3, 0.0, 90.0), vec![0]);
    }

    #[test]
    fn visible_turns_are_bounded_and_zero_rate_freezes() {
        let mut s = Schedule::default();
        s.select(1, 3, &[0, 2], 0, 0.0, 1.0);
        let a = s.select(1, 3, &[0, 2], 1, 0.0, 1.0);
        let b = s.select(1, 3, &[0, 2], 1, 0.0, 1.0);
        assert_eq!((a, b), (vec![0], vec![2]));
        assert!(s.select(1, 3, &[1], 3, 1.0, 0.0).is_empty());
        assert_eq!(s.select(1, 0, &[], 3, 1.0, 1.0), Vec::<usize>::new());
    }
}
