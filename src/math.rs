#![allow(dead_code)]
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

pub const TWO_PI: f32 = 2.0 * PI;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };

    #[inline]
    pub fn new(x: f32, y: f32) -> Self {
        Vec2 { x, y }
    }

    #[inline]
    pub fn length_squared(self) -> f32 {
        self.x * self.x + self.y * self.y
    }

    #[inline]
    pub fn length(self) -> f32 {
        self.length_squared().sqrt()
    }

    #[inline]
    pub fn distance_squared(self, other: Vec2) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        dx * dx + dy * dy
    }

    #[inline]
    pub fn distance(self, other: Vec2) -> f32 {
        self.distance_squared(other).sqrt()
    }

    #[inline]
    pub fn normalized(self) -> Vec2 {
        let len = self.length();
        if len > 1e-6 {
            Vec2::new(self.x / len, self.y / len)
        } else {
            Vec2::ZERO
        }
    }

    #[inline]
    pub fn from_angle(radians: f32) -> Vec2 {
        Vec2::new(radians.cos(), radians.sin())
    }

    #[inline]
    pub fn to_angle(self) -> f32 {
        self.y.atan2(self.x)
    }

    #[inline]
    pub fn dot(self, other: Vec2) -> f32 {
        self.x * other.x + self.y * other.y
    }

    #[inline]
    pub fn clamp_length(self, max_len: f32) -> Vec2 {
        let sq = self.length_squared();
        if sq > max_len * max_len && max_len > 0.0 {
            let scale = max_len / sq.sqrt();
            Vec2::new(self.x * scale, self.y * scale)
        } else {
            self
        }
    }
}

impl std::ops::Add for Vec2 {
    type Output = Vec2;
    #[inline]
    fn add(self, rhs: Vec2) -> Vec2 {
        Vec2::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl std::ops::Sub for Vec2 {
    type Output = Vec2;
    #[inline]
    fn sub(self, rhs: Vec2) -> Vec2 {
        Vec2::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl std::ops::Mul<f32> for Vec2 {
    type Output = Vec2;
    #[inline]
    fn mul(self, rhs: f32) -> Vec2 {
        Vec2::new(self.x * rhs, self.y * rhs)
    }
}

impl std::ops::Div<f32> for Vec2 {
    type Output = Vec2;
    #[inline]
    fn div(self, rhs: f32) -> Vec2 {
        Vec2::new(self.x / rhs, self.y / rhs)
    }
}

impl std::ops::AddAssign for Vec2 {
    #[inline]
    fn add_assign(&mut self, rhs: Vec2) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

#[inline]
pub fn normalize_angle(mut angle: f32) -> f32 {
    while angle > PI {
        angle -= TWO_PI;
    }
    while angle < -PI {
        angle += TWO_PI;
    }
    angle
}

#[inline]
pub fn angle_diff(target: f32, current: f32) -> f32 {
    normalize_angle(target - current)
}

#[inline]
pub fn fast_sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

#[inline]
pub fn fast_tanh(x: f32) -> f32 {
    x.tanh()
}

#[inline]
pub fn clamp(val: f32, min: f32, max: f32) -> f32 {
    if val < min {
        min
    } else if val > max {
        max
    } else {
        val
    }
}
