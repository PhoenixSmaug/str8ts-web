use crate::board::{SimpleStr8ts, N};
use anyhow::Result;
use rustsat::solvers::{Solve, SolverResult};
use rustsat::types::{Clause, Lit, TernaryVal};
use rustsat_batsat::BasicSolver;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SatPuzzleStatus {
    Invalid,
    NonUnique,
    Unique,
}

pub fn solve_sat(s: &mut SimpleStr8ts) -> Result<bool> {
    let cnf = encode(s);
    let mut solver = BasicSolver::default();

    for clause in cnf {
        solver.add_clause(clause)?;
    }

    if solver.solve()? != SolverResult::Sat {
        return Ok(false);
    }

    for x in 0..N {
        for y in 0..N {
            if s.is_black[x][y] {
                continue;
            }
            for n in 1..=9 {
                let lit = Lit::from_ipasir(var_id(x, y, n)).expect("valid lit");
                if solver.lit_val(lit)? == TernaryVal::True {
                    s.numbers[x][y] = n as u8;
                    break;
                }
            }
        }
    }

    Ok(true)
}

pub fn puzzle_status_sat(s: &SimpleStr8ts) -> Result<SatPuzzleStatus> {
    let cnf = encode(s);
    let mut solver = BasicSolver::default();

    for clause in cnf {
        solver.add_clause(clause)?;
    }

    if solver.solve()? != SolverResult::Sat {
        return Ok(SatPuzzleStatus::Invalid);
    }

    let mut first_solution = [[0u8; N]; N];
    for x in 0..N {
        for y in 0..N {
            if s.is_black[x][y] {
                continue;
            }
            for n in 1..=9 {
                let lit = Lit::from_ipasir(var_id(x, y, n)).expect("valid lit");
                if solver.lit_val(lit)? == TernaryVal::True {
                    first_solution[x][y] = n as u8;
                    break;
                }
            }
        }
    }

    let mut blocking_vals = Vec::<i32>::new();
    for x in 0..N {
        for y in 0..N {
            if s.is_black[x][y] {
                continue;
            }
            let n = first_solution[x][y];
            if n == 0 {
                return Ok(SatPuzzleStatus::Invalid);
            }
            blocking_vals.push(-var_id(x, y, n as usize));
        }
    }
    solver.add_clause(Clause::from(
        blocking_vals
            .iter()
            .copied()
            .map(|v| Lit::from_ipasir(v).expect("valid ipasir literal"))
            .collect::<Vec<_>>()
            .as_slice(),
    ))?;

    if solver.solve()? == SolverResult::Sat {
        Ok(SatPuzzleStatus::NonUnique)
    } else {
        Ok(SatPuzzleStatus::Unique)
    }
}

fn var_id(x: usize, y: usize, n: usize) -> i32 {
    (x * 81 + y * 9 + n) as i32
}

fn encode(s: &SimpleStr8ts) -> Vec<Clause> {
    let mut cnf = Vec::new();

    for x in 0..N {
        for y in 0..N {
            if s.numbers[x][y] != 0 {
                add_clause(&mut cnf, vec![var_id(x, y, s.numbers[x][y] as usize)]);
            }
        }
    }

    for x in 0..N {
        for y in 0..N {
            if s.is_black[x][y] && s.numbers[x][y] == 0 {
                for n in 1..=9 {
                    add_clause(&mut cnf, vec![-var_id(x, y, n)]);
                }
                continue;
            }

            add_clause(&mut cnf, (1..=9).map(|n| var_id(x, y, n)).collect());

            for n1 in 1..=9 {
                for n2 in (n1 + 1)..=9 {
                    add_clause(&mut cnf, vec![-var_id(x, y, n1), -var_id(x, y, n2)]);
                }
            }
        }
    }

    for n in 1..=9 {
        for x in 0..N {
            for y1 in 0..N {
                for y2 in (y1 + 1)..N {
                    add_clause(&mut cnf, vec![-var_id(x, y1, n), -var_id(x, y2, n)]);
                }
            }
        }

        for y in 0..N {
            for x1 in 0..N {
                for x2 in (x1 + 1)..N {
                    add_clause(&mut cnf, vec![-var_id(x1, y, n), -var_id(x2, y, n)]);
                }
            }
        }
    }

    for x in 0..N {
        for y in 0..N {
            if s.is_black[x][y] {
                continue;
            }

            if y == 0 || s.is_black[x][y - 1] {
                let mut comp = Vec::new();
                let mut j = y;
                while j < N && !s.is_black[x][j] {
                    comp.push((x, j));
                    j += 1;
                }
                encode_compartment(&mut cnf, &comp);
            }

            if x == 0 || s.is_black[x - 1][y] {
                let mut comp = Vec::new();
                let mut i = x;
                while i < N && !s.is_black[i][y] {
                    comp.push((i, y));
                    i += 1;
                }
                encode_compartment(&mut cnf, &comp);
            }
        }
    }

    cnf
}

fn encode_compartment(cnf: &mut Vec<Clause>, comp: &[(usize, usize)]) {
    let m = comp.len();
    if !(2..=8).contains(&m) {
        return;
    }

    for &(x1, y1) in comp {
        for &(x2, y2) in comp {
            if (x1, y1) == (x2, y2) {
                continue;
            }
            for n1 in 1..=9 {
                for n2 in (n1 + m)..=9 {
                    add_clause(cnf, vec![-var_id(x1, y1, n1), -var_id(x2, y2, n2)]);
                }
                for n2 in 1..=n1.saturating_sub(m) {
                    add_clause(cnf, vec![-var_id(x1, y1, n1), -var_id(x2, y2, n2)]);
                }
            }
        }
    }
}

fn add_clause(cnf: &mut Vec<Clause>, vals: Vec<i32>) {
    let lits: Vec<Lit> = vals
        .into_iter()
        .map(|v| Lit::from_ipasir(v).expect("valid ipasir literal"))
        .collect();
    cnf.push(Clause::from(lits.as_slice()));
}
