use core_assets::AssetKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetContext {
    DebugOverlay,
    CosmeticSurface,
    CollisionCritical,
    BackgroundDecoration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackVisual {
    MagentaSquare,
    GreyMaterial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackOutcome {
    UseFallback {
        reason: &'static str,
        visual: FallbackVisual,
    },
    FailClosed {
        reason: &'static str,
    },
    Skip {
        reason: &'static str,
    },
}

/// Resolve missing-asset behavior from both asset kind and use context.
pub fn fallback_for(kind: AssetKind, context: AssetContext) -> FallbackOutcome {
    match context {
        AssetContext::CollisionCritical => FallbackOutcome::FailClosed {
            reason: "collision-critical asset missing",
        },
        AssetContext::BackgroundDecoration => FallbackOutcome::Skip {
            reason: "non-critical background decoration omitted",
        },
        AssetContext::DebugOverlay => FallbackOutcome::UseFallback {
            reason: "debug overlay missing",
            visual: FallbackVisual::MagentaSquare,
        },
        AssetContext::CosmeticSurface => match kind {
            AssetKind::Sprite | AssetKind::SpriteSheet => FallbackOutcome::UseFallback {
                reason: "cosmetic sprite missing",
                visual: FallbackVisual::MagentaSquare,
            },
            _ => FallbackOutcome::UseFallback {
                reason: "cosmetic surface missing",
                visual: FallbackVisual::GreyMaterial,
            },
        },
    }
}
