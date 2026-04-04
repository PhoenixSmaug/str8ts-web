use std::collections::VecDeque;

pub fn has_perfect_matching(left_adj: &[Vec<usize>], right_size: usize) -> bool {
    hopcroft_karp_size(left_adj, right_size) == left_adj.len()
}

pub fn hopcroft_karp_size(left_adj: &[Vec<usize>], right_size: usize) -> usize {
    let n_left = left_adj.len();
    let mut pair_u = vec![None; n_left];
    let mut pair_v = vec![None; right_size];
    let mut dist = vec![0usize; n_left];

    let mut matching = 0usize;
    while bfs(left_adj, &pair_u, &pair_v, &mut dist) {
        for u in 0..n_left {
            if pair_u[u].is_none() && dfs(u, left_adj, &mut pair_u, &mut pair_v, &mut dist) {
                matching += 1;
            }
        }
    }
    matching
}

fn bfs(left_adj: &[Vec<usize>], pair_u: &[Option<usize>], pair_v: &[Option<usize>], dist: &mut [usize]) -> bool {
    let mut q = VecDeque::new();
    let inf = usize::MAX / 4;

    for u in 0..left_adj.len() {
        if pair_u[u].is_none() {
            dist[u] = 0;
            q.push_back(u);
        } else {
            dist[u] = inf;
        }
    }

    let mut found = false;
    while let Some(u) = q.pop_front() {
        for &v in &left_adj[u] {
            if let Some(u2) = pair_v[v] {
                if dist[u2] == inf {
                    dist[u2] = dist[u] + 1;
                    q.push_back(u2);
                }
            } else {
                found = true;
            }
        }
    }

    found
}

fn dfs(
    u: usize,
    left_adj: &[Vec<usize>],
    pair_u: &mut [Option<usize>],
    pair_v: &mut [Option<usize>],
    dist: &mut [usize],
) -> bool {
    for &v in &left_adj[u] {
        if let Some(u2) = pair_v[v] {
            if dist[u2] == dist[u] + 1 && dfs(u2, left_adj, pair_u, pair_v, dist) {
                pair_u[u] = Some(v);
                pair_v[v] = Some(u);
                return true;
            }
        } else {
            pair_u[u] = Some(v);
            pair_v[v] = Some(u);
            return true;
        }
    }
    dist[u] = usize::MAX / 4;
    false
}
