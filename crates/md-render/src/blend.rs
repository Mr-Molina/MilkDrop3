//! D3D9 alpha-blend states (`D3DRS_SRCBLEND` / `D3DRS_DESTBLEND`) as wgpu
//! blend states.
//!
//! Every pair the C++ sets, with its source lines (code/vis_milk2/;
//! `mfs` = milkdropfs.cpp):
//!
//! | [`BlendMode`] | SRCBLEND | DESTBLEND | C++ |
//! |---|---|---|---|
//! | `Off` | (`ALPHABLENDENABLE` FALSE) | | sprite mode 1 decal mfs:3686; every pass that disables blending |
//! | `Alpha` | `SRCALPHA` | `INVSRCALPHA` | mfs:1226-1227 (motion vectors), 1941-1942, 2135-2136 (warp blend), 2396-2397 (end of shapes), 2293-2294 (shapes), 2647-2648 (custom waves), 2678-2679, 2716-2717 (wave), 3313-3314 (darken centre), 3355-3356 (borders, `DrawSprites`), 3681-3682 (sprite mode 0), 3725-3726 (mode 4), 4408-4409 (comp blend); pluginshell.cpp:1829-1830 (dark box) |
//! | `AlphaAdditive` | `SRCALPHA` | `ONE` | additive shapes mfs:2293-2294, custom waves 2647-2648, wave 2716-2717 |
//! | `Additive` | `ONE` | `ONE` | sprite mode 2 mfs:3695-3696; video echo / gamma passes 4127-4128, 4179-4180; song title 4492-4494, 4705-4706; textmgr.cpp:637-638 (additive blit) |
//! | `Multiply` | `ZERO` | `SRCCOLOR` | none in the C++ (kept from the plan's list) |
//! | `Replace` | `ONE` | `ZERO` | video echo / first pass mfs:4089-4090, 4158-4159; textmgr.cpp:637-638 |
//! | `InvAlpha` | `INVSRCALPHA` | `SRCALPHA` | flipped-alpha blends mfs:1936-1937, 2130-2131, 4403-4404 |
//! | `SrcColor` | `SRCCOLOR` | `INVSRCCOLOR` | sprite mode 3 mfs:3702-3703 |
//! | `InvDestColor` | `INVDESTCOLOR` | `ZERO` | brighten mfs:4213-4214, 4223-4224; invert 4279-4280 |
//! | `DestColor` | `ZERO` | `DESTCOLOR` | brighten mfs:4218-4219; darken 4240-4241 |
//! | `ZeroInvDestColor` | `ZERO` | `INVDESTCOLOR` | solarize mfs:4261-4262 |
//! | `DestColorAdd` | `DESTCOLOR` | `ONE` | solarize mfs:4265-4266 |
//! | `ZeroInvSrcColor` | `ZERO` | `INVSRCCOLOR` | song title shadow mfs:4700-4701 |
//!
//! The C++ never sets `D3DRS_SEPARATEALPHABLENDENABLE` or `D3DRS_BLENDOP`,
//! so D3D9 applies the same factors to the alpha channel with `ADD`;
//! [`BlendMode::state`] mirrors that with identical colour and alpha
//! components. (For alpha, a colour factor such as `SRCCOLOR` reads the
//! source alpha in both APIs.)

/// A D3D9 `SRCBLEND`/`DESTBLEND` pair (see the module table).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlendMode {
    Off,
    /// `SRCALPHA`, `INVSRCALPHA`.
    Alpha,
    /// `SRCALPHA`, `ONE`.
    AlphaAdditive,
    /// `ONE`, `ONE`.
    Additive,
    /// `ZERO`, `SRCCOLOR`.
    Multiply,
    /// `ONE`, `ZERO`.
    Replace,
    /// `INVSRCALPHA`, `SRCALPHA`.
    InvAlpha,
    /// `SRCCOLOR`, `INVSRCCOLOR`.
    SrcColor,
    /// `INVDESTCOLOR`, `ZERO`.
    InvDestColor,
    /// `ZERO`, `DESTCOLOR`.
    DestColor,
    /// `ZERO`, `INVDESTCOLOR`.
    ZeroInvDestColor,
    /// `DESTCOLOR`, `ONE`.
    DestColorAdd,
    /// `ZERO`, `INVSRCCOLOR`.
    ZeroInvSrcColor,
}

impl BlendMode {
    /// Every mode, for iteration.
    pub const ALL: [BlendMode; 13] = [
        BlendMode::Off,
        BlendMode::Alpha,
        BlendMode::AlphaAdditive,
        BlendMode::Additive,
        BlendMode::Multiply,
        BlendMode::Replace,
        BlendMode::InvAlpha,
        BlendMode::SrcColor,
        BlendMode::InvDestColor,
        BlendMode::DestColor,
        BlendMode::ZeroInvDestColor,
        BlendMode::DestColorAdd,
        BlendMode::ZeroInvSrcColor,
    ];

    /// The (source, destination) factors; `None` for [`BlendMode::Off`].
    pub fn factors(self) -> Option<(wgpu::BlendFactor, wgpu::BlendFactor)> {
        use wgpu::BlendFactor as F;
        Some(match self {
            BlendMode::Off => return None,
            BlendMode::Alpha => (F::SrcAlpha, F::OneMinusSrcAlpha),
            BlendMode::AlphaAdditive => (F::SrcAlpha, F::One),
            BlendMode::Additive => (F::One, F::One),
            BlendMode::Multiply => (F::Zero, F::Src),
            BlendMode::Replace => (F::One, F::Zero),
            BlendMode::InvAlpha => (F::OneMinusSrcAlpha, F::SrcAlpha),
            BlendMode::SrcColor => (F::Src, F::OneMinusSrc),
            BlendMode::InvDestColor => (F::OneMinusDst, F::Zero),
            BlendMode::DestColor => (F::Zero, F::Dst),
            BlendMode::ZeroInvDestColor => (F::Zero, F::OneMinusDst),
            BlendMode::DestColorAdd => (F::Dst, F::One),
            BlendMode::ZeroInvSrcColor => (F::Zero, F::OneMinusSrc),
        })
    }

    /// The wgpu blend state: the same factors for colour and alpha with
    /// `Add` (D3D9 without `SEPARATEALPHABLENDENABLE`); `None` when off.
    pub fn state(self) -> Option<wgpu::BlendState> {
        self.factors().map(|(src_factor, dst_factor)| {
            let c = wgpu::BlendComponent {
                src_factor,
                dst_factor,
                operation: wgpu::BlendOperation::Add,
            };
            wgpu::BlendState { color: c, alpha: c }
        })
    }
}
