use core_math::Vec3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Quat {
    pub const IDENTITY: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 1.0,
    };

    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    pub fn norm_squared(self) -> f32 {
        self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w
    }

    fn multiply(self, right: Self) -> Self {
        Self {
            x: self.w * right.x + self.x * right.w + self.y * right.z - self.z * right.y,
            y: self.w * right.y - self.x * right.z + self.y * right.w + self.z * right.x,
            z: self.w * right.z + self.x * right.y - self.y * right.x + self.z * right.w,
            w: self.w * right.w - self.x * right.x - self.y * right.y - self.z * right.z,
        }
    }

    fn rotate(self, vector: Vec3) -> Vec3 {
        let axis = Vec3::new(self.x, self.y, self.z);
        let twice_cross = axis.cross(vector) * 2.0;
        vector + twice_cross * self.w + axis.cross(twice_cross)
    }

    fn inverse(self) -> Self {
        Self::new(-self.x, -self.y, -self.z, self.w)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntityTransform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl EntityTransform {
    pub const IDENTITY: Self = Self {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };

    pub const fn at(translation: Vec3) -> Self {
        Self {
            translation,
            ..Self::IDENTITY
        }
    }

    pub fn compose(self, local: Self) -> Self {
        let scaled_local = Vec3::new(
            local.translation.x * self.scale.x,
            local.translation.y * self.scale.y,
            local.translation.z * self.scale.z,
        );
        Self {
            translation: self.translation + self.rotation.rotate(scaled_local),
            rotation: self.rotation.multiply(local.rotation),
            scale: Vec3::new(
                self.scale.x * local.scale.x,
                self.scale.y * local.scale.y,
                self.scale.z * local.scale.z,
            ),
        }
    }

    pub(crate) fn relative_to(self, world: Self) -> Self {
        let inverse_rotation = self.rotation.inverse();
        let offset = inverse_rotation.rotate(world.translation - self.translation);
        Self {
            translation: Vec3::new(
                offset.x / self.scale.x,
                offset.y / self.scale.y,
                offset.z / self.scale.z,
            ),
            rotation: inverse_rotation.multiply(world.rotation),
            scale: Vec3::new(
                world.scale.x / self.scale.x,
                world.scale.y / self.scale.y,
                world.scale.z / self.scale.z,
            ),
        }
    }
}
