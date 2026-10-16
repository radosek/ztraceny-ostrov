//! Region hit tests (0x450ba4, 0x450c48) and the angle helper (0x456540).

/// Delphi `Round`: FPU default rounding, ties to even.
pub fn dround(v: f64) -> i32 {
    v.round_ties_even() as i32
}

fn bbox(points: &[(i32, i32)], x: i32, y: i32) -> bool {
    let (mut l, mut t, mut r, mut b) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for &(px, py) in points {
        l = l.min(px);
        t = t.min(py);
        r = r.max(px);
        b = b.max(py);
    }
    l <= x && x < r && t <= y && y < b
}

/// 0x450ba4: bbox prefilter, then `CreatePolygonRgn(WINDING)` + `PtInRegion`, modelled as a
/// non-zero winding test at the pixel centre (exact for axis-aligned edges).
pub fn inside(points: &[(i32, i32)], x: i32, y: i32) -> bool {
    if points.len() < 3 || !bbox(points, x, y) {
        return false;
    }
    let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
    let mut wn = 0;
    for i in 0..points.len() {
        let (x0, y0) = (points[i].0 as f64, points[i].1 as f64);
        let p = points[(i + 1) % points.len()];
        let (x1, y1) = (p.0 as f64, p.1 as f64);
        let cross = (x1 - x0) * (py - y0) - (px - x0) * (y1 - y0);
        if y0 <= py {
            if y1 > py && cross > 0.0 {
                wn += 1;
            }
        } else if y1 <= py && cross < 0.0 {
            wn -= 1;
        }
    }
    wn != 0
}

/// 0x450c48: endpoints bbox-tested, then samples k = 1..=10 (start itself is not polygon-tested).
pub fn segment_inside(points: &[(i32, i32)], x0: i32, y0: i32, x1: i32, y1: i32) -> bool {
    if points.len() < 3 || !bbox(points, x0, y0) || !bbox(points, x1, y1) {
        return false;
    }
    (1..=10).all(|k| inside(points, x0 + (x1 - x0) * k / 10, y0 + (y1 - y0) * k / 10))
}

/// Degrees 0..360, 0 = east, 90 = up (screen y grows down).
pub fn angle(x1: i32, y1: i32, x2: i32, y2: i32) -> i32 {
    let dx = x2 - x1;
    let dy = y1 - y2;
    let a = match (dx, dy) {
        (0, 0) => 0.0,
        (0, d) => {
            if d > 0 {
                90.0
            } else {
                270.0
            }
        }
        (d, 0) => {
            if d > 0 {
                0.0
            } else {
                180.0
            }
        }
        _ => {
            let a = (dy.abs() as f64 / dx.abs() as f64).atan().to_degrees();
            match (dx > 0, dy > 0) {
                (true, true) => a,
                (false, true) => 180.0 - a,
                (false, false) => 180.0 + a,
                (true, false) => 360.0 - a,
            }
        }
    };
    (dround(a) + 360) % 360
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bankers_rounding() {
        assert_eq!(dround(2.5), 2);
        assert_eq!(dround(3.5), 4);
        assert_eq!(dround(-0.5), 0);
    }

    #[test]
    fn square_edges() {
        let sq = [(0, 0), (10, 0), (10, 10), (0, 10)];
        assert!(inside(&sq, 0, 0));
        assert!(inside(&sq, 9, 9));
        assert!(!inside(&sq, 10, 5));
        assert!(!inside(&sq, 5, 10));
    }

    #[test]
    fn angles() {
        assert_eq!(angle(0, 0, 10, 0), 0);
        assert_eq!(angle(0, 0, 0, -10), 90);
        assert_eq!(angle(0, 0, -10, 0), 180);
        assert_eq!(angle(0, 0, 0, 10), 270);
        assert_eq!(angle(0, 0, 10, -10), 45);
    }
}
