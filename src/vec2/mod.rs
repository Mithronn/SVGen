#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DVec2 {
    pub x: f64,
    pub y: f64,
}

impl DVec2 {
    pub const EPS: f64 = 1e-8;
    /// All zeroes.
    pub const ZERO: Self = Self::splat(0.0);

    #[inline(always)]
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Creates a vector with all elements set to `v`.
    #[inline]
    #[must_use]
    pub const fn splat(v: f64) -> Self {
        Self { x: v, y: v }
    }

    /// Returns the square of a number.
    #[inline]
    #[must_use]
    pub fn sq(a: f64) -> f64 {
        a * a
    }

    /// Checks whether both components are finite.
    #[inline]
    #[must_use]
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }

    /// Returns the negated vector.
    #[inline]
    #[must_use]
    pub fn negated(self) -> Self {
        Self {
            x: -self.x,
            y: -self.y,
        }
    }

    /// Dot product of two vectors.
    #[inline]
    #[must_use]
    pub fn dot(self, other: Self) -> f64 {
        self.x * other.x + self.y * other.y
    }

    /// Adds two vectors.
    #[inline]
    #[must_use]
    pub fn add(self, other: Self) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }

    /// Subtracts `other` from self.
    #[inline]
    #[must_use]
    pub fn sub(self, other: Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
        }
    }

    /// Returns the midpoint between two vectors.
    #[inline]
    #[must_use]
    pub fn mid(self, other: Self) -> Self {
        Self {
            x: (self.x + other.x) * 0.5,
            y: (self.y + other.y) * 0.5,
        }
    }

    /// Linear interpolation between self and other by factor `t`.
    #[inline]
    #[must_use]
    pub fn interp(self, other: Self, t: f64) -> Self {
        let s = 1.0 - t;
        Self {
            x: self.x * s + other.x * t,
            y: self.y * s + other.y * t,
        }
    }

    /// Multiply-add: self + (other * f).
    #[inline]
    #[must_use]
    pub fn madd(self, other: Self, f: f64) -> Self {
        Self {
            x: self.x + other.x * f,
            y: self.y + other.y * f,
        }
    }

    /// Multiply-subtract: self - (other * f).
    #[inline]
    #[must_use]
    pub fn msub(self, other: Self, f: f64) -> Self {
        Self {
            x: self.x - other.x * f,
            y: self.y - other.y * f,
        }
    }

    /// Multiplies the vector by a scalar.
    #[inline]
    #[must_use]
    pub fn mul(self, f: f64) -> Self {
        Self {
            x: self.x * f,
            y: self.y * f,
        }
    }

    /// Returns the squared length of the vector.
    #[inline]
    #[must_use]
    pub fn len_squared(self) -> f64 {
        Self::sq(self.x) + Self::sq(self.y)
    }

    /// Returns the length of the vector.
    #[inline]
    #[must_use]
    pub fn len(self) -> f64 {
        self.len_squared().sqrt()
    }

    #[inline]
    #[must_use]
    pub fn len_squared_with(self, other: Self) -> f64 {
        Self::sq(self.x - other.x) + Self::sq(self.y - other.y)
    }

    #[inline]
    #[must_use]
    pub fn len_with(self, other: Self) -> f64 {
        self.len_squared_with(other).sqrt()
    }

    #[inline]
    #[must_use]
    pub fn len_squared_negated_with(self, other: Self) -> f64 {
        Self::sq(self.x + other.x) + Self::sq(self.y + other.y)
    }

    #[inline]
    #[must_use]
    pub fn len_negated_with(self, other: Self) -> f64 {
        self.len_squared_negated_with(other).sqrt()
    }

    /// Normalizes the vector in-place.
    /// Returns the original length.
    #[inline]
    #[must_use]
    pub fn normalize(&mut self) -> f64 {
        let mut d = self.len_squared();
        if (d != 0.0)
            && ({
                d = d.sqrt();
                d
            } != 0.0)
        {
            *self = self.mul(1.0 / d);
        }

        d
    }

    /// Returns a normalized copy of the vector.
    #[inline]
    #[must_use]
    pub fn normalized(self) -> Self {
        let mut v = self;
        let _ = v.normalize();
        v
    }

    /// Returns the normalized difference (self - other).
    #[inline]
    #[must_use]
    pub fn normalized_diff(self, other: Self) -> Self {
        self.sub(other).normalized()
    }

    /// Returns the normalized difference (self - other) along with its original length.
    #[inline]
    #[must_use]
    pub fn normalized_diff_with_len(self, other: Self) -> (Self, f64) {
        let mut v = self.sub(other);
        let d = v.normalize();
        (v, d)
    }

    /// Checks if a value is almost zero.
    #[inline]
    #[must_use]
    pub fn is_almost_zero(val: f64) -> bool {
        val.abs() < Self::EPS
    }

    /// Projects self onto a normalized vector `proj`.
    #[inline]
    #[must_use]
    pub fn project_onto_normalized(self, proj: Self) -> Self {
        // Assumes `proj` is normalized.
        proj.mul(self.dot(proj))
    }

    /// Returns the component of self orthogonal to the normalized vector `plane`.
    #[inline]
    #[must_use]
    pub fn project_plane(self, plane: Self) -> Self {
        self.sub(self.project_onto_normalized(plane))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IVec2 {
    pub x: i32,
    pub y: i32,
}

impl IVec2 {
    /// All zeroes.
    pub const ZERO: Self = Self::splat(0);

    #[inline(always)]
    #[must_use]
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// Creates a vector with all elements set to `v`.
    #[inline]
    #[must_use]
    pub const fn splat(v: i32) -> Self {
        Self { x: v, y: v }
    }

    /// Casts all elements of `self` to `f64`.
    #[inline]
    #[must_use]
    pub fn as_dvec2(&self) -> DVec2 {
        DVec2::new(self.x as f64, self.y as f64)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct USizeVec2 {
    pub x: usize,
    pub y: usize,
}

impl USizeVec2 {
    /// All zeroes.
    pub const ZERO: Self = Self::splat(0);

    #[inline(always)]
    #[must_use]
    pub const fn new(x: usize, y: usize) -> Self {
        Self { x, y }
    }

    /// Creates a vector with all elements set to `v`.
    #[inline]
    #[must_use]
    pub const fn splat(v: usize) -> Self {
        Self { x: v, y: v }
    }

    /// Casts all elements of `self` to `usize`.
    #[inline]
    #[must_use]
    pub fn as_dvec2(&self) -> DVec2 {
        DVec2::new(self.x as f64, self.y as f64)
    }
}
