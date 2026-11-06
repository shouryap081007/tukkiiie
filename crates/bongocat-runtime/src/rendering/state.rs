//! What the renderer is drawing and playing right now.
//!
//! The renderer holds one model and independently timed motion layers. A motion
//! that has finished holds
//! its terminal parameters rather than snapping to the model's defaults, because a
//! model that visibly springs back the instant its motion ends is a model a user
//! would describe as glitching.

use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RenderMotionUserDataOccurrence {
    pub(crate) motion: MotionId,
    pub(crate) cycle: u64,
    pub(crate) local_time: Duration,
    pub(crate) value: String,
}

pub(crate) struct RuntimeRenderer {
    pub(crate) producer: RenderProducer,
    pub(crate) model_settings: ModelSettings,
    pub(crate) next_model_generation: u64,
    pub(crate) next_transport_sequence: u64,
    pub(crate) active: Option<ActiveRenderModel>,
    pub(crate) pending: Option<ActiveRenderModel>,
}

pub(crate) struct RuntimeRenderBootstrap {
    pub(crate) producer: RenderProducer,
}

pub(crate) struct ActiveRenderModel {
    pub(crate) model: Live2dModel,
    pub(crate) resources: Arc<bongocat_render::RenderResources>,
    pub(crate) book_assets: Option<TukiBookAssets>,
    pub(crate) book_animation: TukiBookAnimation,
    pub(crate) model_generation: u64,
    pub(crate) next_frame_number: u64,
    pub(crate) last_evaluated_at: Option<Duration>,
    pub(crate) motions: Vec<MotionPlayback>,
    pub(crate) expressions: Vec<ExpressionPlayback>,
}

pub(crate) struct MotionPlayback {
    pub(crate) motion: MotionId,
    pub(crate) clip: MotionClip,
    pub(crate) looping: bool,
    pub(crate) started_at: Duration,
    pub(crate) completed: bool,
    pub(crate) fade_out_started_at: Option<Duration>,
    pub(crate) last_event_elapsed: Option<Duration>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MotionStopStatus {
    Fading,
    Finished,
}

pub(crate) struct ExpressionPlayback {
    pub(crate) clip: ExpressionClip,
    pub(crate) started_at: Duration,
    pub(crate) fade_in_completed: bool,
    pub(crate) fade_out_started_at: Option<Duration>,
}


/// Four optional textures attached to built-in models and drawn via the same
/// native textured-quad path used by key images.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TukiBookAssets {
    pub(crate) closed: bongocat_render::KeyAssetId,
    pub(crate) opening: bongocat_render::KeyAssetId,
    pub(crate) open: bongocat_render::KeyAssetId,
    pub(crate) turning: bongocat_render::KeyAssetId,
}

impl TukiBookAssets {
    pub(crate) fn from_resources(resources: &bongocat_render::RenderResources) -> Option<Self> {
        use bongocat_live2d_render::TUKI_BOOK_ASSET_NAMES as NAMES;

        let find = |name: &str| {
            resources
                .key_assets
                .iter()
                .find(|asset| asset.name == name)
                .map(|asset| asset.id)
        };
        Some(Self {
            closed: find(NAMES[0])?,
            opening: find(NAMES[1])?,
            open: find(NAMES[2])?,
            turning: find(NAMES[3])?,
        })
    }
}

/// Monotonic, renderer-owned book animation state. Imported models without the
/// optional book textures simply omit the prop.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct TukiBookAnimation {
    open: bool,
    opened_at: Option<Duration>,
    transition: Option<TukiBookTransition>,
}

#[derive(Clone, Copy, Debug)]
enum TukiBookTransition {
    Opening { started_at: Duration },
    Closing { started_at: Duration },
}

impl TukiBookAnimation {
    pub(crate) fn set_open(&mut self, open: bool, now: Duration) {
        if self.open == open {
            return;
        }
        self.open = open;
        self.transition = Some(if open {
            self.opened_at = Some(now);
            TukiBookTransition::Opening { started_at: now }
        } else {
            self.opened_at = None;
            TukiBookTransition::Closing { started_at: now }
        });
    }

    pub(crate) fn asset_at(
        self,
        assets: &TukiBookAssets,
        now: Duration,
    ) -> bongocat_render::KeyAssetId {
        if let Some(transition) = self.transition {
            match transition {
                TukiBookTransition::Opening { started_at } => {
                    let elapsed = now.saturating_sub(started_at);
                    if elapsed < Duration::from_millis(120) {
                        return assets.closed;
                    }
                    if elapsed < Duration::from_millis(460) {
                        return assets.opening;
                    }
                }
                TukiBookTransition::Closing { started_at } => {
                    if now.saturating_sub(started_at) < Duration::from_millis(180) {
                        return assets.opening;
                    }
                    return assets.closed;
                }
            }
        }

        if !self.open {
            return assets.closed;
        }

        let opened_at = self.opened_at.unwrap_or(now);
        let elapsed_ms = now.saturating_sub(opened_at).as_millis() as u64;
        let interval_ms = 12_000 + (opened_at.as_millis() as u64 % 8_000);
        if elapsed_ms >= interval_ms && elapsed_ms % interval_ms < 340 {
            assets.turning
        } else {
            assets.open
        }
    }
}
