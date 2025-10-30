/// Calculates the shortest angular distance between two angles in radians.
///
/// Returns a value in the range [0, π], representing the smallest angle
/// between the two input angles when considering the circular nature of angles.
///
/// # Arguments
/// * `a1` - First angle in radians
/// * `a2` - Second angle in radians
///
/// # Examples
/// ```
/// use std::f64::consts::PI;
///
/// // Angles close together
/// let diff = angle_difference(0.1, 0.2);
/// assert!((diff - 0.1).abs() < 1e-10);
///
/// // Angles wrapping around (350° and 10° are only 20° apart)
/// let diff = angle_difference(350.0 * PI / 180.0, 10.0 * PI / 180.0);
/// assert!((diff - 20.0 * PI / 180.0).abs() < 1e-10);
/// ```
#[must_use]
pub fn angle_difference(a1: f64, a2: f64) -> f64 {
    let diff = (a1 - a2).abs();
    if diff > std::f64::consts::PI {
        2.0 * std::f64::consts::PI - diff
    } else {
        diff
    }
}

/// Computes the 2D cross product to determine the orientation of a point relative to a line.
///
/// Returns a positive value if the point is to the left of the line (counter-clockwise),
/// negative if to the right (clockwise), and zero if collinear.
///
/// # Arguments
/// * `line_start` - Starting point of the line segment
/// * `line_end` - Ending point of the line segment
/// * `point` - Point to test
#[must_use]
pub fn cross_product_2d(line_start: (f64, f64), line_end: (f64, f64), point: (f64, f64)) -> f64 {
    (line_end.0 - line_start.0) * (point.1 - line_start.1) -
    (line_end.1 - line_start.1) * (point.0 - line_start.0)
}

/// Checks if two line segments intersect.
///
/// Uses the parametric line equation method for numerical stability.
/// Returns false if the segments are parallel or don't intersect.
///
/// # Arguments
/// * `a1` - First endpoint of segment A
/// * `a2` - Second endpoint of segment A
/// * `b1` - First endpoint of segment B
/// * `b2` - Second endpoint of segment B
#[must_use]
pub fn line_segments_intersect(
    a1: (f64, f64),
    a2: (f64, f64),
    b1: (f64, f64),
    b2: (f64, f64),
) -> bool {
    // Calculate determinant to check if lines are parallel
    let d = (a2.0 - a1.0) * (b2.1 - b1.1) - (a2.1 - a1.1) * (b2.0 - b1.0);

    // Lines are parallel or coincident
    if d.abs() < 1e-10 {
        return false;
    }

    // Calculate parametric intersection points
    let t = ((b1.0 - a1.0) * (b2.1 - b1.1) - (b1.1 - a1.1) * (b2.0 - b1.0)) / d;
    let u = ((b1.0 - a1.0) * (a2.1 - a1.1) - (b1.1 - a1.1) * (a2.0 - a1.0)) / d;

    // Check if intersection occurs within both segments
    (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)
}

/// Calculates the minimum distance from a point to a line segment.
///
/// # Arguments
/// * `point` - The point to measure from
/// * `seg_start` - Starting point of the line segment
/// * `seg_end` - Ending point of the line segment
#[must_use]
pub fn point_to_line_segment_distance(point: (f64, f64), seg_start: (f64, f64), seg_end: (f64, f64)) -> f64 {
    let dx = seg_end.0 - seg_start.0;
    let dy = seg_end.1 - seg_start.1;
    let len_sq = dx * dx + dy * dy;

    if len_sq == 0.0 {
        let px = point.0 - seg_start.0;
        let py = point.1 - seg_start.1;
        return (px * px + py * py).sqrt();
    }

    let t = ((point.0 - seg_start.0) * dx + (point.1 - seg_start.1) * dy) / len_sq;
    let t = t.clamp(0.0, 1.0);

    let closest_x = seg_start.0 + t * dx;
    let closest_y = seg_start.1 + t * dy;

    let px = point.0 - closest_x;
    let py = point.1 - closest_y;
    (px * px + py * py).sqrt()
}

/// Calculates the minimum distance between two line segments.
///
/// Returns 0.0 if the segments intersect.
///
/// # Arguments
/// * `a1` - First endpoint of segment A
/// * `a2` - Second endpoint of segment A
/// * `b1` - First endpoint of segment B
/// * `b2` - Second endpoint of segment B
#[must_use]
pub fn line_segment_distance(
    a1: (f64, f64),
    a2: (f64, f64),
    b1: (f64, f64),
    b2: (f64, f64),
) -> f64 {
    // Check if segments intersect
    if line_segments_intersect(a1, a2, b1, b2) {
        return 0.0;
    }

    // Check distances from endpoints to opposite segments
    let d1 = point_to_line_segment_distance(a1, b1, b2);
    let d2 = point_to_line_segment_distance(a2, b1, b2);
    let d3 = point_to_line_segment_distance(b1, a1, a2);
    let d4 = point_to_line_segment_distance(b2, a1, a2);

    d1.min(d2).min(d3).min(d4)
}
