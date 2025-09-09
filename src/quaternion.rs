use crate::Vector;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quaternion {
    pub a: Vector,
    pub w: f32,
}

impl Quaternion {
    pub fn from_radians(a: Vector, angle: f32) -> Self {
        let angle = -angle / 2.0;
        let sin = angle.sin();
        let cos = angle.cos();

        Self { a: a * sin, w: cos }
    }

    pub fn normalize(self) -> Self {
        let d = (self.a.dot(self.a) + self.w * self.w).sqrt();

        Self {
            a: self.a / d,
            w: self.w / d,
        }
    }

    pub fn to_euler(self) -> Vector {
        let pitch = (2.0 * (self.w * self.a.x - self.a.y * self.a.z))
            .clamp(-1.0, 1.0)
            .asin();

        let yaw = (2.0 * (self.w * self.a.y + self.a.z * self.a.x))
            .atan2(1.0 - 2.0 * (self.a.x * self.a.x + self.a.y * self.a.y));

        let roll = (2.0 * (self.w * self.a.z + self.a.x * self.a.y))
            .atan2(1.0 - 2.0 * (self.a.x * self.a.x + self.a.z * self.a.z));

        let normalize = |angle: f32| {
            if angle < 0.0 {
                -angle
            } else {
                (2.0 * std::f32::consts::PI - angle).rem_euclid(2.0 * std::f32::consts::PI)
            }
        };

        Vector {
            x: normalize(pitch),
            y: normalize(yaw),
            z: normalize(roll),
        }
    }
}

impl Default for Quaternion {
    fn default() -> Self {
        Self {
            a: Vector::default(),
            w: 1.0,
        }
    }
}

impl std::ops::Mul for Quaternion {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        let a = self.a * rhs.w + rhs.a * self.w + self.a.cross(rhs.a);
        let w = self.w * rhs.w - self.a.dot(rhs.a);

        Self { a, w }
    }
}
