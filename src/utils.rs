use bevy::prelude::*;

/// Updates visibility component only if the target visibility differs.
/// This prevents triggering Bevy's internal change-detection systems unnecessarily.
pub fn set_visibility(vis: &mut Visibility, target: Visibility) {
    if *vis != target {
        *vis = target;
    }
}
