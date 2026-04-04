use crate::board::{Compartment, HumanStr8ts, N};
use crate::hopcroft_karp::has_perfect_matching;
use std::collections::{HashMap, HashSet};

pub const H_SINGLE: i32 = 3;
pub const H_SURE_CANDIDATES: i32 = 5;
pub const H_LOCKED: i32 = 30;
pub const H_UNIQUE: i32 = 60;
pub const H_SETTI: i32 = 70;
pub const H_SETTI_CONSIDER: i32 = 75;
pub const H_SETTI_SET: i32 = 75;
pub const H_YWING: i32 = 80;
pub const H_BINARY_GUESS: i32 = 90;
pub const H_UNSOLVABLE: i32 = 100;

fn stranded_hardness(k: usize) -> i32 {
    10 + (k as i32 - 1) * 2
}

fn split_hardness(k: usize) -> i32 {
    15 + (k as i32 - 1) * 2
}

fn mindgap_hardness(k: usize) -> i32 {
    15 + (k as i32 - 1) * 2
}

fn range_check_hardness(k: usize) -> i32 {
    20 + (k as i32 - 1) * 3
}

fn naked_set_hardness(size: usize) -> i32 {
    20 + (size as i32 - 1) * 5
}

fn hidden_set_hardness(size: usize) -> i32 {
    25 + (size as i32 - 1) * 5
}

fn sea_creature_hardness(n: usize) -> i32 {
    50 + n as i32 * 5
}

#[derive(Debug, Clone)]
pub struct HumanSolveResult {
    pub move_hardnesses: Vec<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StrategyKind {
    Single,
    Sure,
    Stranded,
    Split,
    MindGap,
    RangeCheck,
    NakedSet,
    HiddenSet,
    Locked,
    Sea,
    Unique,
    Setti,
    SettiConsider,
    SettiSet,
    YWing,
    BinaryGuess,
}

#[derive(Debug, Clone, Copy)]
struct StrategySpec {
    hardness: i32,
    kind: StrategyKind,
    param: usize,
    order: i32,
}

#[derive(Debug, Clone)]
pub struct StrategyEffect {
    pub row: usize,
    pub col: usize,
    pub set_value: Option<u8>,
    pub removed_mask: u16,
}

#[derive(Debug, Clone)]
struct StrategyOutcome {
    kind: StrategyKind,
    hardness: i32,
    description: String,
    effects: Vec<StrategyEffect>,
}

#[derive(Debug, Clone)]
pub struct HumanStep {
    pub strategy: String,
    pub hardness: i32,
    pub description: String,
    pub immediate_effects: Vec<StrategyEffect>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettiStatus {
    Missing,
    Sure,
    Possible,
}

fn bit(n: u8) -> u16 {
    1u16 << (n - 1)
}

fn mask_has(mask: u16, n: u8) -> bool {
    (mask & bit(n)) != 0
}

fn mask_single(mask: u16) -> Option<u8> {
    if mask.count_ones() != 1 {
        return None;
    }
    Some(mask.trailing_zeros() as u8 + 1)
}

fn mask_values(mask: u16) -> Vec<u8> {
    (1..=9).filter(|&n| mask_has(mask, n)).collect()
}

fn range_mask(start: u8, size: usize) -> u16 {
    let mut mask = 0u16;
    for n in start..=(start + size as u8 - 1) {
        mask |= bit(n);
    }
    mask
}

fn all_compartments(s: &HumanStr8ts) -> Vec<Compartment> {
    s.row_compartments
        .iter()
        .chain(s.col_compartments.iter())
        .cloned()
        .collect()
}

fn combinations<T: Copy>(items: &[T], k: usize) -> Vec<Vec<T>> {
    fn rec<T: Copy>(items: &[T], k: usize, start: usize, cur: &mut Vec<T>, out: &mut Vec<Vec<T>>) {
        if cur.len() == k {
            out.push(cur.clone());
            return;
        }
        for i in start..items.len() {
            cur.push(items[i]);
            rec(items, k, i + 1, cur, out);
            cur.pop();
        }
    }

    let mut out = Vec::new();
    if k == 0 || k > items.len() {
        return out;
    }
    let mut cur = Vec::with_capacity(k);
    rec(items, k, 0, &mut cur, &mut out);
    out
}

fn add(s: &mut HumanStr8ts, r: usize, c: usize, num: u8) {
    if s.solved[r][c] {
        return;
    }
    s.solved[r][c] = true;
    s.numbers[r][c] = num;
    s.candidates[r][c] = bit(num);
}

fn add_with_propagation(s: &mut HumanStr8ts, r: usize, c: usize, num: u8) {
    add(s, r, c, num);
    propagate_add(s, r, c, num);
}

fn propagate_add(s: &mut HumanStr8ts, r: usize, c: usize, num: u8) {
    for cc in 0..N {
        if cc != c && !s.is_black[r][cc] {
            rem_candidate(s, r, cc, num);
        }
    }
    for rr in 0..N {
        if rr != r && !s.is_black[rr][c] {
            rem_candidate(s, rr, c, num);
        }
    }
}

fn rem_candidate(s: &mut HumanStr8ts, r: usize, c: usize, num: u8) {
    s.candidates[r][c] &= !bit(num);
}

fn get_compartment_candidates(s: &HumanStr8ts, comp: &Compartment) -> u16 {
    comp.iter().fold(0u16, |acc, &(r, c)| acc | s.candidates[r][c])
}

fn get_compartment_solved_values(s: &HumanStr8ts, comp: &Compartment) -> u16 {
    let mut solved_mask = 0u16;
    for &(r, c) in comp {
        if s.solved[r][c] {
            solved_mask |= bit(s.numbers[r][c]);
        }
    }
    solved_mask
}

fn get_possible_ranges(candidates: u16, size: usize, required: u16) -> Vec<(u8, u8)> {
    let mut out = Vec::new();
    if size == 0 {
        return out;
    }
    for start in 1..=(10 - size) as u8 {
        let end = start + size as u8 - 1;
        let rmask = range_mask(start, size);
        if (rmask & candidates) == rmask && (required & !rmask) == 0 {
            out.push((start, end));
        }
    }
    out
}

fn get_compartment_ranges(s: &HumanStr8ts, comp: &Compartment) -> Vec<(u8, u8)> {
    let cands = get_compartment_candidates(s, comp);
    let solved = get_compartment_solved_values(s, comp);
    get_possible_ranges(cands, comp.len(), solved)
}

fn get_sure_candidates(ranges: &[(u8, u8)], size: usize) -> u16 {
    if ranges.is_empty() {
        return 0;
    }
    let mut sure = range_mask(ranges[0].0, size);
    for &(start, _) in &ranges[1..] {
        sure &= range_mask(start, size);
    }
    sure
}

fn is_valid(s: &HumanStr8ts) -> bool {
    for r in 0..N {
        for c in 0..N {
            if !s.is_black[r][c] && s.candidates[r][c] == 0 {
                return false;
            }
        }
    }
    true
}

fn is_done(s: &HumanStr8ts) -> bool {
    for r in 0..N {
        for c in 0..N {
            if !s.is_black[r][c] && !s.solved[r][c] {
                return false;
            }
        }
    }
    true
}

fn has_perfect_matching_for_values(cell_candidates: &[u16], range_values: &[u8]) -> bool {
    let n = cell_candidates.len();
    if n != range_values.len() {
        return false;
    }
    if n == 0 {
        return true;
    }

    let mut left_adj = vec![Vec::<usize>::new(); n];
    for (i, &cand_mask) in cell_candidates.iter().enumerate() {
        for (j, &val) in range_values.iter().enumerate() {
            if mask_has(cand_mask, val) {
                left_adj[i].push(j);
            }
        }
    }
    has_perfect_matching(&left_adj, n)
}

fn use_single(s: &mut HumanStr8ts) -> i32 {
    for r in 0..N {
        for c in 0..N {
            if !s.is_black[r][c] && !s.solved[r][c] && s.candidates[r][c].count_ones() == 1 {
                let n = mask_single(s.candidates[r][c]).unwrap();
                add(s, r, c, n);
                return H_SINGLE;
            }
        }
    }

    for comp in &s.row_compartments {
        let ranges = get_compartment_ranges(s, comp);
        let sure = get_sure_candidates(&ranges, comp.len());
        for n in mask_values(sure) {
            let cells_with_n: Vec<(usize, usize)> = comp
                .iter()
                .copied()
                .filter(|&(r, c)| mask_has(s.candidates[r][c], n))
                .collect();
            if cells_with_n.len() == 1 {
                let (r, c) = cells_with_n[0];
                if !s.solved[r][c] {
                    add(s, r, c, n);
                    return H_SINGLE;
                }
            }
        }
    }

    for comp in &s.col_compartments {
        let ranges = get_compartment_ranges(s, comp);
        let sure = get_sure_candidates(&ranges, comp.len());
        for n in mask_values(sure) {
            let cells_with_n: Vec<(usize, usize)> = comp
                .iter()
                .copied()
                .filter(|&(r, c)| mask_has(s.candidates[r][c], n))
                .collect();
            if cells_with_n.len() == 1 {
                let (r, c) = cells_with_n[0];
                if !s.solved[r][c] {
                    add(s, r, c, n);
                    return H_SINGLE;
                }
            }
        }
    }

    0
}

fn use_compartment_range_check(s: &mut HumanStr8ts, comp_size: usize) -> i32 {
    let mut effective = false;

    for comp in all_compartments(s) {
        if comp.len() != comp_size {
            continue;
        }

        let ranges = get_compartment_ranges(s, &comp);
        if ranges.is_empty() {
            continue;
        }

        let unsolved: Vec<(usize, usize)> = comp.iter().copied().filter(|&(r, c)| !s.solved[r][c]).collect();
        let n_unsolved = unsolved.len();
        if n_unsolved == 0 {
            continue;
        }

        let mut valid_assignments = vec![0u16; n_unsolved];

        for (start, end) in ranges {
            let mut compatible = true;
            let mut solved_values = Vec::<u8>::new();
            for &(r, c) in &comp {
                if s.solved[r][c] {
                    let v = s.numbers[r][c];
                    if v < start || v > end {
                        compatible = false;
                        break;
                    }
                    solved_values.push(v);
                }
            }
            if !compatible {
                continue;
            }

            let mut available_values = Vec::<u8>::new();
            for n in start..=end {
                if !solved_values.contains(&n) {
                    available_values.push(n);
                }
            }
            if available_values.len() != n_unsolved {
                continue;
            }

            let cell_candidates: Vec<u16> = unsolved.iter().map(|&(r, c)| s.candidates[r][c]).collect();
            if !has_perfect_matching_for_values(&cell_candidates, &available_values) {
                continue;
            }

            for i in 0..n_unsolved {
                for &val in &available_values {
                    if !mask_has(cell_candidates[i], val) {
                        continue;
                    }
                    let mut test_candidates = cell_candidates.clone();
                    test_candidates[i] = bit(val);
                    if has_perfect_matching_for_values(&test_candidates, &available_values) {
                        valid_assignments[i] |= bit(val);
                    }
                }
            }
        }

        for (i, &(r, c)) in unsolved.iter().enumerate() {
            let old = s.candidates[r][c];
            let new_mask = old & valid_assignments[i];
            if new_mask != old {
                s.candidates[r][c] = new_mask;
                effective = true;
            }
        }
    }

    if effective {
        range_check_hardness(comp_size)
    } else {
        0
    }
}

fn use_stranded_digits(s: &mut HumanStr8ts, comp_size: usize) -> i32 {
    let mut effective = false;

    for comp in all_compartments(s) {
        let size = comp.len();
        if size != comp_size {
            continue;
        }

        let cands = get_compartment_candidates(s, &comp);
        let solved = get_compartment_solved_values(s, &comp);

        for n in mask_values(cands) {
            let mut can_be_part = false;
            let start_min = usize::max(1, (n as usize).saturating_sub(size.saturating_sub(1)));
            let start_max = usize::min(n as usize, 10 - size);
            for start in start_min..=start_max {
                let rmask = range_mask(start as u8, size);
                if (rmask & cands) == rmask && (solved & !rmask) == 0 {
                    can_be_part = true;
                    break;
                }
            }
            if !can_be_part {
                for &(r, c) in &comp {
                    if mask_has(s.candidates[r][c], n) {
                        rem_candidate(s, r, c, n);
                        effective = true;
                    }
                }
            }
        }

        if size == 2 {
            let (r1, c1) = comp[0];
            let (r2, c2) = comp[1];
            for (ra, ca, rb, cb) in [(r1, c1, r2, c2), (r2, c2, r1, c1)] {
                if s.solved[ra][ca] {
                    continue;
                }
                let other = s.candidates[rb][cb];
                for n in mask_values(s.candidates[ra][ca]) {
                    let has_adjacent = (n > 1 && mask_has(other, n - 1)) || (n < 9 && mask_has(other, n + 1));
                    if !has_adjacent {
                        rem_candidate(s, ra, ca, n);
                        effective = true;
                    }
                }
            }
        }
    }

    if effective {
        stranded_hardness(comp_size)
    } else {
        0
    }
}

fn overlaps(r1: (u8, u8), r2: (u8, u8)) -> bool {
    r1.0 <= r2.1 && r2.0 <= r1.1
}

fn use_split_compartment(s: &mut HumanStr8ts, comp_size: usize) -> i32 {
    let mut effective = false;

    for comp in all_compartments(s) {
        let size = comp.len();
        if size != comp_size {
            continue;
        }

        let ranges = get_compartment_ranges(s, &comp);
        if ranges.len() <= 1 {
            continue;
        }

        let mut groups: Vec<Vec<(u8, u8)>> = Vec::new();
        for rng in ranges {
            let mut found = false;
            for g in &mut groups {
                if g.iter().any(|&r| overlaps(rng, r)) {
                    g.push(rng);
                    found = true;
                    break;
                }
            }
            if !found {
                groups.push(vec![rng]);
            }
        }

        let mut merged = true;
        while merged {
            merged = false;
            let mut i = 0;
            while i < groups.len() {
                let mut j = i + 1;
                while j < groups.len() {
                    if groups[i]
                        .iter()
                        .any(|&r1| groups[j].iter().any(|&r2| overlaps(r1, r2)))
                    {
                        let other = groups.remove(j);
                        groups[i].extend(other);
                        merged = true;
                    } else {
                        j += 1;
                    }
                }
                i += 1;
            }
        }

        if groups.len() > 1 {
            for group in groups {
                let group_sure = get_sure_candidates(&group, size);
                let group_min = group.iter().map(|r| r.0).min().unwrap();
                let group_max = group.iter().map(|r| r.1).max().unwrap();
                let mut group_mask = 0u16;
                for n in group_min..=group_max {
                    group_mask |= bit(n);
                }

                for n in mask_values(group_sure) {
                    for &(r, c) in &comp {
                        if !s.solved[r][c] && mask_has(s.candidates[r][c], n) {
                            let cell_cands_in_group = s.candidates[r][c] & group_mask;
                            if cell_cands_in_group == 0 {
                                rem_candidate(s, r, c, n);
                                effective = true;
                            }
                        }
                    }
                }
            }
        }
    }

    if effective {
        split_hardness(comp_size)
    } else {
        0
    }
}

fn use_mind_the_gap(s: &mut HumanStr8ts, comp_size: usize) -> i32 {
    let mut effective = false;

    for comp in all_compartments(s) {
        let size = comp.len();
        if size != comp_size {
            continue;
        }

        for &(r, c) in &comp {
            if s.solved[r][c] {
                continue;
            }
            let mut cands = mask_values(s.candidates[r][c]);
            if cands.len() < 2 {
                continue;
            }
            cands.sort_unstable();
            let min_c = cands[0];
            let max_c = *cands.last().unwrap();
            let gap = max_c as i32 - min_c as i32;
            if gap < size as i32 {
                continue;
            }

            let border = (gap - size as i32) as u8;
            let low_side: Vec<u8> = cands.iter().copied().filter(|&n| n <= min_c + border).collect();
            let high_side: Vec<u8> = cands.iter().copied().filter(|&n| n >= max_c - border).collect();

            if cands.len() == 2 && low_side.len() == 1 && high_side.len() == 1 {
                let low_forced = low_side[0];
                let high_forced = high_side[0];
                for &(r2, c2) in &comp {
                    if (r2, c2) != (r, c) && !s.solved[r2][c2] {
                        if mask_has(s.candidates[r2][c2], low_forced) {
                            rem_candidate(s, r2, c2, low_forced);
                            effective = true;
                        }
                        if mask_has(s.candidates[r2][c2], high_forced) {
                            rem_candidate(s, r2, c2, high_forced);
                            effective = true;
                        }
                    }
                }
            } else if cands.len() == 3 {
                if low_side.len() == 1 && high_side.len() > 1 && low_side[0] == min_c {
                    let low_forced = low_side[0];
                    for &(r2, c2) in &comp {
                        if (r2, c2) != (r, c) && !s.solved[r2][c2] && mask_has(s.candidates[r2][c2], low_forced) {
                            rem_candidate(s, r2, c2, low_forced);
                            effective = true;
                        }
                    }
                } else if high_side.len() == 1 && low_side.len() > 1 && high_side[0] == max_c {
                    let high_forced = high_side[0];
                    for &(r2, c2) in &comp {
                        if (r2, c2) != (r, c) && !s.solved[r2][c2] && mask_has(s.candidates[r2][c2], high_forced) {
                            rem_candidate(s, r2, c2, high_forced);
                            effective = true;
                        }
                    }
                }
            }
        }

        for i in 0..comp.len() {
            for j in (i + 1)..comp.len() {
                let (r1, c1) = comp[i];
                let (r2, c2) = comp[j];
                if s.solved[r1][c1] || s.solved[r2][c2] {
                    continue;
                }
                let cands1 = s.candidates[r1][c1];
                let cands2 = s.candidates[r2][c2];
                if cands1.count_ones() != 2 || cands2.count_ones() != 2 {
                    continue;
                }

                let common = cands1 & cands2;
                if common.count_ones() != 1 {
                    continue;
                }
                let bridge = mask_single(common).unwrap();
                let mut low = mask_single(cands1 & !common).unwrap();
                let mut high = mask_single(cands2 & !common).unwrap();
                let mut a = (r1, c1);
                let mut b = (r2, c2);
                if low > bridge {
                    std::mem::swap(&mut low, &mut high);
                    std::mem::swap(&mut a, &mut b);
                }
                if !(low < bridge && bridge < high) {
                    continue;
                }
                let gap = high as i32 - low as i32;
                if gap >= size as i32 {
                    for &(r, c) in &comp {
                        if (r, c) != a && (r, c) != b && !s.solved[r][c] && mask_has(s.candidates[r][c], bridge) {
                            rem_candidate(s, r, c, bridge);
                            effective = true;
                        }
                    }
                }
            }
        }
    }

    if effective {
        mindgap_hardness(comp_size)
    } else {
        0
    }
}

fn use_naked_set(s: &mut HumanStr8ts, set_size: usize) -> i32 {
    for comp in all_compartments(s) {
        if comp.len() <= set_size {
            continue;
        }
        let unsolved: Vec<(usize, usize)> = comp.iter().copied().filter(|&(r, c)| !s.solved[r][c]).collect();
        if unsolved.len() <= set_size {
            continue;
        }
        let small_cells: Vec<(usize, usize)> = unsolved
            .iter()
            .copied()
            .filter(|&(r, c)| s.candidates[r][c].count_ones() as usize <= set_size)
            .collect();

        for subset in combinations(&small_cells, set_size) {
            let mut union_cands = 0u16;
            for &(r, c) in &subset {
                union_cands |= s.candidates[r][c];
            }
            if union_cands.count_ones() as usize == set_size {
                let subset_set: HashSet<(usize, usize)> = subset.iter().copied().collect();
                let mut effective = false;
                for &(r, c) in &comp {
                    if !subset_set.contains(&(r, c)) && !s.solved[r][c] {
                        let old = s.candidates[r][c];
                        let new_mask = old & !union_cands;
                        if new_mask != old {
                            s.candidates[r][c] = new_mask;
                            effective = true;
                        }
                    }
                }
                if effective {
                    return naked_set_hardness(set_size);
                }
            }
        }
    }

    for r in 0..N {
        let unsolved: Vec<(usize, usize)> = (0..N)
            .filter(|&c| !s.is_black[r][c] && !s.solved[r][c])
            .map(|c| (r, c))
            .collect();
        if unsolved.len() <= set_size {
            continue;
        }
        let small_cells: Vec<(usize, usize)> = unsolved
            .iter()
            .copied()
            .filter(|&(rr, c)| s.candidates[rr][c].count_ones() as usize <= set_size)
            .collect();

        for subset in combinations(&small_cells, set_size) {
            let mut union_cands = 0u16;
            for &(rr, c) in &subset {
                union_cands |= s.candidates[rr][c];
            }
            if union_cands.count_ones() as usize == set_size {
                let subset_set: HashSet<(usize, usize)> = subset.iter().copied().collect();
                let mut effective = false;
                for c in 0..N {
                    if !s.is_black[r][c] && !s.solved[r][c] && !subset_set.contains(&(r, c)) {
                        let old = s.candidates[r][c];
                        let new_mask = old & !union_cands;
                        if new_mask != old {
                            s.candidates[r][c] = new_mask;
                            effective = true;
                        }
                    }
                }
                if effective {
                    return naked_set_hardness(set_size);
                }
            }
        }
    }

    for c in 0..N {
        let unsolved: Vec<(usize, usize)> = (0..N)
            .filter(|&r| !s.is_black[r][c] && !s.solved[r][c])
            .map(|r| (r, c))
            .collect();
        if unsolved.len() <= set_size {
            continue;
        }
        let small_cells: Vec<(usize, usize)> = unsolved
            .iter()
            .copied()
            .filter(|&(r, cc)| s.candidates[r][cc].count_ones() as usize <= set_size)
            .collect();

        for subset in combinations(&small_cells, set_size) {
            let mut union_cands = 0u16;
            for &(r, cc) in &subset {
                union_cands |= s.candidates[r][cc];
            }
            if union_cands.count_ones() as usize == set_size {
                let subset_set: HashSet<(usize, usize)> = subset.iter().copied().collect();
                let mut effective = false;
                for r in 0..N {
                    if !s.is_black[r][c] && !s.solved[r][c] && !subset_set.contains(&(r, c)) {
                        let old = s.candidates[r][c];
                        let new_mask = old & !union_cands;
                        if new_mask != old {
                            s.candidates[r][c] = new_mask;
                            effective = true;
                        }
                    }
                }
                if effective {
                    return naked_set_hardness(set_size);
                }
            }
        }
    }

    0
}

fn use_hidden_set(s: &mut HumanStr8ts, set_size: usize) -> i32 {
    for comp in all_compartments(s) {
        if comp.len() <= set_size {
            continue;
        }
        let ranges = get_compartment_ranges(s, &comp);
        let sure = get_sure_candidates(&ranges, comp.len());
        if (sure.count_ones() as usize) < set_size {
            continue;
        }

        let sure_vals = mask_values(sure);
        let mut candidate_cells: HashMap<u8, Vec<(usize, usize)>> = HashMap::new();
        for n in &sure_vals {
            let cells: Vec<(usize, usize)> = comp
                .iter()
                .copied()
                .filter(|&(r, c)| mask_has(s.candidates[r][c], *n))
                .collect();
            candidate_cells.insert(*n, cells);
        }

        for subset in combinations(&sure_vals, set_size) {
            let mut cells: HashSet<(usize, usize)> = HashSet::new();
            for n in &subset {
                if let Some(v) = candidate_cells.get(n) {
                    for &cell in v {
                        cells.insert(cell);
                    }
                }
            }
            if cells.len() == set_size {
                let mut subset_mask = 0u16;
                for &n in &subset {
                    subset_mask |= bit(n);
                }
                let mut effective = false;
                for &(r, c) in &cells {
                    if !s.solved[r][c] {
                        let old = s.candidates[r][c];
                        let new_mask = old & subset_mask;
                        if new_mask != old {
                            s.candidates[r][c] = new_mask;
                            effective = true;
                        }
                    }
                }
                if effective {
                    return hidden_set_hardness(set_size);
                }
            }
        }
    }

    for r in 0..N {
        let white_cells: Vec<(usize, usize)> = (0..N).filter(|&c| !s.is_black[r][c]).map(|c| (r, c)).collect();
        if white_cells.len() <= set_size {
            continue;
        }

        let all_cands = white_cells.iter().fold(0u16, |acc, &(rr, c)| acc | s.candidates[rr][c]);
        let all_vals = mask_values(all_cands);
        for subset in combinations(&all_vals, set_size) {
            let mut cells: HashSet<(usize, usize)> = HashSet::new();
            for &n in &subset {
                for &(rr, c) in &white_cells {
                    if mask_has(s.candidates[rr][c], n) {
                        cells.insert((rr, c));
                    }
                }
            }

            if cells.len() == set_size {
                let mut all_sure = true;
                for &n in &subset {
                    let Some(&(_, c0)) = cells.iter().find(|&&(rr, cc)| rr == r && mask_has(s.candidates[rr][cc], n)) else {
                        all_sure = false;
                        break;
                    };
                    let comp_idx = s.cell_to_row_compartment[r][c0];
                    let comp = &s.row_compartments[comp_idx];
                    let comp_ranges = get_compartment_ranges(s, comp);
                    let comp_sure = get_sure_candidates(&comp_ranges, comp.len());
                    if !mask_has(comp_sure, n) {
                        all_sure = false;
                        break;
                    }
                }

                if all_sure {
                    let mut subset_mask = 0u16;
                    for &n in &subset {
                        subset_mask |= bit(n);
                    }
                    let mut effective = false;
                    for &(rr, c) in &cells {
                        if !s.solved[rr][c] {
                            let old = s.candidates[rr][c];
                            let new_mask = old & subset_mask;
                            if new_mask != old {
                                s.candidates[rr][c] = new_mask;
                                effective = true;
                            }
                        }
                    }
                    if effective {
                        return hidden_set_hardness(set_size);
                    }
                }
            }
        }
    }

    0
}

fn check_locked_compartments(s: &mut HumanStr8ts, comp1: &Compartment, comp2: &Compartment) -> bool {
    let mut effective = false;
    let ranges1 = get_compartment_ranges(s, comp1);
    let ranges2 = get_compartment_ranges(s, comp2);
    if ranges1.is_empty() || ranges2.is_empty() {
        return false;
    }

    let sure1 = get_sure_candidates(&ranges1, comp1.len());
    let sure2 = get_sure_candidates(&ranges2, comp2.len());
    let row1 = comp1[0].0;
    let row2 = comp2[0].0;
    let col1 = comp1[0].1;
    let col2 = comp2[0].1;

    if row1 == row2 || col1 == col2 {
        for n in mask_values(sure1) {
            for &(r, c) in comp2 {
                if mask_has(s.candidates[r][c], n) {
                    rem_candidate(s, r, c, n);
                    effective = true;
                }
            }
        }
        for n in mask_values(sure2) {
            for &(r, c) in comp1 {
                if mask_has(s.candidates[r][c], n) {
                    rem_candidate(s, r, c, n);
                    effective = true;
                }
            }
        }
    }

    effective
}

fn use_locked_compartments(s: &mut HumanStr8ts) -> i32 {
    let mut effective = false;

    for r in 0..N {
        let comps: Vec<Compartment> = s
            .row_compartments
            .iter()
            .filter(|comp| !comp.is_empty() && comp[0].0 == r)
            .cloned()
            .collect();
        if comps.len() >= 2 {
            for i in 0..comps.len() {
                for j in (i + 1)..comps.len() {
                    effective |= check_locked_compartments(s, &comps[i], &comps[j]);
                }
            }
        }
    }

    for c in 0..N {
        let comps: Vec<Compartment> = s
            .col_compartments
            .iter()
            .filter(|comp| !comp.is_empty() && comp[0].1 == c)
            .cloned()
            .collect();
        if comps.len() >= 2 {
            for i in 0..comps.len() {
                for j in (i + 1)..comps.len() {
                    effective |= check_locked_compartments(s, &comps[i], &comps[j]);
                }
            }
        }
    }

    if effective { H_LOCKED } else { 0 }
}

fn use_sure_candidates(s: &mut HumanStr8ts) -> i32 {
    let mut effective = false;

    for r in 0..N {
        let row_comp_indices: Vec<usize> = s
            .row_compartments
            .iter()
            .enumerate()
            .filter(|(_, comp)| !comp.is_empty() && comp[0].0 == r)
            .map(|(idx, _)| idx)
            .collect();

        for &idx in &row_comp_indices {
            let comp = &s.row_compartments[idx];
            let ranges = get_compartment_ranges(s, comp);
            let sure = get_sure_candidates(&ranges, comp.len());
            for c in 0..N {
                if !s.is_black[r][c] {
                    let comp_idx = s.cell_to_row_compartment[r][c];
                    if comp_idx != idx {
                        for n in mask_values(sure) {
                            if mask_has(s.candidates[r][c], n) {
                                rem_candidate(s, r, c, n);
                                effective = true;
                            }
                        }
                    }
                }
            }
        }
    }

    for c in 0..N {
        let col_comp_indices: Vec<usize> = s
            .col_compartments
            .iter()
            .enumerate()
            .filter(|(_, comp)| !comp.is_empty() && comp[0].1 == c)
            .map(|(idx, _)| idx)
            .collect();

        for &idx in &col_comp_indices {
            let comp = &s.col_compartments[idx];
            let ranges = get_compartment_ranges(s, comp);
            let sure = get_sure_candidates(&ranges, comp.len());
            for r in 0..N {
                if !s.is_black[r][c] {
                    let comp_idx = s.cell_to_col_compartment[r][c];
                    if comp_idx != idx {
                        for n in mask_values(sure) {
                            if mask_has(s.candidates[r][c], n) {
                                rem_candidate(s, r, c, n);
                                effective = true;
                            }
                        }
                    }
                }
            }
        }
    }

    if effective {
        H_SURE_CANDIDATES
    } else {
        0
    }
}

fn use_sea_creature(s: &mut HumanStr8ts, n: usize) -> i32 {
    for num in 1..=9u8 {
        let mut sure_rows = Vec::<usize>::new();
        let mut row_cells: HashMap<usize, Vec<usize>> = HashMap::new();
        for r in 0..N {
            let mut is_sure = false;
            let mut cols = Vec::<usize>::new();
            for comp in &s.row_compartments {
                if comp.is_empty() || comp[0].0 != r {
                    continue;
                }
                let ranges = get_compartment_ranges(s, comp);
                let sure = get_sure_candidates(&ranges, comp.len());
                if mask_has(sure, num) {
                    is_sure = true;
                    for &(rr, c) in comp {
                        if mask_has(s.candidates[rr][c], num) {
                            cols.push(c);
                        }
                    }
                }
            }
            if is_sure && !cols.is_empty() && cols.len() <= n {
                sure_rows.push(r);
                row_cells.insert(r, cols);
            }
        }

        for rows in combinations(&sure_rows, n) {
            let mut all_cols = HashSet::<usize>::new();
            for &r in &rows {
                if let Some(cols) = row_cells.get(&r) {
                    for &c in cols {
                        all_cols.insert(c);
                    }
                }
            }
            if all_cols.len() == n {
                let row_set: HashSet<usize> = rows.iter().copied().collect();
                let mut effective = false;
                for &c in &all_cols {
                    for r in 0..N {
                        if !row_set.contains(&r) && !s.is_black[r][c] && mask_has(s.candidates[r][c], num) {
                            rem_candidate(s, r, c, num);
                            effective = true;
                        }
                    }
                }
                if effective {
                    return sea_creature_hardness(n);
                }
            }
        }
    }

    for num in 1..=9u8 {
        let mut sure_cols = Vec::<usize>::new();
        let mut col_cells: HashMap<usize, Vec<usize>> = HashMap::new();
        for c in 0..N {
            let mut is_sure = false;
            let mut rows = Vec::<usize>::new();
            for comp in &s.col_compartments {
                if comp.is_empty() || comp[0].1 != c {
                    continue;
                }
                let ranges = get_compartment_ranges(s, comp);
                let sure = get_sure_candidates(&ranges, comp.len());
                if mask_has(sure, num) {
                    is_sure = true;
                    for &(r, cc) in comp {
                        if mask_has(s.candidates[r][cc], num) {
                            rows.push(r);
                        }
                    }
                }
            }
            if is_sure && !rows.is_empty() && rows.len() <= n {
                sure_cols.push(c);
                col_cells.insert(c, rows);
            }
        }

        for cols in combinations(&sure_cols, n) {
            let mut all_rows = HashSet::<usize>::new();
            for &c in &cols {
                if let Some(rows) = col_cells.get(&c) {
                    for &r in rows {
                        all_rows.insert(r);
                    }
                }
            }
            if all_rows.len() == n {
                let col_set: HashSet<usize> = cols.iter().copied().collect();
                let mut effective = false;
                for &r in &all_rows {
                    for c in 0..N {
                        if !col_set.contains(&c) && !s.is_black[r][c] && mask_has(s.candidates[r][c], num) {
                            rem_candidate(s, r, c, num);
                            effective = true;
                        }
                    }
                }
                if effective {
                    return sea_creature_hardness(n);
                }
            }
        }
    }

    0
}

fn setti_row_has_placed(s: &HumanStr8ts, r: usize, num: u8) -> bool {
    (0..N).any(|c| s.numbers[r][c] == num && (s.solved[r][c] || (s.is_black[r][c] && s.numbers[r][c] != 0)))
}

fn setti_col_has_placed(s: &HumanStr8ts, c: usize, num: u8) -> bool {
    (0..N).any(|r| s.numbers[r][c] == num && (s.solved[r][c] || (s.is_black[r][c] && s.numbers[r][c] != 0)))
}

fn setti_row_sure(s: &HumanStr8ts, r: usize, num: u8) -> bool {
    if setti_row_has_placed(s, r, num) {
        return true;
    }
    for comp in &s.row_compartments {
        if comp.is_empty() || comp[0].0 != r {
            continue;
        }
        let ranges = get_compartment_ranges(s, comp);
        let sure = get_sure_candidates(&ranges, comp.len());
        if mask_has(sure, num) {
            return true;
        }
    }
    false
}

fn setti_col_sure(s: &HumanStr8ts, c: usize, num: u8) -> bool {
    if setti_col_has_placed(s, c, num) {
        return true;
    }
    for comp in &s.col_compartments {
        if comp.is_empty() || comp[0].1 != c {
            continue;
        }
        let ranges = get_compartment_ranges(s, comp);
        let sure = get_sure_candidates(&ranges, comp.len());
        if mask_has(sure, num) {
            return true;
        }
    }
    false
}

fn setti_statuses(
    s: &HumanStr8ts,
    num: u8,
    forced_missing_rows: &[bool; 9],
    forced_missing_cols: &[bool; 9],
) -> Option<([SettiStatus; 9], [SettiStatus; 9])> {
    let mut row_status = [SettiStatus::Missing; 9];
    let mut col_status = [SettiStatus::Missing; 9];

    for r in 0..N {
        if forced_missing_rows[r] && setti_row_has_placed(s, r, num) {
            return None;
        }
    }
    for c in 0..N {
        if forced_missing_cols[c] && setti_col_has_placed(s, c, num) {
            return None;
        }
    }

    for r in 0..N {
        if forced_missing_rows[r] {
            row_status[r] = SettiStatus::Missing;
            continue;
        }
        if setti_row_sure(s, r, num) {
            row_status[r] = SettiStatus::Sure;
            continue;
        }
        let mut possible = false;
        for c in 0..N {
            if forced_missing_cols[c] {
                continue;
            }
            if !s.is_black[r][c] && !s.solved[r][c] && mask_has(s.candidates[r][c], num) {
                possible = true;
                break;
            }
        }
        row_status[r] = if possible { SettiStatus::Possible } else { SettiStatus::Missing };
    }

    for c in 0..N {
        if forced_missing_cols[c] {
            col_status[c] = SettiStatus::Missing;
            continue;
        }
        if setti_col_sure(s, c, num) {
            col_status[c] = SettiStatus::Sure;
            continue;
        }
        let mut possible = false;
        for r in 0..N {
            if forced_missing_rows[r] {
                continue;
            }
            if !s.is_black[r][c] && !s.solved[r][c] && mask_has(s.candidates[r][c], num) {
                possible = true;
                break;
            }
        }
        col_status[c] = if possible { SettiStatus::Possible } else { SettiStatus::Missing };
    }

    Some((row_status, col_status))
}

fn setti_missing_bounds(status: &[SettiStatus; 9]) -> (i32, i32) {
    let missing = status.iter().filter(|&&s| s == SettiStatus::Missing).count() as i32;
    let possible = status.iter().filter(|&&s| s == SettiStatus::Possible).count() as i32;
    (missing, missing + possible)
}

fn setti_possible_indices(status: &[SettiStatus; 9]) -> Vec<usize> {
    (0..N).filter(|&i| status[i] == SettiStatus::Possible).collect()
}

fn setti_remove_from_row(s: &mut HumanStr8ts, r: usize, num: u8) -> bool {
    let mut effective = false;
    for c in 0..N {
        if !s.is_black[r][c] && !s.solved[r][c] && mask_has(s.candidates[r][c], num) {
            rem_candidate(s, r, c, num);
            effective = true;
        }
    }
    effective
}

fn setti_remove_from_col(s: &mut HumanStr8ts, c: usize, num: u8) -> bool {
    let mut effective = false;
    for r in 0..N {
        if !s.is_black[r][c] && !s.solved[r][c] && mask_has(s.candidates[r][c], num) {
            rem_candidate(s, r, c, num);
            effective = true;
        }
    }
    effective
}

fn setti_place_if_single_in_row(s: &mut HumanStr8ts, r: usize, num: u8) -> bool {
    if setti_row_has_placed(s, r, num) {
        return false;
    }
    let cells: Vec<(usize, usize)> = (0..N)
        .filter(|&c| !s.is_black[r][c] && !s.solved[r][c] && mask_has(s.candidates[r][c], num))
        .map(|c| (r, c))
        .collect();
    if cells.len() == 1 {
        add(s, cells[0].0, cells[0].1, num);
        return true;
    }
    false
}

fn setti_place_if_single_in_col(s: &mut HumanStr8ts, c: usize, num: u8) -> bool {
    if setti_col_has_placed(s, c, num) {
        return false;
    }
    let cells: Vec<(usize, usize)> = (0..N)
        .filter(|&r| !s.is_black[r][c] && !s.solved[r][c] && mask_has(s.candidates[r][c], num))
        .map(|r| (r, c))
        .collect();
    if cells.len() == 1 {
        add(s, cells[0].0, cells[0].1, num);
        return true;
    }
    false
}

fn use_settis_rule(s: &mut HumanStr8ts) -> i32 {
    let mut effective = false;
    let no_forced_rows = [false; 9];
    let no_forced_cols = [false; 9];

    for num in 1..=9u8 {
        let mut changed = true;
        while changed {
            changed = false;
            let Some((row_status, col_status)) = setti_statuses(s, num, &no_forced_rows, &no_forced_cols) else {
                break;
            };

            let (min_rows, max_rows) = setti_missing_bounds(&row_status);
            let (min_cols, max_cols) = setti_missing_bounds(&col_status);
            let lo = min_rows.max(min_cols);
            let hi = max_rows.min(max_cols);
            if lo > hi {
                break;
            }

            if lo == hi {
                let missing_count = lo;
                let possible_rows = setti_possible_indices(&row_status);
                let possible_cols = setti_possible_indices(&col_status);

                if max_rows == missing_count {
                    for r in possible_rows.iter().copied() {
                        if setti_remove_from_row(s, r, num) {
                            effective = true;
                            changed = true;
                        }
                    }
                }

                if max_cols == missing_count {
                    for c in possible_cols.iter().copied() {
                        if setti_remove_from_col(s, c, num) {
                            effective = true;
                            changed = true;
                        }
                    }
                }

                if min_rows == missing_count {
                    for r in possible_rows.iter().copied() {
                        if setti_place_if_single_in_row(s, r, num) {
                            effective = true;
                            changed = true;
                        }
                    }
                }

                if min_cols == missing_count {
                    for c in possible_cols.iter().copied() {
                        if setti_place_if_single_in_col(s, c, num) {
                            effective = true;
                            changed = true;
                        }
                    }
                }
            }
        }
    }

    if effective { H_SETTI } else { 0 }
}

fn use_setti_consider(s: &mut HumanStr8ts) -> i32 {
    for num in 1..=9u8 {
        let no_forced_rows = [false; 9];
        let no_forced_cols = [false; 9];

        if let Some((_, col_status)) = setti_statuses(s, num, &no_forced_rows, &no_forced_cols) {
            for c in setti_possible_indices(&col_status) {
                let mut forced_cols = [false; 9];
                forced_cols[c] = true;
                let hyp = setti_statuses(s, num, &no_forced_rows, &forced_cols);
                match hyp {
                    None => {
                        if setti_place_if_single_in_col(s, c, num) {
                            return H_SETTI_CONSIDER;
                        }
                    }
                    Some((row_hyp, col_hyp)) => {
                        let (min_rows, max_rows) = setti_missing_bounds(&row_hyp);
                        let (min_cols, max_cols) = setti_missing_bounds(&col_hyp);
                        let lo = min_rows.max(min_cols);
                        let hi = max_rows.min(max_cols);
                        if lo > hi && setti_place_if_single_in_col(s, c, num) {
                            return H_SETTI_CONSIDER;
                        }
                    }
                }
            }
        }

        if let Some((row_status, _)) = setti_statuses(s, num, &no_forced_rows, &no_forced_cols) {
            for r in setti_possible_indices(&row_status) {
                let mut forced_rows = [false; 9];
                forced_rows[r] = true;
                let hyp = setti_statuses(s, num, &forced_rows, &no_forced_cols);
                match hyp {
                    None => {
                        if setti_place_if_single_in_row(s, r, num) {
                            return H_SETTI_CONSIDER;
                        }
                    }
                    Some((row_hyp, col_hyp)) => {
                        let (min_rows, max_rows) = setti_missing_bounds(&row_hyp);
                        let (min_cols, max_cols) = setti_missing_bounds(&col_hyp);
                        let lo = min_rows.max(min_cols);
                        let hi = max_rows.min(max_cols);
                        if lo > hi && setti_place_if_single_in_row(s, r, num) {
                            return H_SETTI_CONSIDER;
                        }
                    }
                }
            }
        }
    }
    0
}

fn use_setti_set(s: &mut HumanStr8ts) -> i32 {
    for set_size in 2..=3 {
        let digits: Vec<u8> = (1..=9).collect();
        for digit_subset in combinations(&digits, set_size) {
            let mut row_by_digit: HashMap<u8, [SettiStatus; 9]> = HashMap::new();
            let mut col_by_digit: HashMap<u8, [SettiStatus; 9]> = HashMap::new();
            let mut valid = true;
            let no_forced_rows = [false; 9];
            let no_forced_cols = [false; 9];

            for &d in &digit_subset {
                let Some((row_status, col_status)) = setti_statuses(s, d, &no_forced_rows, &no_forced_cols) else {
                    valid = false;
                    break;
                };
                row_by_digit.insert(d, row_status);
                col_by_digit.insert(d, col_status);
            }
            if !valid {
                continue;
            }

            let mut row_min = 0;
            let mut row_max = 0;
            let mut col_min = 0;
            let mut col_max = 0;
            for &d in &digit_subset {
                let (mn_r, mx_r) = setti_missing_bounds(row_by_digit.get(&d).unwrap());
                let (mn_c, mx_c) = setti_missing_bounds(col_by_digit.get(&d).unwrap());
                row_min += mn_r;
                row_max += mx_r;
                col_min += mn_c;
                col_max += mx_c;
            }

            let lo = row_min.max(col_min);
            let hi = row_max.min(col_max);
            if lo > hi {
                continue;
            }
            if lo == hi {
                let total_missing = lo;
                let mut effective = false;

                if row_max == total_missing {
                    for &d in &digit_subset {
                        for r in setti_possible_indices(row_by_digit.get(&d).unwrap()) {
                            if setti_remove_from_row(s, r, d) {
                                effective = true;
                            }
                        }
                    }
                }

                if col_max == total_missing {
                    for &d in &digit_subset {
                        for c in setti_possible_indices(col_by_digit.get(&d).unwrap()) {
                            if setti_remove_from_col(s, c, d) {
                                effective = true;
                            }
                        }
                    }
                }

                if row_min == total_missing {
                    for &d in &digit_subset {
                        for r in setti_possible_indices(row_by_digit.get(&d).unwrap()) {
                            if setti_place_if_single_in_row(s, r, d) {
                                effective = true;
                            }
                        }
                    }
                }

                if col_min == total_missing {
                    for &d in &digit_subset {
                        for c in setti_possible_indices(col_by_digit.get(&d).unwrap()) {
                            if setti_place_if_single_in_col(s, c, d) {
                                effective = true;
                            }
                        }
                    }
                }

                if effective {
                    return H_SETTI_SET;
                }
            }
        }
    }
    0
}

fn use_unique_solution_constraint(s: &mut HumanStr8ts) -> i32 {
    for r1 in 0..8 {
        for r2 in (r1 + 1)..9 {
            for c1 in 0..8 {
                for c2 in (c1 + 1)..9 {
                    let cells = [(r1, c1), (r1, c2), (r2, c1), (r2, c2)];
                    if cells.iter().any(|&(r, c)| s.is_black[r][c] || s.solved[r][c]) {
                        continue;
                    }
                    let cand_sets: Vec<u16> = cells.iter().map(|&(r, c)| s.candidates[r][c]).collect();
                    let two_candidate: Vec<u16> = cand_sets.iter().copied().filter(|m| m.count_ones() == 2).collect();
                    if two_candidate.len() >= 3 {
                        let mut pair_counts: HashMap<u16, i32> = HashMap::new();
                        for cs in two_candidate {
                            *pair_counts.entry(cs).or_insert(0) += 1;
                        }
                        for (pair, count) in pair_counts {
                            if count >= 3 {
                                let mut effective = false;
                                for (i, &(r, c)) in cells.iter().enumerate() {
                                    let cs = cand_sets[i];
                                    if (pair & cs) == pair && cs.count_ones() > 2 {
                                        let extras = cs & !pair;
                                        if extras != 0 {
                                            let new_mask = s.candidates[r][c] & !pair;
                                            if new_mask != s.candidates[r][c] {
                                                s.candidates[r][c] = new_mask;
                                                effective = true;
                                            }
                                        }
                                    }
                                }
                                if effective {
                                    return H_UNIQUE;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    0
}

fn use_ywing(s: &mut HumanStr8ts) -> i32 {
    for r in 0..N {
        for c in 0..N {
            if s.is_black[r][c] || s.solved[r][c] || s.candidates[r][c].count_ones() != 2 {
                continue;
            }
            let base = mask_values(s.candidates[r][c]);
            let x = base[0];
            let y = base[1];

            let mut neighbors = HashSet::<(usize, usize)>::new();
            for cc in 0..N {
                if cc != c && !s.is_black[r][cc] && !s.solved[r][cc] {
                    neighbors.insert((r, cc));
                }
            }
            for rr in 0..N {
                if rr != r && !s.is_black[rr][c] && !s.solved[rr][c] {
                    neighbors.insert((rr, c));
                }
            }

            for &(r1, c1) in &neighbors {
                let cands1 = s.candidates[r1][c1];
                if cands1.count_ones() != 2 {
                    continue;
                }
                let base_mask = bit(x) | bit(y);
                let common1 = cands1 & base_mask;
                if common1.count_ones() != 1 {
                    continue;
                }
                let shared1 = mask_single(common1).unwrap();
                let z = mask_single(cands1 & !common1).unwrap();
                let other_base = if shared1 == x { y } else { x };
                let target_wing2 = bit(other_base) | bit(z);

                for &(r2, c2) in &neighbors {
                    if (r2, c2) == (r1, c1) || s.candidates[r2][c2] != target_wing2 {
                        continue;
                    }

                    let mut seen1 = HashSet::<(usize, usize)>::new();
                    let mut seen2 = HashSet::<(usize, usize)>::new();
                    for cc in 0..N {
                        if !s.is_black[r1][cc] {
                            seen1.insert((r1, cc));
                        }
                        if !s.is_black[r2][cc] {
                            seen2.insert((r2, cc));
                        }
                    }
                    for rr in 0..N {
                        if !s.is_black[rr][c1] {
                            seen1.insert((rr, c1));
                        }
                        if !s.is_black[rr][c2] {
                            seen2.insert((rr, c2));
                        }
                    }

                    let mut effective = false;
                    for &(rr, cc) in seen1.intersection(&seen2) {
                        if (rr, cc) != (r, c)
                            && (rr, cc) != (r1, c1)
                            && (rr, cc) != (r2, c2)
                            && !s.solved[rr][cc]
                            && mask_has(s.candidates[rr][cc], z)
                        {
                            rem_candidate(s, rr, cc, z);
                            effective = true;
                        }
                    }

                    if effective {
                        return H_YWING;
                    }
                }
            }
        }
    }
    0
}

fn strategy_order() -> Vec<StrategySpec> {
    let mut specs = Vec::<StrategySpec>::new();
    let mut order = 0;

    order += 1;
    specs.push(StrategySpec {
        hardness: H_SINGLE,
        kind: StrategyKind::Single,
        param: 0,
        order,
    });

    order += 1;
    specs.push(StrategySpec {
        hardness: H_SURE_CANDIDATES,
        kind: StrategyKind::Sure,
        param: 0,
        order,
    });

    for k in 2..=9 {
        order += 1;
        specs.push(StrategySpec {
            hardness: stranded_hardness(k),
            kind: StrategyKind::Stranded,
            param: k,
            order,
        });
    }

    for k in 3..=9 {
        order += 1;
        specs.push(StrategySpec {
            hardness: split_hardness(k),
            kind: StrategyKind::Split,
            param: k,
            order,
        });
    }

    for k in 2..=9 {
        order += 1;
        specs.push(StrategySpec {
            hardness: mindgap_hardness(k),
            kind: StrategyKind::MindGap,
            param: k,
            order,
        });
    }

    for k in 2..=9 {
        order += 1;
        specs.push(StrategySpec {
            hardness: range_check_hardness(k),
            kind: StrategyKind::RangeCheck,
            param: k,
            order,
        });
    }

    for k in 2..=5 {
        order += 1;
        specs.push(StrategySpec {
            hardness: naked_set_hardness(k),
            kind: StrategyKind::NakedSet,
            param: k,
            order,
        });
    }

    for k in 2..=5 {
        order += 1;
        specs.push(StrategySpec {
            hardness: hidden_set_hardness(k),
            kind: StrategyKind::HiddenSet,
            param: k,
            order,
        });
    }

    order += 1;
    specs.push(StrategySpec {
        hardness: H_LOCKED,
        kind: StrategyKind::Locked,
        param: 0,
        order,
    });

    for n in 2..=5 {
        order += 1;
        specs.push(StrategySpec {
            hardness: sea_creature_hardness(n),
            kind: StrategyKind::Sea,
            param: n,
            order,
        });
    }

    order += 1;
    specs.push(StrategySpec {
        hardness: H_UNIQUE,
        kind: StrategyKind::Unique,
        param: 0,
        order,
    });

    order += 1;
    specs.push(StrategySpec {
        hardness: H_SETTI,
        kind: StrategyKind::Setti,
        param: 0,
        order,
    });

    order += 1;
    specs.push(StrategySpec {
        hardness: H_SETTI_CONSIDER,
        kind: StrategyKind::SettiConsider,
        param: 0,
        order,
    });

    order += 1;
    specs.push(StrategySpec {
        hardness: H_SETTI_SET,
        kind: StrategyKind::SettiSet,
        param: 0,
        order,
    });

    order += 1;
    specs.push(StrategySpec {
        hardness: H_YWING,
        kind: StrategyKind::YWing,
        param: 0,
        order,
    });

    order += 1;
    specs.push(StrategySpec {
        hardness: H_BINARY_GUESS,
        kind: StrategyKind::BinaryGuess,
        param: 0,
        order,
    });

    specs.sort_by_key(|s| (s.hardness, s.order));
    specs
}

fn apply_ordered_strategy(s: &mut HumanStr8ts, kind: StrategyKind, param: usize) -> i32 {
    match kind {
        StrategyKind::Single => use_single(s),
        StrategyKind::Sure => use_sure_candidates(s),
        StrategyKind::Stranded => use_stranded_digits(s, param),
        StrategyKind::Split => use_split_compartment(s, param),
        StrategyKind::MindGap => use_mind_the_gap(s, param),
        StrategyKind::RangeCheck => use_compartment_range_check(s, param),
        StrategyKind::NakedSet => use_naked_set(s, param),
        StrategyKind::HiddenSet => use_hidden_set(s, param),
        StrategyKind::Locked => use_locked_compartments(s),
        StrategyKind::Sea => use_sea_creature(s, param),
        StrategyKind::Unique => use_unique_solution_constraint(s),
        StrategyKind::Setti => use_settis_rule(s),
        StrategyKind::SettiConsider => use_setti_consider(s),
        StrategyKind::SettiSet => use_setti_set(s),
        StrategyKind::YWing => use_ywing(s),
        StrategyKind::BinaryGuess => use_binary_guess(s),
    }
}

fn collect_strategy_effects(before: &HumanStr8ts, after: &HumanStr8ts) -> Vec<StrategyEffect> {
    let mut effects = Vec::new();
    for r in 0..N {
        for c in 0..N {
            if before.is_black[r][c] {
                continue;
            }
            let set_value = if !before.solved[r][c] && after.solved[r][c] {
                Some(after.numbers[r][c])
            } else {
                None
            };

            let mut removed_mask = before.candidates[r][c] & !after.candidates[r][c];
            if set_value.is_some() {
                removed_mask = 0;
            }

            if set_value.is_some() || removed_mask != 0 {
                effects.push(StrategyEffect {
                    row: r,
                    col: c,
                    set_value,
                    removed_mask,
                });
            }
        }
    }
    effects
}

fn format_mask_values_csv(mask: u16) -> String {
    let vals = mask_values(mask);
    if vals.is_empty() {
        return "?".to_string();
    }
    vals.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", ")
}

fn build_strategy_description(kind: StrategyKind, param: usize, effects: &[StrategyEffect]) -> String {
    match kind {
        StrategyKind::Single => "Single Candidate Left".to_string(),
        StrategyKind::Sure => "Sure Candidates".to_string(),
        StrategyKind::Stranded => format!("Stranded Digits (length {param})"),
        StrategyKind::Split => format!("Split Compartment (length {param})"),
        StrategyKind::MindGap => format!("Mind The Gap (length {param})"),
        StrategyKind::RangeCheck => format!("Compartment Range Check (length {param})"),
        StrategyKind::NakedSet => {
            let removed_union = effects.iter().fold(0u16, |acc, e| acc | e.removed_mask);
            format!("Naked Set: {}", format_mask_values_csv(removed_union))
        }
        StrategyKind::HiddenSet => {
            let removed_union = effects.iter().fold(0u16, |acc, e| acc | e.removed_mask);
            format!("Hidden Set: {}", format_mask_values_csv(removed_union))
        }
        StrategyKind::Locked => "Locked Compartments".to_string(),
        StrategyKind::Sea => match param {
            2 => "X-Wing".to_string(),
            3 => "Swordfish".to_string(),
            4 => "Jellyfish".to_string(),
            5 => "Squirmbag".to_string(),
            _ => format!("Sea Creature (length {param})"),
        },
        StrategyKind::Unique => "Unique Solution Constraint".to_string(),
        StrategyKind::Setti => "Setti Rule".to_string(),
        StrategyKind::SettiConsider => "Setti Consider".to_string(),
        StrategyKind::SettiSet => "Setti Set".to_string(),
        StrategyKind::YWing => "Y-Wing".to_string(),
        StrategyKind::BinaryGuess => "Binary Guess".to_string(),
    }
}

fn strategy_name(kind: StrategyKind) -> &'static str {
    match kind {
        StrategyKind::Single => "Single",
        StrategyKind::Sure => "SureCandidates",
        StrategyKind::Stranded => "StrandedDigits",
        StrategyKind::Split => "SplitCompartment",
        StrategyKind::MindGap => "MindTheGap",
        StrategyKind::RangeCheck => "CompartmentRangeCheck",
        StrategyKind::NakedSet => "NakedSet",
        StrategyKind::HiddenSet => "HiddenSet",
        StrategyKind::Locked => "LockedCompartments",
        StrategyKind::Sea => "SeaCreature",
        StrategyKind::Unique => "UniqueSolutionConstraint",
        StrategyKind::Setti => "SettiRule",
        StrategyKind::SettiConsider => "SettiConsider",
        StrategyKind::SettiSet => "SettiSet",
        StrategyKind::YWing => "YWing",
        StrategyKind::BinaryGuess => "BinaryGuess",
    }
}

fn run_strategy_on_clone(base: &HumanStr8ts, kind: StrategyKind, param: usize) -> Option<StrategyOutcome> {
    let mut trial = base.clone();
    let hardness = apply_ordered_strategy(&mut trial, kind, param);
    if hardness <= 0 {
        return None;
    }

    let mut effects = collect_strategy_effects(base, &trial);
    if kind == StrategyKind::BinaryGuess {
        append_binary_guess_focus_effect(base, &mut effects);
    }
    if effects.is_empty() {
        return None;
    }

    Some(StrategyOutcome {
        kind,
        hardness,
        description: build_strategy_description(kind, param, &effects),
        effects,
    })
}

fn apply_immediate_effects(s: &mut HumanStr8ts, effects: &[StrategyEffect]) -> Vec<(usize, usize, u8)> {
    let mut placed = Vec::<(usize, usize, u8)>::new();

    for effect in effects {
        let r = effect.row;
        let c = effect.col;

        if let Some(num) = effect.set_value {
            if !s.solved[r][c] {
                add(s, r, c, num);
                placed.push((r, c, num));
            }
            continue;
        }

        if effect.removed_mask != 0 && !s.solved[r][c] {
            s.candidates[r][c] &= !effect.removed_mask;
        }
    }

    placed
}

fn apply_propagation_from_placements(
    s: &mut HumanStr8ts,
    placements: &[(usize, usize, u8)],
) -> Vec<StrategyEffect> {
    let mut removed = [[0u16; N]; N];

    for &(r, c, num) in placements {
        for cc in 0..N {
            if cc == c || s.is_black[r][cc] || s.solved[r][cc] {
                continue;
            }
            if mask_has(s.candidates[r][cc], num) {
                rem_candidate(s, r, cc, num);
                removed[r][cc] |= bit(num);
            }
        }
        for rr in 0..N {
            if rr == r || s.is_black[rr][c] || s.solved[rr][c] {
                continue;
            }
            if mask_has(s.candidates[rr][c], num) {
                rem_candidate(s, rr, c, num);
                removed[rr][c] |= bit(num);
            }
        }
    }

    let mut effects = Vec::new();
    for r in 0..N {
        for c in 0..N {
            if removed[r][c] != 0 {
                effects.push(StrategyEffect {
                    row: r,
                    col: c,
                    set_value: None,
                    removed_mask: removed[r][c],
                });
            }
        }
    }
    effects
}

fn apply_strategy_outcome(s: &mut HumanStr8ts, outcome: &StrategyOutcome) {
    let placed = apply_immediate_effects(s, &outcome.effects);

    for (r, c, num) in placed {
        propagate_add(s, r, c, num);
    }
}

pub fn next_human_step(s: &HumanStr8ts) -> Option<HumanStep> {
    let specs = strategy_order();
    let outcome = find_next_strategy_outcome(s, &specs, true)?;

    Some(HumanStep {
        strategy: strategy_name(outcome.kind).to_string(),
        hardness: outcome.hardness,
        description: outcome.description,
        immediate_effects: outcome.effects,
    })
}

pub fn apply_human_step(s: &mut HumanStr8ts, step: &HumanStep) -> Vec<StrategyEffect> {
    let placements = apply_immediate_effects(s, &step.immediate_effects);
    apply_propagation_from_placements(s, &placements)
}

fn find_next_strategy_outcome(
    s: &HumanStr8ts,
    specs: &[StrategySpec],
    allow_binary_guess: bool,
) -> Option<StrategyOutcome> {
    for spec in specs {
        if !allow_binary_guess && spec.kind == StrategyKind::BinaryGuess {
            continue;
        }
        if let Some(outcome) = run_strategy_on_clone(s, spec.kind, spec.param) {
            return Some(outcome);
        }
    }
    None
}

fn apply_ordered_strategy_with_post_propagation(s: &mut HumanStr8ts, kind: StrategyKind, param: usize) -> i32 {
    let mut was_solved = [[false; N]; N];
    for r in 0..N {
        for c in 0..N {
            was_solved[r][c] = s.solved[r][c];
        }
    }

    let h = apply_ordered_strategy(s, kind, param);
    if h <= 0 {
        return h;
    }

    for r in 0..N {
        for c in 0..N {
            if !was_solved[r][c] && s.solved[r][c] {
                propagate_add(s, r, c, s.numbers[r][c]);
            }
        }
    }

    h
}

fn propagate_without_binary_guess_for_trial(s: &mut HumanStr8ts, specs: &[StrategySpec]) {
    while !is_done(s) && is_valid(s) {
        let mut progress = false;
        for spec in specs {
            if spec.kind == StrategyKind::BinaryGuess {
                continue;
            }
            let h = apply_ordered_strategy_with_post_propagation(s, spec.kind, spec.param);
            if h > 0 {
                progress = true;
                break;
            }
        }
        if !progress {
            break;
        }
    }
}

fn collect_trial_removals(base: &HumanStr8ts, trial: &HumanStr8ts) -> HashMap<(usize, usize), u16> {
    let mut removed = HashMap::new();
    for r in 0..N {
        for c in 0..N {
            if base.is_black[r][c] || base.solved[r][c] {
                continue;
            }
            let missing = base.candidates[r][c] & !trial.candidates[r][c];
            if missing != 0 {
                removed.insert((r, c), missing);
            }
        }
    }
    removed
}

#[derive(Debug, Clone)]
struct BinaryGuessOutcome {
    focus_row: usize,
    focus_col: usize,
    forced_value: Option<u8>,
    common_removals: Vec<(usize, usize, u16)>,
}

fn analyze_binary_guess(s: &HumanStr8ts, specs: &[StrategySpec]) -> Option<BinaryGuessOutcome> {
    for r in 0..N {
        for c in 0..N {
            if s.is_black[r][c] || s.solved[r][c] || s.candidates[r][c].count_ones() != 2 {
                continue;
            }
            let mut vals = mask_values(s.candidates[r][c]);
            vals.sort_unstable();

            let mut contradiction = [false; 2];
            let mut trials = vec![s.clone(), s.clone()];
            for i in 0..2 {
                let mut trial = s.clone();
                add_with_propagation(&mut trial, r, c, vals[i]);
                propagate_without_binary_guess_for_trial(&mut trial, specs);
                contradiction[i] = !is_valid(&trial);
                trials[i] = trial;
            }

            if contradiction[0] != contradiction[1] {
                let chosen = if contradiction[0] { vals[1] } else { vals[0] };
                return Some(BinaryGuessOutcome {
                    focus_row: r,
                    focus_col: c,
                    forced_value: Some(chosen),
                    common_removals: Vec::new(),
                });
            }

            if !contradiction[0] && !contradiction[1] {
                let removed1 = collect_trial_removals(s, &trials[0]);
                let removed2 = collect_trial_removals(s, &trials[1]);

                let mut common_removals = Vec::<(usize, usize, u16)>::new();
                for (cell, rem1) in &removed1 {
                    if let Some(rem2) = removed2.get(cell) {
                        let common = rem1 & rem2;
                        if common == 0 {
                            continue;
                        }
                        let (rr, cc) = *cell;
                        let old = s.candidates[rr][cc];
                        let new_mask = old & !common;
                        if new_mask != old {
                            common_removals.push((rr, cc, common));
                        }
                    }
                }

                if !common_removals.is_empty() {
                    return Some(BinaryGuessOutcome {
                        focus_row: r,
                        focus_col: c,
                        forced_value: None,
                        common_removals,
                    });
                }
            }
        }
    }

    None
}

fn use_binary_guess(s: &mut HumanStr8ts) -> i32 {
    let specs = strategy_order();
    if let Some(outcome) = analyze_binary_guess(s, &specs) {
        if let Some(chosen) = outcome.forced_value {
            add(s, outcome.focus_row, outcome.focus_col, chosen);
            return H_BINARY_GUESS;
        }

        let mut effective = false;
        for (rr, cc, common) in outcome.common_removals {
            let old = s.candidates[rr][cc];
            let new_mask = old & !common;
            if new_mask != old {
                s.candidates[rr][cc] = new_mask;
                effective = true;
            }
        }
        if effective {
            return H_BINARY_GUESS;
        }
    }

    0
}

fn append_binary_guess_focus_effect(s: &HumanStr8ts, effects: &mut Vec<StrategyEffect>) {
    let specs = strategy_order();
    let Some(outcome) = analyze_binary_guess(s, &specs) else {
        return;
    };

    if effects
        .iter()
        .any(|e| e.row == outcome.focus_row && e.col == outcome.focus_col)
    {
        return;
    }

    effects.push(StrategyEffect {
        row: outcome.focus_row,
        col: outcome.focus_col,
        set_value: None,
        removed_mask: 0,
    });
}

pub fn solve_human(s: &mut HumanStr8ts) -> HumanSolveResult {
    let specs = strategy_order();
    let mut move_hardnesses = Vec::<i32>::new();
    let mut _move_descriptions = Vec::<String>::new();

    while !is_done(s) && is_valid(s) {
        let Some(outcome) = find_next_strategy_outcome(s, &specs, true) else {
            break;
        };
        move_hardnesses.push(outcome.hardness);
        _move_descriptions.push(outcome.description.clone());
        apply_strategy_outcome(s, &outcome);
    }

    if !is_valid(s) {
        return HumanSolveResult {
            move_hardnesses: vec![-1],
        };
    }

    if !is_done(s) {
        move_hardnesses.push(H_UNSOLVABLE);
    }

    HumanSolveResult { move_hardnesses }
}

pub fn puzzle_hardness(moves: &[i32]) -> i32 {
    *moves.iter().max().unwrap_or(&0)
}
