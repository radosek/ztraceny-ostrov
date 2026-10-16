//! Region hit tests (0x450ba4, 0x450c48) and the angle helper (0x456540).

/// Delphi `Round`: FPU default rounding, ties to even.
pub fn dround(v: f64) -> i32 {
    v.round_ties_even() as i32
}

/// `CreatePolygonRgn(..., WINDING)` + `PtInRegion`, after the bbox prefilter (0x450c08).
/// GDI fills pixel centres with a top-left rule, which a non-zero winding test on
/// (x+0.5, y+0.5) matches for integer polygons.
pub fn inside(points: &[(i32, i32)], x: i32, y: i32) -> bool {
    if points.len() < 3 {
        return false;
    }
    let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
    let mut wn = 0;
    for i in 0..points.len() {
        let (x0, y0) = (points[i].0 as f64, points[i].1 as f64);
        let (x1, y1) = {
            let p = points[(i + 1) % points.len()];
            (p.0 as f64, p.1 as f64)
        };
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

/// 0x450c48: ten samples along the segment must all be inside.
pub fn segment_inside(points: &[(i32, i32)], x0: i32, y0: i32, x1: i32, y1: i32) -> bool {
    (0..=10).all(|i| {
        let x = x0 + (x1 - x0) * i / 10;
        let y = y0 + (y1 - y0) * i / 10;
        inside(points, x, y)
    })
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
