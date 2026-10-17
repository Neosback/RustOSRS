//! Camera state transitions. Callers collect input and resolve world coordinates; this owner
//! applies camera movement without exposing mutable camera internals.
use super::InteractionRuntime;
use crate::camera::{CameraMode, FlyCamera};

#[derive(Default)]
pub(in crate::app) struct CameraInput {
    pub delta: [f32; 2],
    pub pan: bool,
    pub secondary_drag: bool,
    pub shift: bool,
    pub scroll: f32,
    pub pinch: f32,
    pub hovered: bool,
    pub movement: [f32; 3],
    pub dt: f32,
}

impl InteractionRuntime {
    pub(in crate::app) fn camera(&self) -> &FlyCamera {
        &self.camera
    }
    pub(in crate::app) fn set_camera_mode(&mut self, mode: CameraMode) {
        self.camera.mode = mode;
    }
    pub(in crate::app) fn toggle_camera_mode(&mut self) {
        self.camera.mode = match self.camera.mode {
            CameraMode::Fly => CameraMode::Orbit,
            CameraMode::Orbit => CameraMode::Fly,
        };
    }
    pub(in crate::app) fn set_camera_settings(&mut self, fov: f32, speed: f32, mode: CameraMode) {
        self.camera.fov_degrees = fov;
        self.camera.speed = speed;
        self.camera.mode = mode;
    }
    pub(in crate::app) fn face_north(&mut self) {
        self.camera.yaw = 0.0;
    }
    pub(in crate::app) fn focus_camera(&mut self, target: [f32; 3], distance: f32) {
        self.camera.focus(target, distance);
    }
    pub(in crate::app) fn teleport_camera(&mut self, local: [f32; 3], plane: u8) {
        [self.camera.x, self.camera.y, self.camera.z] = local;
        self.set_view_plane(plane);
    }
    /// Set the view direction (radians) and, when given, the camera height (client units above
    /// the plane origin, positive up). Used by deterministic workload scripts.
    pub(in crate::app) fn set_camera_pose(&mut self, yaw: f32, pitch: f32, height: Option<f32>) {
        self.camera.yaw = yaw;
        self.camera.pitch = pitch;
        if let Some(height) = height {
            self.camera.y = -height;
        }
    }
    pub(in crate::app) fn shift_camera_origin(&mut self, delta: [f32; 2]) {
        self.camera.x += delta[0];
        self.camera.z += delta[1];
    }
    /// Inspection uses an independent camera with the author's initial pose.
    #[cfg(not(target_arch = "wasm32"))]
    pub(in crate::app) fn copy_view_from(&mut self, other: &Self) {
        self.camera = other.camera;
        self.view_plane = other.view_plane;
    }
    pub(in crate::app) fn apply_camera_input(&mut self, input: CameraInput) {
        let [dx, dy] = input.delta;
        match self.camera.mode {
            CameraMode::Fly => {
                if input.secondary_drag && !input.shift {
                    self.camera.look(dx, dy);
                }
                if input.pan {
                    self.camera.pan(dx, dy);
                }
            }
            CameraMode::Orbit => {
                if input.pan {
                    self.camera.pan(dx, dy);
                } else if input.secondary_drag {
                    self.camera.orbit(dx, dy);
                }
            }
        }
        match self.camera.mode {
            CameraMode::Fly if input.hovered && input.scroll != 0.0 => {
                if input.secondary_drag {
                    let delta_pct = (input.scroll * 0.002).clamp(-0.25, 0.25);
                    self.camera.speed =
                        (self.camera.speed * (1.0 + delta_pct)).clamp(50.0, 20_000.0);
                } else {
                    let f = self.camera.forward();
                    let step = input.scroll * (self.camera.speed * 0.002).max(1.0);
                    self.camera.x += f[0] * step;
                    self.camera.y += f[1] * step;
                    self.camera.z += f[2] * step;
                }
            }
            CameraMode::Orbit if input.hovered => {
                self.camera.zoom(input.scroll + input.pinch * 500.0)
            }
            _ => {}
        }
        let [forward, right, up] = input.movement;
        if input.movement != [0.0; 3] {
            self.camera.translate(forward, right, up, input.dt);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pose(camera: &FlyCamera) -> [f32; 8] {
        [
            camera.x,
            camera.y,
            camera.z,
            camera.yaw,
            camera.pitch,
            camera.speed,
            camera.orbit_distance,
            camera.fov_degrees,
        ]
    }
    #[test]
    fn camera_input_preserves_fly_and_orbit_gesture_ordering() {
        for mode in [CameraMode::Fly, CameraMode::Orbit] {
            for (pan, shift) in [(false, false), (true, false), (true, true)] {
                let mut expected = FlyCamera::new(500.0, -900.0, -300.0);
                expected.mode = mode;
                let mut runtime = InteractionRuntime::new(expected);
                match mode {
                    CameraMode::Fly => {
                        if !shift {
                            expected.look(12.0, -4.0);
                        }
                        if pan {
                            expected.pan(12.0, -4.0);
                        }
                        expected.speed =
                            (expected.speed * (1.0 + 30.0 * 0.002)).clamp(50.0, 20_000.0);
                    }
                    CameraMode::Orbit => {
                        if pan {
                            expected.pan(12.0, -4.0);
                        } else {
                            expected.orbit(12.0, -4.0);
                        }
                        expected.zoom(30.0 + 0.1 * 500.0);
                    }
                }
                expected.translate(1.0, -1.0, 0.0, 0.02);
                runtime.apply_camera_input(CameraInput {
                    delta: [12.0, -4.0],
                    pan,
                    secondary_drag: true,
                    shift,
                    scroll: 30.0,
                    pinch: 0.1,
                    hovered: true,
                    movement: [1.0, -1.0, 0.0],
                    dt: 0.02,
                });
                assert_eq!(pose(runtime.camera()), pose(&expected));
            }
        }
    }
    #[test]
    fn an_unhovered_idle_frame_does_not_recompute_the_orbit_pose() {
        let mut camera = FlyCamera::new(500.123, -900.456, -300.789);
        camera.mode = CameraMode::Orbit;
        let mut runtime = InteractionRuntime::new(camera);
        runtime.apply_camera_input(CameraInput::default());
        assert_eq!(pose(runtime.camera()), pose(&camera));
    }
    #[test]
    fn casual_scroll_in_fly_mode_dollies_camera_without_mutating_speed() {
        let camera = FlyCamera::new(0.0, -1000.0, 0.0);
        let initial_speed = camera.speed;
        let initial_z = camera.z;
        let mut runtime = InteractionRuntime::new(camera);

        runtime.apply_camera_input(CameraInput {
            scroll: 50.0,
            hovered: true,
            secondary_drag: false,
            ..Default::default()
        });

        assert_eq!(runtime.camera().speed, initial_speed);
        assert_ne!(runtime.camera().z, initial_z);
    }
}
