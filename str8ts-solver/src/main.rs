mod backtrack;
mod board;
mod generator;
mod hopcroft_karp;
mod human;
mod sat;

use anyhow::Result;
use backtrack::solve_simple;
use board::{HumanStr8ts, SimpleStr8ts};
use generator::Difficulty;
use human::{puzzle_hardness, solve_human};
use sat::solve_sat;
use std::env;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy)]
enum SolverKind {
    Backtracking,
    Sat,
    Human,
}

impl SolverKind {
    fn parse(value: &str) -> Option<Self> {
        match value.to_lowercase().as_str() {
            "backtracking" | "backtrack" | "simple" => Some(Self::Backtracking),
            "sat" => Some(Self::Sat),
            "human" => Some(Self::Human),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Backtracking => "Backtracking",
            Self::Sat => "SAT (RustSAT + BatSat)",
            Self::Human => "Human",
        }
    }
}

#[derive(Debug, Default)]
struct CliOptions {
    solver: Option<String>,
    puzzle: Option<String>,
    small_test: bool,
    /// Puzzle generation: "<n> <difficulty> [sym|asym]"
    generate: Option<(usize, String, bool)>,
    /// Benchmark mode: generate 100 puzzles of each difficulty and report timing
    generate_bench: bool,
}

fn parse_args() -> Result<CliOptions> {
    let mut opts = CliOptions::default();

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--solver" => {
                let value = args.next().ok_or_else(|| anyhow::anyhow!("--solver needs a value"))?;
                opts.solver = Some(value);
            }
            "--puzzle" => {
                let value = args.next().ok_or_else(|| anyhow::anyhow!("--puzzle needs a value"))?;
                opts.puzzle = Some(value);
            }
            "--generate" => {
                let n = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("--generate needs <n>"))?
                    .parse::<usize>()?;
                let diff = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("--generate needs <difficulty>"))?;
                let sym_str = args.next().unwrap_or_else(|| "asym".to_string());
                let symmetric = sym_str == "sym";
                opts.generate = Some((n, diff, symmetric));
            }
            "--gen-bench" => {
                opts.generate_bench = true;
            }
            "--small-test" => {
                opts.small_test = true;
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            other => {
                anyhow::bail!("Unknown argument: {other}");
            }
        }
    }

    Ok(opts)
}

fn print_help() {
    println!("Str8ts Rust Solver CLI");
    println!("\nSmall test mode (fixed one-puzzle demo):");
    println!("  cargo run --release -- --small-test");
    println!("\nSingle puzzle mode:");
    println!("  cargo run --release -- --solver <backtracking|sat|human> --puzzle <81-char-board>");
    println!("\nPuzzle generation:");
    println!("  cargo run --release -- --generate <n> <difficulty> [sym|asym]");
    println!("  difficulty: easy  medium  hard  diabolic");
    println!("  Prints n \"<puzzle> <solution>\" lines (asym = no enforced symmetry, default).");
    println!("\nGeneration benchmark (100 puzzles × 4 difficulties):");
    println!("  cargo run --release -- --gen-bench");
}

// Diabolic puzzle under CC BY-SA 3.0 license, https://de.wikipedia.org/wiki/Str8ts#/media/Datei:Str8ts9x9_Very_Hard_PUZ.png
const SMALL_TEST_PUZZLE: &str = "##2..#9..#....h6.#52..f...#..........#a5..##.........6#...e..3.h..i....#...#.2.d#";

fn run_small_test() -> Result<()> {
    println!("Using Backtracking Solver:\n");
    {
        let mut s = SimpleStr8ts::from_str(SMALL_TEST_PUZZLE)?;
        println!("{}", s);
        solve_simple(&mut s);
        println!("{}", s);
    }

    println!("\nUsing SAT Solver:\n");
    {
        let mut s = SimpleStr8ts::from_str(SMALL_TEST_PUZZLE)?;
        println!("{}", s);
        solve_sat(&mut s)?;
        println!("{}", s);
    }

    println!("\nUsing Human Solver:\n");
    {
        let mut s = HumanStr8ts::from_str(SMALL_TEST_PUZZLE)?;
        println!("{}", s);
        let moves = solve_human(&mut s);
        println!("{}", s);
        println!("Hardest move hardness: {}", puzzle_hardness(&moves.move_hardnesses));
    }

    Ok(())
}

fn run_single_puzzle(solver: SolverKind, puzzle: &str) -> Result<()> {
    println!("Solver: {}", solver.label());
    match solver {
        SolverKind::Backtracking => {
            let mut s = SimpleStr8ts::from_str(puzzle)?;
            println!("Input:\n{}", s);
            let ok = solve_simple(&mut s);
            println!("Solved: {ok}");
            println!("Output:\n{}", s);
        }
        SolverKind::Sat => {
            let mut s = SimpleStr8ts::from_str(puzzle)?;
            println!("Input:\n{}", s);
            let ok = solve_sat(&mut s)?;
            println!("Solved: {ok}");
            println!("Output:\n{}", s);
        }
        SolverKind::Human => {
            let mut s = HumanStr8ts::from_str(puzzle)?;
            println!("Input:\n{}", s);
            let result = solve_human(&mut s);
            println!("Hardest move hardness: {}", puzzle_hardness(&result.move_hardnesses));
            println!("Output:\n{}", s);
        }
    }

    Ok(())
}

fn main() -> Result<()> {
    let opts = parse_args()?;

    if opts.small_test {
        return run_small_test();
    }

    let solver_kind = if let Some(solver) = opts.solver.as_deref() {
        Some(SolverKind::parse(solver).ok_or_else(|| anyhow::anyhow!("Unknown solver: {solver}"))?)
    } else {
        None
    };

    if let (Some(kind), Some(puzzle)) = (solver_kind, opts.puzzle.as_deref()) {
        return run_single_puzzle(kind, puzzle);
    }

    if let Some((n, diff_str, symmetric)) = opts.generate {
        return run_generate(n, &diff_str, symmetric);
    }

    if opts.generate_bench {
        return run_gen_bench();
    }

    print_help();
    Ok(())
}

fn run_generate(n: usize, diff_str: &str, symmetric: bool) -> Result<()> {
    let difficulty = Difficulty::from_str(diff_str)
        .ok_or_else(|| anyhow::anyhow!("Unknown difficulty '{}'; use: easy medium hard diabolic", diff_str))?;

    let sym_label = if symmetric { "sym" } else { "asym" };
    eprintln!("Generating {} {} {} puzzle(s)...", n, difficulty.name(), sym_label);

    for i in 1..=n {
        let start = Instant::now();
        match generator::generate_puzzle(difficulty, symmetric, 500)? {
            Some((puzzle, solution)) => {
                let elapsed = start.elapsed();
                println!("{puzzle} {solution}");
                eprintln!("  [{i}/{n}] {:.3}s", elapsed.as_secs_f64());
            }
            None => {
                anyhow::bail!("Could not generate puzzle {} after 500 attempts", i);
            }
        }
    }
    Ok(())
}

fn run_gen_bench() -> Result<()> {
    let difficulties = [Difficulty::Easy, Difficulty::Medium, Difficulty::Hard, Difficulty::Diabolic];
    let n = 100usize;
    let timeout_per = Duration::from_secs(1);

    println!("Generation benchmark: {} puzzles per difficulty (1s timeout each)", n);
    println!("{:<12} {:>10} {:>10} {:>10} {:>10} {:>8}", "Difficulty", "Generated", "Failed", "Total(s)", "Avg(ms)", "Max(ms)");
    println!("{}", "-".repeat(65));

    for &diff in &difficulties {
        let mut generated = 0usize;
        let mut failed = 0usize;
        let mut total_ms = 0.0f64;
        let mut max_ms = 0.0f64;

        for _ in 0..n {
            let start = Instant::now();
            match generator::generate_puzzle(diff, false, 300)? {
                Some(_) => {
                    let ms = start.elapsed().as_secs_f64() * 1000.0;
                    generated += 1;
                    total_ms += ms;
                    if ms > max_ms {
                        max_ms = ms;
                    }
                    if start.elapsed() > timeout_per {
                        eprintln!("  WARN: {} puzzle exceeded 1s ({:.1}ms)", diff.name(), ms);
                    }
                }
                None => {
                    failed += 1;
                }
            }
        }

        let avg_ms = if generated > 0 { total_ms / generated as f64 } else { 0.0 };
        println!(
            "{:<12} {:>10} {:>10} {:>10.2} {:>10.1} {:>8.1}",
            diff.name(),
            generated,
            failed,
            total_ms / 1000.0,
            avg_ms,
            max_ms
        );
    }

    Ok(())
}
