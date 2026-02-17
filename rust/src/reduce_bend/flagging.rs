use anyhow::{anyhow, Result};

use crate::geometry::RbGeom;

use super::bend::Bend;

pub fn flag_bend_to_reduce(bends: &mut [Bend], rb_geom: &RbGeom, diameter_tol: f64) -> Result<()> {
    let min_adj_area = Bend::calculate_min_adj_area(diameter_tol);

    if rb_geom.is_closed() && bends.len() == 1 {
        return Err(anyhow!("closed lines cannot contain only one bend"));
    }

    let mut lst_bends = bends
        .iter()
        .enumerate()
        .filter(|(_, bend)| bend.area < min_adj_area)
        .map(|(idx, bend)| (bend.adj_area, idx))
        .collect::<Vec<_>>();
    lst_bends.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    for (adj_area, i) in lst_bends {
        if adj_area > min_adj_area {
            break;
        }

        let prev_reduce = is_previous_bend_to_reduce(bends, rb_geom.is_closed(), i);
        let next_reduce = is_next_bend_to_reduce(bends, rb_geom.is_closed(), i);
        if !prev_reduce && !next_reduce {
            if let Some(b) = bends.get_mut(i) {
                b.to_reduce = true;
            }
        }
    }

    Ok(())
}

fn is_next_bend_to_reduce(bends: &[Bend], is_closed: bool, i: usize) -> bool {
    if bends.is_empty() {
        return false;
    }
    if is_closed {
        bends[(i + 1) % bends.len()].to_reduce
    } else if i + 1 >= bends.len() {
        false
    } else {
        bends[i + 1].to_reduce
    }
}

fn is_previous_bend_to_reduce(bends: &[Bend], is_closed: bool, i: usize) -> bool {
    if bends.is_empty() {
        return false;
    }
    if is_closed {
        bends[(i + bends.len() - 1) % bends.len()].to_reduce
    } else if i == 0 {
        false
    } else {
        bends[i - 1].to_reduce
    }
}
