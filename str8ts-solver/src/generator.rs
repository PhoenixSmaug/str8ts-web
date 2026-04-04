/// Puzzle generator for Str8ts.
///
/// Pipeline (following the german guide https://de.wikipedia.org/wiki/Str8ts#Erzeugen_von_Str8ts-R%C3%A4tseln):
///   1. Generate a black-tile pattern using the logit tables
///   2. Fill white cells with any valid solution (SAT solver)
///   3. Assign valid clue digits to black cells
///   4. Strip white cells one-by-one while puzzle stays uniquely solvable (SAT)
///   5. Strip black-cell clues one-by-one while puzzle stays uniquely solvable (SAT)
///   6. Assess difficulty using the human solver; if too hard, re-add hint cells
///   7. If difficulty is outside [min, max] for target level, retry from step 1

use crate::board::{HumanStr8ts, SimpleStr8ts, N};
use crate::human::{puzzle_hardness, solve_human};
use crate::backtrack::check as backtrack_check;
use crate::sat::{puzzle_status_sat, solve_sat, SatPuzzleStatus};
use anyhow::Result;
use rand::prelude::*;

// ─── Logit tables for nice black patterns ──────────────────────────
// 45-element upper-triangle vectors; A[r,c] = A[c,r].

#[rustfmt::skip]
const LOGIT_TRI_ASYM: [f64; 45] = [
     0.6, -1.0,  1.1, -0.1,  0.1,  0.3, -0.1, -0.5,  1.9,  // row 0
    -5.0, -1.7, -1.8, -1.5, -1.6, -2.0, -4.7, -1.0,        // row 1
     0.1,  0.2,  0.0,  0.6,  0.4, -1.2,  1.3,              // row 2
    -0.4, -0.4, -0.2,  0.1, -1.5,  0.9,                    // row 3
    -1.4, -0.2, -0.3, -1.2,  0.7,                          // row 4
     0.4,  0.5, -1.2,  0.9,                                // row 5
     0.9, -1.3,  0.9,                                      // row 6
    -5.1, -0.4,                                            // row 7
     4.1,                                                  // row 8
];

#[rustfmt::skip]
const LOGIT_TRI_SYM: [f64; 45] = [
     0.6, -1.0,  0.9, -0.2,  0.5,  1.0,  0.6, -0.7,  1.7,
    -3.1, -1.7, -1.7, -1.1, -1.4, -1.5, -3.1, -0.7,
     0.2,  0.1,  0.1,  0.5,  0.4, -1.5,  0.6,
    -0.2, -1.3, -0.0,  0.5, -1.4,  1.0,
    -0.4, -1.3,  0.1, -1.1,  0.5,
    -0.2,  0.1, -1.7, -0.2,
     0.2, -1.7,  0.9,
    -3.1, -1.0,
     0.6,
];

// Mean logit offsets from diabolic (index: 0=easy, 1=medium, 2=hard, 3=diabolic, 4=cruel, 5=extreme)
const DIFF_SHIFT_ASYM: [f64; 6] = [0.3909, 0.1229, 0.0963, 0.0, -0.05, -0.10];
const DIFF_SHIFT_SYM: [f64; 6] = [0.4240, 0.2103, 0.1056, 0.0, -0.05, -0.10];

const DIST2_EPS: f64 = 0.02;
const DENSITY_MIN: f64 = 0.185;
const DENSITY_MAX: f64 = 0.340;

// ─── Difficulty enum ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Difficulty {
    Easy = 0,
    Medium = 1,
    Hard = 2,
    Diabolic = 3,
    Cruel = 4,
    Extreme = 5,
}

impl Difficulty {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "easy" => Some(Self::Easy),
            "medium" => Some(Self::Medium),
            "hard" => Some(Self::Hard),
            "diabolic" => Some(Self::Diabolic),
            "cruel" => Some(Self::Cruel),
            "extreme" => Some(Self::Extreme),
            _ => None,
        }
    }

    /// Maximum acceptable puzzle hardness (= max move hardness over all moves).
    pub fn max_hardness(self) -> i32 {
        match self {
            Self::Easy => 15,
            Self::Medium => 22,
            Self::Hard => 30,
            Self::Diabolic => 38,
            Self::Cruel => 50,
            Self::Extreme => 99,
        }
    }

    /// Minimum acceptable hardness for the STRIPPED puzzle (before hint addition).
    /// A stripped puzzle below this threshold indicates the black pattern is too
    /// simple for this difficulty tier; retry with a different pattern.
    pub fn min_hardness(self) -> i32 {
        match self {
            Self::Easy => 5,
            Self::Medium => 15,
            Self::Hard => 20,
            Self::Diabolic => 25,
            Self::Cruel => 35,
            Self::Extreme => 70,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Easy => "easy",
            Self::Medium => "medium",
            Self::Hard => "hard",
            Self::Diabolic => "diabolic",
            Self::Cruel => "cruel",
            Self::Extreme => "extreme",
        }
    }
}

// ─── Logit / probability helpers ─────────────────────────────────────────────

fn sigmoid(h: f64) -> f64 {
    1.0 / (1.0 + (-h).exp())
}

/// Expand a 45-element upper-triangle vector into a symmetric 9×9 matrix.
fn expand_tri(tri: &[f64; 45]) -> [[f64; N]; N] {
    let mut a = [[0f64; N]; N];
    let mut k = 0usize;
    for r in 0..N {
        for c in r..N {
            a[r][c] = tri[k];
            a[c][r] = tri[k];
            k += 1;
        }
    }
    a
}

fn cell_prob_table(difficulty: Difficulty, symmetric: bool) -> [[f64; N]; N] {
    let base = if symmetric {
        expand_tri(&LOGIT_TRI_SYM)
    } else {
        expand_tri(&LOGIT_TRI_ASYM)
    };
    let shift = if symmetric {
        DIFF_SHIFT_SYM[difficulty as usize]
    } else {
        DIFF_SHIFT_ASYM[difficulty as usize]
    };
    let mut out = [[0f64; N]; N];
    for r in 0..N {
        for c in 0..N {
            out[r][c] = sigmoid(base[r][c] + shift);
        }
    }
    out
}

// ─── Black-tile pattern helpers ───────────────────────────────────────────────

/// Sum of the four von-Neumann neighbours of (r,c) in the binary grid.
fn vn_sum(b: &[[u8; N]; N], r: usize, c: usize) -> u8 {
    let mut s = 0u8;
    if r > 0 {
        s += b[r - 1][c];
    }
    if r + 1 < N {
        s += b[r + 1][c];
    }
    if c > 0 {
        s += b[r][c - 1];
    }
    if c + 1 < N {
        s += b[r][c + 1];
    }
    s
}

/// Temporarily place (r,c) as black and check hard constraints:
///   • no 2×2 all-black block
///   • no VN-5-cross (centre and all 4 neighbours black)
/// Returns true if (r,c) can be placed; leaves b unchanged.
fn can_place(b: &mut [[u8; N]; N], r: usize, c: usize) -> bool {
    b[r][c] = 1;

    // 2×2 all-black check: test all four 2×2 blocks that include (r,c)
    for dr in 0usize..2 {
        for dc in 0usize..2 {
            if r >= dr && c >= dc {
                let r1 = r - dr;
                let c1 = c - dc;
                if r1 + 1 < N && c1 + 1 < N {
                    if b[r1][c1] + b[r1 + 1][c1] + b[r1][c1 + 1] + b[r1 + 1][c1 + 1] == 4 {
                        b[r][c] = 0;
                        return false;
                    }
                }
            }
        }
    }

    // VN-cross check: placing (r,c) as black must not give any neighbour all 4 black neighbours
    let neighbors = [
        (r.wrapping_sub(1), c),
        (r.wrapping_add(1), c),
        (r, c.wrapping_sub(1)),
        (r, c.wrapping_add(1)),
    ];
    for (nr, nc) in neighbors {
        if nr < N && nc < N && vn_sum(b, nr, nc) == 4 {
            b[r][c] = 0;
            return false;
        }
    }

    b[r][c] = 0;
    true
}

/// Number of black cells at distance-2 (same row/col, skipping one cell).
fn dist2_count(b: &[[u8; N]; N], r: usize, c: usize) -> u8 {
    let mut s = 0u8;
    if r >= 2 {
        s += b[r - 2][c];
    }
    if r + 2 < N {
        s += b[r + 2][c];
    }
    if c >= 2 {
        s += b[r][c - 2];
    }
    if c + 2 < N {
        s += b[r][c + 2];
    }
    s
}

/// BFS check: are all white (0) cells 4-connected?
fn white_connected(b: &[[u8; N]; N]) -> bool {
    let mut total = 0usize;
    let mut start = None;
    for r in 0..N {
        for c in 0..N {
            if b[r][c] == 0 {
                total += 1;
                if start.is_none() {
                    start = Some((r, c));
                }
            }
        }
    }
    if total == 0 {
        return true;
    }
    let (sr, sc) = start.unwrap();

    let mut visited = [[false; N]; N];
    let mut queue = vec![(sr, sc)];
    visited[sr][sc] = true;
    let mut reached = 0usize;
    while let Some((r, c)) = queue.pop() {
        reached += 1;
        let neighbors = [
            (r.wrapping_sub(1), c),
            (r.wrapping_add(1), c),
            (r, c.wrapping_sub(1)),
            (r, c.wrapping_add(1)),
        ];
        for (nr, nc) in neighbors {
            if nr < N && nc < N && !visited[nr][nc] && b[nr][nc] == 0 {
                visited[nr][nc] = true;
                queue.push((nr, nc));
            }
        }
    }
    reached == total
}

/// Generate one black-tile pattern for a 9×9 Str8ts board.
///
/// # Algorithm
///
/// The function loops, sampling a fresh candidate grid on each iteration, until
/// both a density constraint and a connectivity constraint are satisfied.
///
/// **Per-iteration steps:**
///
/// 1. **Cell-probability table** (`cell_prob`): a 9×9 matrix of independent
///    Bernoulli probabilities, derived from difficulty- and symmetry-specific
///    logit values mapped through a sigmoid.  Higher-difficulty puzzles tend to
///    have more black tiles; the table encodes a prior over spatial placement.
///
/// 2. **Raster scan** (row-major, top-left → bottom-right):
///    For each cell `(r, c)`:
///    - *Hard constraints* (`can_place`): temporarily place the cell as black and
///      reject it immediately if either:
///      - a 2×2 all-black block would be formed (checked against all four
///        2×2 blocks that contain `(r, c)`), or
///      - any von-Neumann neighbour would end up surrounded on all four sides
///        (VN-5-cross: the neighbour and all its four neighbours are black).
///    - *Distance-2 penalty* (`dist2_count`): count how many already-placed
///      black cells share the same row or column and lie exactly 2 steps away.
///      If that count is non-zero the base probability is multiplied by
///      `DIST2_EPS` (= 0.02), sharply discouraging adjacent-compartment
///      placement and preventing degenerate unit-length compartments.
///    - The cell is placed as black with the resulting (possibly penalised)
///      probability `p`.
///
/// 3. **Symmetry** (`symmetric = true`): the grid is point-symmetric about its
///    centre.  When scanning in raster order, a cell whose 180°-mirror has
///    already been decided simply copies that value.  When a new cell is placed,
///    its mirror is placed immediately as well — but only if `can_place` permits
///    it; if the mirror would violate a hard constraint the original placement
///    is rolled back (both cells remain white).
///
/// 4. **Acceptance check** — the candidate is accepted only when:
///    - `DENSITY_MIN` (18.5 %) ≤ fraction of black cells ≤ `DENSITY_MAX` (34.0 %)
///    - All white cells form a single 4-connected component (`white_connected`,
///      BFS over white cells).
///
///    Any candidate that fails either check is discarded and the loop retries.
fn generate_black_pattern(
    rng: &mut impl Rng,
    cell_prob: &[[f64; N]; N],
    symmetric: bool,
) -> [[u8; N]; N] {
    loop {
        let mut b = [[0u8; N]; N];

        for r in 0..N {
            for c in 0..N {
                if symmetric {
                    let mr = N - 1 - r;
                    let mc = N - 1 - c;
                    // Mirror cell has a lower raster index → already decided, copy
                    if mr * N + mc < r * N + c {
                        b[r][c] = b[mr][mc];
                        continue;
                    }
                }

                if !can_place(&mut b, r, c) {
                    continue;
                }

                let d2 = dist2_count(&b, r, c);
                let mut p = cell_prob[r][c];
                if d2 > 0 {
                    p *= DIST2_EPS;
                }

                if rng.r#gen::<f64>() < p {
                    b[r][c] = 1;
                    if symmetric {
                        let mr = N - 1 - r;
                        let mc = N - 1 - c;
                        if mr != r || mc != c {
                            if can_place(&mut b, mr, mc) {
                                b[mr][mc] = 1;
                            } else {
                                b[r][c] = 0; // can't mirror → unplace both
                            }
                        }
                    }
                }
            }
        }

        let black_count: usize = b.iter().flatten().map(|&x| x as usize).sum();
        let density = black_count as f64 / 81.0;
        if density >= DENSITY_MIN && density <= DENSITY_MAX && white_connected(&b) {
            return b;
        }
    }
}

// ─── Board construction helpers ───────────────────────────────────────────────

fn black_to_simple(black: &[[u8; N]; N], numbers: &[[u8; N]; N]) -> SimpleStr8ts {
    let mut is_black = [[false; N]; N];
    for r in 0..N {
        for c in 0..N {
            is_black[r][c] = black[r][c] == 1;
        }
    }
    SimpleStr8ts {
        is_black,
        numbers: *numbers,
    }
}

/// Fill white cells of a blank pattern with any valid solution using the SAT solver.
/// Returns None when the pattern has no solution (should be very rare; caller retries).
fn fill_white_cells(black: &[[u8; N]; N]) -> Result<Option<[[u8; N]; N]>> {
    let mut s = black_to_simple(black, &[[0u8; N]; N]);
    if solve_sat(&mut s)? {
        Ok(Some(s.numbers))
    } else {
        Ok(None)
    }
}

/// For each black cell, assign a random valid clue digit: a digit that does NOT
/// appear in the white cells of its row OR its column.  Cells with no valid
/// digit remain 0 (empty).
fn fill_black_cells(
    rng: &mut impl Rng,
    black: &[[u8; N]; N],
    white_solution: &[[u8; N]; N],
) -> [[u8; N]; N] {
    let mut numbers = *white_solution;

    for r in 0..N {
        for c in 0..N {
            if black[r][c] == 0 {
                continue;
            }
            numbers[r][c] = 0; // ensure clean

            // Collect digits already used in this row and column.
            // Use `numbers` (not `white_solution`) so that previously assigned
            // black-cell digits are also excluded, preventing same-row duplicates.
            let mut used = [false; 10]; // index 1..=9
            for ci in 0..N {
                if ci != c && numbers[r][ci] != 0 {
                    used[numbers[r][ci] as usize] = true;
                }
            }
            for ri in 0..N {
                if ri != r && numbers[ri][c] != 0 {
                    used[numbers[ri][c] as usize] = true;
                }
            }

            let valid: Vec<u8> = (1u8..=9).filter(|&n| !used[n as usize]).collect();
            if let Some(&digit) = valid.choose(rng) {
                numbers[r][c] = digit;
            }
        }
    }

    numbers
}

/// Try to remove each white cell's value in random order; keep the removal
/// only if the puzzle remains uniquely solvable (SAT only — no human solver).
fn strip_white_cells(
    rng: &mut impl Rng,
    black: &[[u8; N]; N],
    numbers: &mut [[u8; N]; N],
) -> Result<()> {
    let mut order: Vec<(usize, usize)> = (0..N)
        .flat_map(|r| (0..N).map(move |c| (r, c)))
        .filter(|&(r, c)| black[r][c] == 0 && numbers[r][c] != 0)
        .collect();
    order.shuffle(rng);

    for (r, c) in order {
        let saved = numbers[r][c];
        numbers[r][c] = 0;
        let s = black_to_simple(black, numbers);

        // Fast path: count locally valid values using the backtracking check.
        // If exactly 1 value is locally consistent the cell is uniquely forced
        // without needing a full SAT uniqueness solve.
        let valid_count = (1u8..=9).filter(|&v| backtrack_check(&s, r, c, v)).count();
        let keep = match valid_count {
            0 => false, // board became invalid — restore
            1 => true,  // uniquely forced — keep without SAT
            _ => matches!(puzzle_status_sat(&s)?, SatPuzzleStatus::Unique),
        };
        if !keep {
            numbers[r][c] = saved;
        }
    }
    Ok(())
}

/// Try to remove each black-cell clue in random order; keep the removal
/// only if the puzzle remains uniquely solvable (SAT only — no human solver).
fn strip_black_cells(
    rng: &mut impl Rng,
    black: &[[u8; N]; N],
    numbers: &mut [[u8; N]; N],
) -> Result<()> {
    let mut order: Vec<(usize, usize)> = (0..N)
        .flat_map(|r| (0..N).map(move |c| (r, c)))
        .filter(|&(r, c)| black[r][c] == 1 && numbers[r][c] != 0)
        .collect();
    order.shuffle(rng);

    for (r, c) in order {
        let saved = numbers[r][c];
        numbers[r][c] = 0;
        let s = black_to_simple(black, numbers);
        let valid_count = (1u8..=9).filter(|&v| backtrack_check(&s, r, c, v)).count();
        let keep = match valid_count {
            0 => false,
            1 => true,
            _ => matches!(puzzle_status_sat(&s)?, SatPuzzleStatus::Unique),
        };
        if !keep {
            numbers[r][c] = saved;
        }
    }
    Ok(())
}

/// Convert black/numbers into the 81-char puzzle string:
///   black without digit → '#',  black with digit → 'a'..='i'
///   white without digit → '.',  white with digit → '1'..='9'
pub fn to_puzzle_string(black: &[[u8; N]; N], numbers: &[[u8; N]; N]) -> String {
    let mut s = String::with_capacity(81);
    for r in 0..N {
        for c in 0..N {
            if black[r][c] == 1 {
                let n = numbers[r][c];
                if n == 0 {
                    s.push('#');
                } else {
                    s.push((b'a' + n - 1) as char);
                }
            } else {
                let n = numbers[r][c];
                if n == 0 {
                    s.push('.');
                } else {
                    s.push((b'0' + n) as char);
                }
            }
        }
    }
    s
}

/// Build the solution string: same black cells as puzzle, but ALL white cells filled.
pub fn to_solution_string(
    black: &[[u8; N]; N],
    puzzle_numbers: &[[u8; N]; N],
    white_solution: &[[u8; N]; N],
) -> String {
    let mut s = String::with_capacity(81);
    for r in 0..N {
        for c in 0..N {
            if black[r][c] == 1 {
                let n = puzzle_numbers[r][c];
                if n == 0 {
                    s.push('#');
                } else {
                    s.push((b'a' + n - 1) as char);
                }
            } else {
                s.push((b'0' + white_solution[r][c]) as char);
            }
        }
    }
    s
}

/// Run the human solver on the current puzzle state and return the max hardness.
fn assess_hardness(black: &[[u8; N]; N], numbers: &[[u8; N]; N]) -> Result<i32> {
    let board_str = to_puzzle_string(black, numbers);
    let mut s = HumanStr8ts::from_str(&board_str)?;
    let result = solve_human(&mut s);
    Ok(puzzle_hardness(&result.move_hardnesses))
}

/// Restore stripped cell values one-by-one until hardness lands in [1, target_max].
///
/// Strategy:
///   Phase 1 – while h == 100 (completely unsolvable), restore BATCH cells at a time to
///             quickly reach a solvable state.
///   Phase 2 – once h < 100 (partially solvable but still too hard), restore one cell at a
///             time.  If restoring a cell *overshoots* (h drops to 0, fully given), undo that
///             cell and count the failure.  After MAX_OVERSHOOT consecutive overshoots the
///             board can't be brought into range; return the current h so the caller retries.
///
/// Restores both white AND black cells (original_numbers contains all filled values).
/// Returns the index within `remaining` of the candidate that maximises its
/// minimum Manhattan distance to any already-placed value in `numbers`.
/// Used to spread hints across the board rather than clustering them.
fn best_spread_idx(remaining: &[(usize, usize)], numbers: &[[u8; N]; N]) -> usize {
    let filled: Vec<(usize, usize)> = (0..N)
        .flat_map(|r| (0..N).map(move |c| (r, c)))
        .filter(|&(r, c)| numbers[r][c] != 0)
        .collect();

    if filled.is_empty() {
        return 0;
    }

    remaining
        .iter()
        .enumerate()
        .map(|(i, &(r, c))| {
            let min_dist = filled
                .iter()
                .map(|&(fr, fc)| (r as i32 - fr as i32).abs() + (c as i32 - fc as i32).abs())
                .min()
                .unwrap_or(0);
            (i, min_dist)
        })
        .max_by_key(|&(_, d)| d)
        .map(|(i, _)| i)
        .unwrap_or(0)
}

fn add_hints_to_target(
    rng: &mut impl Rng,
    black: &[[u8; N]; N],
    numbers: &mut [[u8; N]; N],
    original_numbers: &[[u8; N]; N],
    target_max: i32,
) -> Result<i32> {
    const H_UNSOLVABLE: i32 = 100;
    const BATCH: usize = 3;
    const MAX_OVERSHOOT: usize = 15;

    // All empty cells that have an original value to restore, in random order.
    let mut candidates: Vec<(usize, usize)> = (0..N)
        .flat_map(|r| (0..N).map(move |c| (r, c)))
        .filter(|&(r, c)| numbers[r][c] == 0 && original_numbers[r][c] != 0)
        .collect();
    candidates.shuffle(rng);

    let mut h = assess_hardness(black, numbers)?;
    if h >= 1 && h <= target_max {
        return Ok(h);
    }
    if h == 0 {
        return Ok(0);
    }

    let mut idx = 0usize;

    // ── Phase 1: batch restore while completely unsolvable ────────────────────
    while idx < candidates.len() && h == H_UNSOLVABLE {
        let end = (idx + BATCH).min(candidates.len());
        for &(r, c) in &candidates[idx..end] {
            numbers[r][c] = original_numbers[r][c];
        }
        idx = end;
        h = assess_hardness(black, numbers)?;
        if h >= 1 && h <= target_max {
            return Ok(h);
        }
    }

    // ── Phase 2: one-at-a-time with undo on overshoot ─────────────────────────
    let mut overshoot_count = 0usize;
    while idx < candidates.len() {
        // Among the remaining candidates pick the one farthest from existing hints
        // so that clues end up spread across the board.
        let best_rel = best_spread_idx(&candidates[idx..], numbers);
        candidates.swap(idx, idx + best_rel);

        let (r, c) = candidates[idx];
        idx += 1;
        numbers[r][c] = original_numbers[r][c];
        let h_new = assess_hardness(black, numbers)?;
        if h_new >= 1 && h_new <= target_max {
            return Ok(h_new);
        }
        if h_new == 0 {
            // Overshoot: restoring this cell made the puzzle trivially given — undo.
            numbers[r][c] = 0;
            overshoot_count += 1;
            if overshoot_count >= MAX_OVERSHOOT {
                return Ok(h); // stuck, let caller retry with a different board
            }
        } else {
            h = h_new;
            overshoot_count = 0; // progress made, reset counter
        }
    }
    Ok(h)
}

// ─── Top-level generation ─────────────────────────────────────────────────────

/// Try to generate one puzzle.  Returns Some((puzzle_str, solution_str)) on
/// success, or None if this attempt should be retried (wrong difficulty, etc.).
fn try_generate_one(
    rng: &mut impl Rng,
    difficulty: Difficulty,
    symmetric: bool,
    cell_prob: &[[f64; N]; N],
) -> Result<Option<(String, String)>> {
    // Step 1: black tile pattern
    let black = generate_black_pattern(rng, cell_prob, symmetric);

    // Step 2: fill white cells
    let white_solution = match fill_white_cells(&black)? {
        Some(v) => v,
        None => return Ok(None), // unsolvable pattern (very rare)
    };

    // Step 3: populate black cells with valid clues
    let mut numbers = fill_black_cells(rng, &black, &white_solution);
    // Save the fully-filled numbers (before any stripping) for use in add_hints_to_target.
    // This includes both white cell solutions AND black cell clue values.
    let original_numbers = numbers;

    let max_h = difficulty.max_hardness();

    // Steps 4 & 5: strip white and black cells using SAT uniqueness gate.
    strip_white_cells(rng, &black, &mut numbers)?;
    strip_black_cells(rng, &black, &mut numbers)?;

    // Step 6: assess hardness of the stripped puzzle.
    let h_stripped = assess_hardness(&black, &numbers)?;

    // h == 0 → trivially given (SAT couldn't remove any cells).  Reject and retry.
    if h_stripped == 0 {
        return Ok(None);
    }

    let h_final = if h_stripped >= 1 && h_stripped <= max_h {
        // Already at target difficulty — nothing more to do.
        h_stripped
    } else {
        // h_stripped > max_h: puzzle is too hard.  Restore hints one-by-one until we
        // land in [1, max_h].  original_numbers supplies both white and black cell values.
        add_hints_to_target(rng, &black, &mut numbers, &original_numbers, max_h)?
    };

    // Accept only puzzles firmly in [1, max_h].
    if h_final == 0 || h_final > max_h {
        return Ok(None);
    }

    let puzzle_str = to_puzzle_string(&black, &numbers);
    let solution_str = to_solution_string(&black, &numbers, &white_solution);

    Ok(Some((puzzle_str, solution_str)))
}

/// Generate a single puzzle of the requested (difficulty, symmetric) variant.
/// Returns `Ok(Some(...))` on success, `Ok(None)` if no puzzle was found
/// within `max_attempts` tries (unlikely in practice).
pub fn generate_puzzle(
    difficulty: Difficulty,
    symmetric: bool,
    max_attempts: usize,
) -> Result<Option<(String, String)>> {
    let mut rng = rand::rngs::StdRng::from_entropy();
    let cell_prob = cell_prob_table(difficulty, symmetric);

    for _ in 0..max_attempts {
        if let Some(result) = try_generate_one(&mut rng, difficulty, symmetric, &cell_prob)? {
            return Ok(Some(result));
        }
    }
    Ok(None)
}

/// WASM-friendly wrapper: accepts a pre-built RNG to avoid reseeding per call.
pub fn generate_puzzle_with_rng(
    rng: &mut impl Rng,
    difficulty: Difficulty,
    symmetric: bool,
    max_attempts: usize,
) -> Result<Option<(String, String)>> {
    let cell_prob = cell_prob_table(difficulty, symmetric);
    for _ in 0..max_attempts {
        if let Some(result) = try_generate_one(rng, difficulty, symmetric, &cell_prob)? {
            return Ok(Some(result));
        }
    }
    Ok(None)
}
