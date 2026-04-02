/// Grid size in pixels - all positions snap to this grid
pub const GRID_SIZE: f64 = 30.0;

/// 8 compass directions (45° increments)
/// E, SE, S, SW, W, NW, N, NE
pub const COMPASS_DIRECTIONS: [f64; 8] = [
    0.0,                                   // E (0°)
    std::f64::consts::FRAC_PI_4,           // SE (45°)
    std::f64::consts::FRAC_PI_2,           // S (90°)
    3.0 * std::f64::consts::FRAC_PI_4,     // SW (135°)
    std::f64::consts::PI,                  // W (180°)
    -3.0 * std::f64::consts::FRAC_PI_4,    // NW (-135°)
    -std::f64::consts::FRAC_PI_2,          // N (-90°)
    -std::f64::consts::FRAC_PI_4,          // NE (-45°)
];
