//! Free-fly camera in reference scene-local units.
//!
//! Orientation follows the reference matrices: `rotateX(pitch) * rotateY(yaw)`, with scene `x`
//! east, `z` north, and `y` pointing down (negative is up). Positive pitch looks downward.

use osrs_render::gpu::ReferenceCamera;

#[derive(Debug, Clone, Copy)]
pub struct FlyCamera {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub scale: f32,
    /// Movement speed in local units per second (128 = one tile).
    pub speed: f32,
}

impl FlyCamera {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self {
            x,
            y,
            z,
            yaw: 0.0,
            pitch: 0.75,
            scale: 512.0,
            speed: 900.0,
        }
    }

    pub fn reference(&self) -> ReferenceCamera {
        ReferenceCamera {
            x: self.x,
            y: self.y,
            z: self.z,
            yaw: self.yaw,
            pitch: self.pitch,
            scale: self.scale,
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
}
