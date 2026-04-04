use crate::board::{SimpleStr8ts, N};

pub fn solve_simple(s: &mut SimpleStr8ts) -> bool {
    let Some((x, y, cands)) = find_best_empty(s) else {
        return true;
    };

    for value in cands {
        s.numbers[x][y] = value;
        if solve_simple(s) {
            return true;
        }
        s.numbers[x][y] = 0;
    }
    false
}

fn find_best_empty(s: &SimpleStr8ts) -> Option<(usize, usize, Vec<u8>)> {
    let mut best: Option<(usize, usize, Vec<u8>)> = None;

    for i in 0..N {
        for j in 0..N {
            if s.numbers[i][j] == 0 && !s.is_black[i][j] {
                let mut cands = Vec::with_capacity(9);
                for value in 1..=9 {
                    if check(s, i, j, value) {
                        cands.push(value);
                    }
                }

                if cands.is_empty() {
                    return Some((i, j, cands));
                }

                match &best {
                    None => best = Some((i, j, cands)),
                    Some((_, _, prev)) if cands.len() < prev.len() => {
                        best = Some((i, j, cands));
                        if best.as_ref().is_some_and(|(_, _, b)| b.len() == 1) {
                            return best;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    best
}

/// Check whether placing `value` at `(x, y)` is locally consistent:
/// row/column uniqueness + both compartment straight constraints.
/// Used by the strip fast-path to avoid SAT calls on near-full boards.
pub(crate) fn check(s: &SimpleStr8ts, x: usize, y: usize, value: u8) -> bool {
    for i in 0..N {
        if i != y && s.numbers[x][i] == value {
            return false;
        }
    }

    for i in 0..N {
        if i != x && s.numbers[i][y] == value {
            return false;
        }
    }

    check_compartment(s, x, y, value, &[(1, 0), (-1, 0)])
        && check_compartment(s, x, y, value, &[(0, 1), (0, -1)])
}

fn check_compartment(s: &SimpleStr8ts, x: usize, y: usize, value: u8, directions: &[(isize, isize)]) -> bool {
    let mut compartment_size = 1usize;
    let mut max_diff = 0u8;

    for &(dx, dy) in directions {
        let mut i = x as isize;
        let mut j = y as isize;
        loop {
            i += dx;
            j += dy;
            if !(0..N as isize).contains(&i) || !(0..N as isize).contains(&j) {
                break;
            }
            let iu = i as usize;
            let ju = j as usize;
            if s.is_black[iu][ju] {
                break;
            }

            compartment_size += 1;
            let n = s.numbers[iu][ju];
            if n != 0 {
                max_diff = max_diff.max(n.abs_diff(value));
            }
        }
    }

    max_diff as usize <= compartment_size - 1
}

fn count_solutions_inner(s: &mut SimpleStr8ts, limit: usize, count: &mut usize) {
    let Some((x, y, cands)) = find_best_empty(s) else {
        *count += 1;
        return;
    };
    for value in cands {
        if *count >= limit {
            return;
        }
        s.numbers[x][y] = value;
        count_solutions_inner(s, limit, count);
        s.numbers[x][y] = 0;
    }
}

/// Count the number of solutions, stopping early once `limit` is reached.
/// Fully restores the board state after the call (backtracking).
/// Used for uniqueness checking during strip: `count_solutions(s, 2) == 1`.
pub(crate) fn count_solutions(s: &mut SimpleStr8ts, limit: usize) -> usize {
    let mut count = 0;
    count_solutions_inner(s, limit, &mut count);
    count
}
