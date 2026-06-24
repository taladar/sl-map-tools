//! Extended Environment (EEP) value types: the colour, glow, and cloud
//! parameters carried by a region's or parcel's sky and water settings.
//!
//! These are distinct named types (rather than bare `[f32; N]`) so a colour
//! cannot be transposed with a position, a direction, a scale, or a rotation —
//! all of which are also arrays of `f32`.

/// An RGB colour — three `f32` channels (normally `0.0..=1.0`, but HDR
/// environment colours can exceed `1.0`). A named type so a colour cannot be
/// transposed with a position, direction, or scale.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Color {
    /// The red channel.
    red: f32,
    /// The green channel.
    green: f32,
    /// The blue channel.
    blue: f32,
}

impl Color {
    /// Creates a colour from its red/green/blue channels.
    #[must_use]
    pub const fn new(red: f32, green: f32, blue: f32) -> Self {
        Self { red, green, blue }
    }

    /// The red channel.
    #[must_use]
    pub const fn red(&self) -> f32 {
        self.red
    }

    /// The green channel.
    #[must_use]
    pub const fn green(&self) -> f32 {
        self.green
    }

    /// The blue channel.
    #[must_use]
    pub const fn blue(&self) -> f32 {
        self.blue
    }
}

/// An RGBA colour — four `f32` channels (RGB plus an alpha channel). The
/// alpha-carrying sibling of [`Color`]; a distinct type so it can't be
/// transposed with a 3-channel colour, a position, or a rotation quaternion
/// (all of which are also arrays of `f32`). Its one wire user is the windlight
/// `sunlight_color`. Channels are normally `0.0..=1.0` but HDR values can
/// exceed `1.0`.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ColorAlpha {
    /// The red channel.
    red: f32,
    /// The green channel.
    green: f32,
    /// The blue channel.
    blue: f32,
    /// The alpha channel.
    alpha: f32,
}

impl ColorAlpha {
    /// Creates a colour from its red/green/blue/alpha channels.
    #[must_use]
    pub const fn new(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    /// The red channel.
    #[must_use]
    pub const fn red(&self) -> f32 {
        self.red
    }

    /// The green channel.
    #[must_use]
    pub const fn green(&self) -> f32 {
        self.green
    }

    /// The blue channel.
    #[must_use]
    pub const fn blue(&self) -> f32 {
        self.blue
    }

    /// The alpha channel.
    #[must_use]
    pub const fn alpha(&self) -> f32 {
        self.alpha
    }
}

/// A windlight sun/moon **glow** parameter. The wire packs it as a 3-vector
/// `(size, reserved, focus)` whose middle component is unused/reserved (the
/// viewer always sends `0`); it is preserved verbatim so a decode/encode round
/// trip is byte-identical. The meaningful channels are [`size`](Self::size) and
/// [`focus`](Self::focus).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Glow {
    /// The glow size.
    size: f32,
    /// The unused/reserved middle component (preserved for round-trip).
    reserved: f32,
    /// The glow focus.
    focus: f32,
}

impl Glow {
    /// Creates a glow from its wire `(size, reserved, focus)` components.
    #[must_use]
    pub const fn new(size: f32, reserved: f32, focus: f32) -> Self {
        Self {
            size,
            reserved,
            focus,
        }
    }

    /// The glow size.
    #[must_use]
    pub const fn size(&self) -> f32 {
        self.size
    }

    /// The unused/reserved middle component (normally `0`).
    #[must_use]
    pub const fn reserved(&self) -> f32 {
        self.reserved
    }

    /// The glow focus.
    #[must_use]
    pub const fn focus(&self) -> f32 {
        self.focus
    }
}

/// A windlight cloud layer's scroll **position** (X, Y) packed with its
/// **density** (Z) in one wire 3-vector (the viewer's `cloud_pos_density*`).
/// The three components are semantically distinct — two are a 2-D scroll offset,
/// one is a density — so they get named accessors rather than `x`/`y`/`z`, and
/// this type cannot be confused with a position or direction.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CloudPosDensity {
    /// The cloud-scroll x position.
    position_x: f32,
    /// The cloud-scroll y position.
    position_y: f32,
    /// The cloud density.
    density: f32,
}

impl CloudPosDensity {
    /// Creates a value from its wire `(position_x, position_y, density)`
    /// components.
    #[must_use]
    pub const fn new(position_x: f32, position_y: f32, density: f32) -> Self {
        Self {
            position_x,
            position_y,
            density,
        }
    }

    /// The cloud-scroll x position.
    #[must_use]
    pub const fn position_x(&self) -> f32 {
        self.position_x
    }

    /// The cloud-scroll y position.
    #[must_use]
    pub const fn position_y(&self) -> f32 {
        self.position_y
    }

    /// The cloud density.
    #[must_use]
    pub const fn density(&self) -> f32 {
        self.density
    }
}
