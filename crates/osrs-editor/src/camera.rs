//! Free-fly camera in reference scene-local units.
//!
//! Orientation follows the reference matrices: `rotateX(pitch) * rotateY(yaw)`, with scene `x`
//! east, `z` north, and `y` pointing down (negative is up). Positive pitch looks downward.

use osrs_render::gpu::ReferenceCamera;

/// How mouse input moves the camera.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CameraMode {
    /// First-person: drag looks, WASD flies, scroll changes the speed.
    #[default]
    Fly,
    /// Around a pivot in front of the camera: drag orbits, Shift/right drag pans, scroll zooms.
    Orbit,
}

const MIN_ORBIT_DISTANCE: f32 = 100.0;
const MAX_ORBIT_DISTANCE: f32 = 60_000.0;

#[derive(Debug, Clone, Copy)]
pub struct FlyCamera {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,
    pub pitch: f32,
    /// Horizontal field of view in degrees.
    pub fov_degrees: f32,
    /// Movement speed in local units per second (128 = one tile).
    pub speed: f32,
    pub mode: CameraMode,
    /// Distance from the camera to the orbit pivot (the pivot is `position + forward * distance`).
    pub orbit_distance: f32,
}

impl FlyCamera {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self {
            x,
            y,
            z,
            yaw: 0.0,
            pitch: 0.75,
            fov_degrees: 60.0,
            speed: 900.0,
            mode: CameraMode::Fly,
            orbit_distance: 1500.0,
        }
    }

    /// Reference camera for a viewport `width` pixels wide. The reference projection maps
    /// `scale / distance` pixels per unit, so the scale that yields the requested horizontal
    /// field of view is `(width / 2) / tan(fov / 2)`.
    pub fn reference(&self, width: f32) -> ReferenceCamera {
        let half_fov = self.fov_degrees.to_radians() * 0.5;
        ReferenceCamera {
            x: self.x,
            y: self.y,
            z: self.z,
            yaw: self.yaw,
            pitch: self.pitch,
            scale: (width * 0.5) / half_fov.tan(),
        }
    }

    /// Unit view direction.
    pub fn forward(&self) -> [f32; 3] {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        [-sy * cp, sp, cy * cp]
    }

    /// Unit direction to the camera's right.
    pub fn right(&self) -> [f32; 3] {
        let (sy, cy) = self.yaw.sin_cos();
        [cy, 0.0, sy]
    }

    /// Turn by screen-space drag deltas (pixels). Dragging right turns right.
    pub fn look(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * 0.004;
        self.pitch = (self.pitch + dy * 0.004).clamp(-1.5, 1.5);
    }

    /// Move along the view axes: `forward`, `right`, `up` in `-1..=1`.
    pub fn translate(&mut self, forward: f32, right: f32, up: f32, dt: f32) {
        let f = self.forward();
        let r = self.right();
        let step = self.speed * dt;
        self.x += (f[0] * forward + r[0] * right) * step;
        self.y += (f[1] * forward + r[1] * right) * step - up * step;
        self.z += (f[2] * forward + r[2] * right) * step;
    }

    /// Direction toward the top of the screen (`y` is negative up).
    pub fn up(&self) -> [f32; 3] {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        [-sy * sp, -cp, cy * sp]
    }

    /// The point the orbit camera turns around.
    pub fn pivot(&self) -> [f32; 3] {
        let f = self.forward();
        [
            self.x + f[0] * self.orbit_distance,
            self.y + f[1] * self.orbit_distance,
            self.z + f[2] * self.orbit_distance,
        ]
    }

    /// Place the camera `distance` away from `target`, keeping the current orientation.
    pub fn focus(&mut self, target: [f32; 3], distance: f32) {
        self.orbit_distance = distance.clamp(MIN_ORBIT_DISTANCE, MAX_ORBIT_DISTANCE);
        let f = self.forward();
        self.x = target[0] - f[0] * self.orbit_distance;
        self.y = target[1] - f[1] * self.orbit_distance;
        self.z = target[2] - f[2] * self.orbit_distance;
    }

    /// Orbit around the pivot by screen-space drag deltas (pixels).
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        let pivot = self.pivot();
        self.look(dx, dy);
        self.focus(pivot, self.orbit_distance);
    }

    /// Slide the camera (and its pivot) in the screen plane by drag deltas (pixels); the scene
    /// follows the cursor.
    pub fn pan(&mut self, dx: f32, dy: f32) {
        let r = self.right();
        let u = self.up();
        let scale = self.orbit_distance * 0.0012;
        for axis in 0..3 {
            let delta = (-r[axis] * dx + u[axis] * dy) * scale;
            match axis {
                0 => self.x += delta,
                1 => self.y += delta,
                _ => self.z += delta,
            }
        }
    }

    /// Move toward (positive) or away from the pivot; the pivot stays fixed.
    pub fn zoom(&mut self, amount: f32) {
        let pivot = self.pivot();
        let distance = (self.orbit_distance * (1.0 - amount * 0.002))
            .clamp(MIN_ORBIT_DISTANCE, MAX_ORBIT_DISTANCE);
        self.focus(pivot, distance);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orbit_keeps_the_pivot_fixed() {
        let mut camera = FlyCamera::new(1000.0, -2000.0, 3000.0);
        let before = camera.pivot();
        camera.orbit(120.0, -40.0);
        camera.zoom(50.0);
        let after = camera.pivot();
        for axis in 0..3 {
            assert!((before[axis] - after[axis]).abs() < 0.5, "axis {axis}");
        }
    }

    #[test]
    fn pan_moves_the_pivot_with_the_camera() {
        let mut camera = FlyCamera::new(0.0, -2000.0, 0.0);
        let before = camera.pivot();
        camera.pan(100.0, 0.0);
        let after = camera.pivot();
        let moved = (0..3)
            .map(|a| (after[a] - before[a]).powi(2))
            .sum::<f32>()
            .sqrt();
        assert!(moved > 10.0);
        // Dragging right moves the camera left (scene follows the cursor).
        let right = camera.right();
        let shift = [after[0] - before[0], after[2] - before[2]];
        assert!(shift[0] * right[0] + shift[1] * right[2] < 0.0);
    }
}
