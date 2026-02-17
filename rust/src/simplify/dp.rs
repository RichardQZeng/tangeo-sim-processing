use crate::geometry::Coord;

pub fn init_process_line_stack(is_line_closed: bool, points: &[Coord]) -> Vec<(usize, usize)> {
    let mut stack = Vec::new();
    if points.is_empty() {
        return stack;
    }

    let last_index = points.len() - 1;

    if is_line_closed {
        if last_index >= 4 {
            let origin = points[0];
            let mut max_d = f64::NEG_INFINITY;
            let mut mid_index = 0usize;
            for (idx, p) in points.iter().enumerate() {
                let dx = p.x - origin.x;
                let dy = p.y - origin.y;
                let d = (dx * dx + dy * dy).sqrt();
                if d > max_d {
                    max_d = d;
                    mid_index = idx;
                }
            }

            let (farthest_a, dist_a) = find_farthest_point(points, 0, mid_index);
            let (farthest_b, dist_b) = find_farthest_point(points, mid_index, last_index);

            if dist_a > 0.0 {
                stack.push((0, farthest_a));
                stack.push((farthest_a, mid_index));
            }
            if dist_b > 0.0 {
                stack.push((mid_index, farthest_b));
                stack.push((farthest_b, last_index));
            }
        }
    } else {
        stack.push((0, last_index));
    }

    stack
}

pub fn find_farthest_point(points: &[Coord], first: usize, last: usize) -> (usize, f64) {
    if last < first + 2 {
        return (first, -1.0);
    }

    let a = points[first];
    let b = points[last];
    let mut farthest_index = first;
    let mut farthest_dist = f64::NEG_INFINITY;

    for (idx, p) in points.iter().enumerate().take(last).skip(first + 1) {
        let d = point_to_segment_dist(*p, a, b);
        if d > farthest_dist {
            farthest_dist = d;
            farthest_index = idx;
        }
    }

    (farthest_index, farthest_dist)
}

pub fn point_to_segment_dist(p: Coord, a: Coord, b: Coord) -> f64 {
    let vx = b.x - a.x;
    let vy = b.y - a.y;
    let wx = p.x - a.x;
    let wy = p.y - a.y;

    let seg_len2 = vx * vx + vy * vy;
    if seg_len2 == 0.0 {
        let dx = p.x - a.x;
        let dy = p.y - a.y;
        return (dx * dx + dy * dy).sqrt();
    }

    let t = ((wx * vx) + (wy * vy)) / seg_len2;
    let t = t.clamp(0.0, 1.0);

    let proj_x = a.x + t * vx;
    let proj_y = a.y + t * vy;

    let dx = p.x - proj_x;
    let dy = p.y - proj_y;

    (dx * dx + dy * dy).sqrt()
}
