//! Actor slots are not dynamic-loc playback: movement loops from zero; actions
//! respect their repeat limit. Missing/invalid definitions deactivate the slot.
use super::RuntimeAssets;
use osrs_core::entity::ActorSequenceState;
pub(super) fn advance(slot: &mut ActorSequenceState, assets: &RuntimeAssets, movement: bool) {
    let Some(id) = slot.sequence else {
        return;
    };
    let Some(sequence) = assets.sequences.get(&id) else {
        slot.clear();
        return;
    };
    if let Some(range) = sequence
        .skeletal_range
        .filter(|_| sequence.skeletal_animation.is_some())
    {
        let length = range.end.saturating_sub(range.start) as u32;
        let frame = slot.frame.unwrap_or(0).saturating_add(1);
        if frame >= length {
            loop_slot(
                slot,
                length,
                sequence.frame_step,
                sequence.max_loops,
                movement,
            );
        } else {
            slot.frame = Some(frame);
        }
    } else {
        let frame = slot.frame.unwrap_or(0) as usize;
        let Some(&delay) = sequence.frame_delays.get(frame) else {
            slot.clear();
            return;
        };
        slot.frame_cycle += 1;
        if slot.frame_cycle > i32::from(delay) {
            slot.frame_cycle = 1;
            let next = frame + 1;
            if next >= sequence.frame_ids.len() {
                loop_slot(
                    slot,
                    sequence.frame_ids.len() as u32,
                    sequence.frame_step,
                    sequence.max_loops,
                    movement,
                );
            } else {
                slot.frame = Some(next as u32);
            }
        }
    }
}
fn loop_slot(
    slot: &mut ActorSequenceState,
    length: u32,
    step: Option<u16>,
    limit: u16,
    movement: bool,
) {
    if movement {
        slot.frame = Some(0);
        slot.frame_cycle = 0;
        return;
    }
    slot.loop_count += 1;
    let next = step.and_then(|s| length.checked_sub(u32::from(s)));
    if slot.loop_count >= i32::from(limit) || next.is_none_or(|n| n >= length) {
        slot.clear();
    } else {
        slot.frame = next;
    }
}
