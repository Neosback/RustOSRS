/// Reference interpretation of one raw semantic face-alpha byte on the normal
/// software face path.
///
/// The raw signed byte remains available for diagnostics. `rasterizer_alpha`
/// mirrors the pinned reference rule: absent data is 0, ordinary stored bytes
/// are interpreted unsigned, and raw `-1` is the special normal-path value 253.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceFaceAlpha {
    raw: Option<i8>,
    rasterizer_alpha: u8,
}

impl ReferenceFaceAlpha {
    pub const fn from_raw(raw: Option<i8>) -> Self {
        let rasterizer_alpha = match raw {
            None => 0,
            Some(-1) => 253,
            Some(value) => value as u8,
        };
        Self {
            raw,
            rasterizer_alpha,
        }
    }

    pub const fn raw(self) -> Option<i8> {
        self.raw
    }

    pub const fn rasterizer_alpha(self) -> u8 {
        self.rasterizer_alpha
    }

    pub const fn is_minus_one_sentinel(self) -> bool {
        matches!(self.raw, Some(-1))
    }

    pub const fn is_reference_opaque(self) -> bool {
        self.rasterizer_alpha == 0
    }
}
