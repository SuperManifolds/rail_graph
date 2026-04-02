//! Octilinear geometry helpers for the MIP metro map layout.
//!
//! Uses the sector/direction system from Nollenburg & Wolff:
//! - 8 sectors numbered 0-7 counterclockwise from positive x-axis
//! - Each sector is a 45 degree wedge centered on an octilinear direction
//! - Sector 0 = East, 1 = NE, 2 = North, 3 = NW, 4 = West, 5 = SW, 6 = South, 7 = SE

/// Compute the sector of point `to` relative to point `from`.
///
/// Returns a sector index 0-7 based on the angle from `from` to `to`.
///
/// Sector numbering (counterclockwise from East):
///   0=E, 1=NE, 2=N, 3=NW, 4=W, 5=SW, 6=S, 7=SE
///
/// Note: input is geographic (lon, lat) -- lat increases northward.
/// The MIP constraints handle screen Y inversion separately.
#[must_use]
pub fn sector(from: (f64, f64), to: (f64, f64)) -> u8 {
    let dx = to.0 - from.0; // East is positive
    let dy = to.1 - from.1; // North is positive (geographic)
    let angle = dy.atan2(dx);

    // Convert angle to sector: each sector is 45 degrees wide
    // Offset by half a sector (22.5 degrees) so boundaries fall between sectors
    let normalized = (angle + std::f64::consts::FRAC_PI_8).rem_euclid(std::f64::consts::TAU);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let sector = (normalized / std::f64::consts::FRAC_PI_4) as u8;
    sector.min(7)
}

/// Get the 3 admissible directions for an edge based on its geographic sector.
/// Returns (predecessor, original, successor) sector indices.
/// These are the sector matching the geographic direction and its two neighbors.
#[must_use]
pub fn admissible_directions(geographic_sector: u8) -> (u8, u8, u8) {
    let orig = geographic_sector;
    let prec = (orig + 7) % 8; // One sector clockwise
    let succ = (orig + 1) % 8; // One sector counterclockwise
    (prec, orig, succ)
}

/// Coordinate constraints for each octilinear direction.
/// Given direction d, returns the relationship between endpoint coordinates:
/// (`dx_sign`, `dy_sign`, diagonal) where:
/// - `dx_sign`: -1, 0, or 1 indicating x relationship
/// - `dy_sign`: -1, 0, or 1 indicating y relationship
/// - `diagonal`: true if this is a 45 degree diagonal direction
///
/// For direction d from u to v:
/// - `dx_sign = 1` means x(v) > x(u)
/// - `dy_sign = 1` means y(v) > y(u) (in the paper's coordinate system where y increases upward)
#[must_use]
pub fn direction_deltas(direction: u8) -> (i8, i8, bool) {
    match direction {
        1 => (1, 1, true),    // NE: dx > 0, dy > 0
        2 => (0, 1, false),   // N:  dx = 0, dy > 0
        3 => (-1, 1, true),   // NW: dx < 0, dy > 0
        4 => (-1, 0, false),  // W:  dx < 0, dy = 0
        5 => (-1, -1, true),  // SW: dx < 0, dy < 0
        6 => (0, -1, false),  // S:  dx = 0, dy < 0
        7 => (1, -1, true),   // SE: dx > 0, dy < 0
        // 0 and fallback: E (dx > 0, dy = 0)
        _ => (1, 0, false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sector_cardinal() {
        // East (positive lon)
        assert_eq!(sector((0.0, 0.0), (1.0, 0.0)), 0);
        // North (positive lat)
        assert_eq!(sector((0.0, 0.0), (0.0, 1.0)), 2);
        // West (negative lon)
        assert_eq!(sector((0.0, 0.0), (-1.0, 0.0)), 4);
        // South (negative lat)
        assert_eq!(sector((0.0, 0.0), (0.0, -1.0)), 6);
    }

    #[test]
    fn test_sector_diagonal() {
        // NE (positive lon, positive lat)
        assert_eq!(sector((0.0, 0.0), (1.0, 1.0)), 1);
        // SE (positive lon, negative lat)
        assert_eq!(sector((0.0, 0.0), (1.0, -1.0)), 7);
    }

    #[test]
    fn test_admissible_directions() {
        assert_eq!(admissible_directions(0), (7, 0, 1)); // E: SE, E, NE
        assert_eq!(admissible_directions(2), (1, 2, 3)); // N: NE, N, NW
    }
}
