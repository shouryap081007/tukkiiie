//! Deterministic idle effects shared by every model preset.
//!
//! The same elapsed time always produces the same blink, so replay and frame
//! comparisons remain stable. The interval varies per four-second slot, while
//! eased close/open curves make the blink less mechanical than a square wave.

use super::*;

const BLINK_SLOT_SECONDS: f64 = 4.0;
const BLINK_FIRST_OFFSET_SECONDS: f64 = 0.4;
const BLINK_JITTER_SECONDS: f64 = 3.0;
const BLINK_CLOSE_SECONDS: f64 = 0.07;
const BLINK_HOLD_SECONDS: f64 = 0.04;
const BLINK_OPEN_SECONDS: f64 = 0.11;

pub(crate) fn apply_automatic_effects(
    model: &mut Live2dModel,
    now: Duration,
) -> Result<(), RuntimeRenderErrorCode> {
    let (breath_time, blink) = automatic_effect_values(now);
    model
        .apply_automatic_effects(breath_time, blink)
        .map_err(|error| map_live2d_error(error, RuntimeRenderErrorCode::ModelEvaluationFailed))?;
    Ok(())
}

pub(crate) fn automatic_effect_values(now: Duration) -> (Duration, f32) {
    (now, blink_value(now))
}

fn blink_slot_offset(slot: u64) -> f64 {
    BLINK_FIRST_OFFSET_SECONDS + BLINK_JITTER_SECONDS * stable_unit_interval(slot)
}

fn stable_unit_interval(seed: u64) -> f64 {
    let mut value = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    ((value ^ (value >> 31)) >> 11) as f64 / (1u64 << 53) as f64
}

fn ease_in_out(value: f64) -> f64 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

fn blink_value(now: Duration) -> f32 {
    let seconds = now.as_secs_f64();
    let slot = (seconds / BLINK_SLOT_SECONDS).floor() as u64;
    let blink_start = slot as f64 * BLINK_SLOT_SECONDS + blink_slot_offset(slot);
    let elapsed = seconds - blink_start;
    let total = BLINK_CLOSE_SECONDS + BLINK_HOLD_SECONDS + BLINK_OPEN_SECONDS;
    if !(0.0..=total).contains(&elapsed) {
        return 0.0;
    }

    let closed = if elapsed < BLINK_CLOSE_SECONDS {
        ease_in_out(elapsed / BLINK_CLOSE_SECONDS)
    } else if elapsed < BLINK_CLOSE_SECONDS + BLINK_HOLD_SECONDS {
        1.0
    } else {
        1.0 - ease_in_out(
            (elapsed - BLINK_CLOSE_SECONDS - BLINK_HOLD_SECONDS) / BLINK_OPEN_SECONDS,
        )
    };
    -(closed as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blink_timing_is_repeatable_but_not_fixed_to_the_same_offset() {
        assert_eq!(blink_slot_offset(7), blink_slot_offset(7));
        assert_ne!(blink_slot_offset(7), blink_slot_offset(8));
    }

    #[test]
    fn blink_closes_and_opens_with_a_smooth_curve() {
        let start = blink_slot_offset(0);
        let at_start = Duration::from_secs_f64(start);
        let at_close = Duration::from_secs_f64(start + BLINK_CLOSE_SECONDS);
        let at_open = Duration::from_secs_f64(
            start + BLINK_CLOSE_SECONDS + BLINK_HOLD_SECONDS + BLINK_OPEN_SECONDS,
        );

        assert_eq!(blink_value(at_start), 0.0);
        assert_eq!(blink_value(at_close), -1.0);
        assert_eq!(blink_value(at_open), 0.0);
        assert!(blink_value(Duration::from_secs_f64(start + 0.025)) < 0.0);
    }

    #[test]
    fn blink_is_stable_for_a_replayed_timestamp() {
        let now = Duration::from_millis(8_123);
        assert_eq!(blink_value(now), blink_value(now));
    }
}
