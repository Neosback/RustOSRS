use osrs_core::model::FacePriority;

/// One visible face admitted to the reference priority-order preparation path.
///
/// `depth_bucket` is the integer bucket already produced by camera/depth
/// preparation. The ordering routine deliberately does not reinterpret model
/// geometry or camera state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferencePriorityFace {
    pub face_index: usize,
    pub priority: FacePriority,
    pub depth_bucket: i32,
}

impl ReferencePriorityFace {
    pub const fn new(face_index: usize, priority: FacePriority, depth_bucket: i32) -> Self {
        Self {
            face_index,
            priority,
            depth_bucket,
        }
    }
}

/// Diagnostic thresholds used by the audited reference priority algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferencePriorityThresholds {
    pub avg12: i64,
    pub avg34: i64,
    pub avg68: i64,
}

/// Deterministic CPU face-emission plan for `FACE-002`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferencePriorityOrder {
    ordered_face_indices: Vec<usize>,
    thresholds: ReferencePriorityThresholds,
    priority_counts: [usize; 12],
}

impl ReferencePriorityOrder {
    pub fn ordered_face_indices(&self) -> &[usize] {
        &self.ordered_face_indices
    }

    pub const fn thresholds(&self) -> ReferencePriorityThresholds {
        self.thresholds
    }

    pub const fn priority_counts(&self) -> &[usize; 12] {
        &self.priority_counts
    }
}

/// Reproduce the reference priority-queue emission order used by the pinned
/// `Model.method5946` oracle.
///
/// Faces are first traversed by descending depth bucket, with face index as the
/// deterministic equal-depth tie breaker. They are then grouped into priorities
/// `0..=11`. Priorities 10 and 11 form consecutive special queues that are
/// interleaved before ordinary priority bands 0, 3, and 5 using the audited
/// `(1,2)`, `(3,4)`, and `(6,8)` average-depth thresholds. Priority 11 is not
/// considered until priority 10 is exhausted.
pub fn prepare_reference_priority_order(
    faces: &[ReferencePriorityFace],
) -> ReferencePriorityOrder {
    let mut depth_ordered = faces.to_vec();
    depth_ordered.sort_by(|left, right| {
        right
            .depth_bucket
            .cmp(&left.depth_bucket)
            .then_with(|| left.face_index.cmp(&right.face_index))
    });

    let mut queues: [Vec<ReferencePriorityFace>; 12] = std::array::from_fn(|_| Vec::new());
    for face in depth_ordered {
        queues[usize::from(face.priority.get())].push(face);
    }

    let thresholds = ReferencePriorityThresholds {
        avg12: average_depth(&queues, 1, 2),
        avg34: average_depth(&queues, 3, 4),
        avg68: average_depth(&queues, 6, 8),
    };
    let priority_counts = std::array::from_fn(|priority| queues[priority].len());

    let special: Vec<ReferencePriorityFace> = queues[10]
        .iter()
        .chain(queues[11].iter())
        .copied()
        .collect();
    let mut special_index = 0_usize;
    let mut ordered_face_indices = Vec::with_capacity(faces.len());

    for priority in 0..10 {
        if let Some(threshold) = threshold_before_priority(priority, thresholds) {
            while special
                .get(special_index)
                .is_some_and(|face| i64::from(face.depth_bucket) > threshold)
            {
                ordered_face_indices.push(special[special_index].face_index);
                special_index += 1;
            }
        }

        ordered_face_indices.extend(queues[priority].iter().map(|face| face.face_index));
    }

    ordered_face_indices.extend(
        special[special_index..]
            .iter()
            .map(|face| face.face_index),
    );

    ReferencePriorityOrder {
        ordered_face_indices,
        thresholds,
        priority_counts,
    }
}

fn average_depth(queues: &[Vec<ReferencePriorityFace>; 12], first: usize, second: usize) -> i64 {
    let count = queues[first].len() + queues[second].len();
    if count == 0 {
        return 0;
    }

    let sum: i64 = queues[first]
        .iter()
        .chain(queues[second].iter())
        .map(|face| i64::from(face.depth_bucket))
        .sum();
    sum / count as i64
}

const fn threshold_before_priority(
    priority: usize,
    thresholds: ReferencePriorityThresholds,
) -> Option<i64> {
    match priority {
        0 => Some(thresholds.avg12),
        3 => Some(thresholds.avg34),
        5 => Some(thresholds.avg68),
        _ => None,
    }
}
