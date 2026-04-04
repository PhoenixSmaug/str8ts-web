mod backtrack;
mod board;
mod generator;
mod hopcroft_karp;
mod human;
mod sat;
use generator::Difficulty;

use board::{HumanStr8ts, N};
use human::{apply_human_step, next_human_step, StrategyEffect};
use rand::SeedableRng;
use sat::{puzzle_status_sat, solve_sat, SatPuzzleStatus};
use serde::Serialize;
use serde_json::Value;
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
struct EffectDto {
    row: usize,
    col: usize,
    set_value: Option<u8>,
    removed: Vec<u8>,
}

#[derive(Serialize)]
struct StepResponse {
    ok: bool,
    strategy: String,
    description: String,
    immediate_effects: Vec<EffectDto>,
    propagation_effects: Vec<EffectDto>,
}

#[derive(Serialize)]
struct ErrorResponse {
    ok: bool,
    error: String,
}

#[derive(Serialize)]
struct CreatorValidationResponse {
    ok: bool,
    valid: bool,
    unique: bool,
    message: String,
    solution: Option<String>,
}

fn mask_values(mask: u16) -> Vec<u8> {
    (1..=9).filter(|&n| (mask & (1u16 << (n - 1))) != 0).collect()
}

fn to_effect_dto(effects: &[StrategyEffect]) -> Vec<EffectDto> {
    effects
        .iter()
        .map(|e| EffectDto {
            row: e.row,
            col: e.col,
            set_value: e.set_value,
            removed: mask_values(e.removed_mask),
        })
        .collect()
}

fn candidate_mask_from_digit_list(value: &Value) -> anyhow::Result<u16> {
    let arr = value
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Candidate entry must be an array of digits"))?;
    let mut mask = 0u16;
    for v in arr {
        let d = v
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("Candidate digit must be an integer"))?;
        if !(1..=9).contains(&d) {
            anyhow::bail!("Candidate digit out of range: {d}");
        }
        mask |= 1u16 << (d as u8 - 1);
    }
    Ok(mask)
}

fn apply_candidates_json(s: &mut HumanStr8ts, candidates_json: &str) -> anyhow::Result<()> {
    let trimmed = candidates_json.trim();
    if trimmed.is_empty() || trimmed == "null" {
        return Ok(());
    }

    let value: Value = serde_json::from_str(trimmed)?;
    let rows = value
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Candidates JSON must be a 9x9 array"))?;
    if rows.len() != N {
        anyhow::bail!("Candidates JSON must have 9 rows");
    }

    for r in 0..N {
        let cols = rows[r]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("Candidates row must be an array"))?;
        if cols.len() != N {
            anyhow::bail!("Candidates row {r} must have 9 columns");
        }

        for c in 0..N {
            if s.is_black[r][c] || s.solved[r][c] {
                continue;
            }

            let mask = if let Some(m) = cols[c].as_u64() {
                if m > 0x01FF {
                    anyhow::bail!("Candidate mask out of range at ({r},{c})");
                }
                m as u16
            } else {
                candidate_mask_from_digit_list(&cols[c])?
            };

            if mask != 0 {
                s.candidates[r][c] &= mask;
            }
        }
    }

    Ok(())
}

fn json_error(message: impl Into<String>) -> String {
    serde_json::to_string(&ErrorResponse {
        ok: false,
        error: message.into(),
    })
    .unwrap_or_else(|_| "{\"ok\":false,\"error\":\"serialization failure\"}".to_string())
}

#[wasm_bindgen]
pub fn human_single_step(board: &str, candidates_json: &str) -> String {
    let mut state = match HumanStr8ts::from_str(board) {
        Ok(v) => v,
        Err(e) => return json_error(format!("Invalid board: {e}")),
    };

    if let Err(e) = apply_candidates_json(&mut state, candidates_json) {
        return json_error(format!("Invalid candidates_json: {e}"));
    }

    let Some(step) = next_human_step(&state) else {
        return json_error("No applicable human strategy found");
    };

    let immediate = step.immediate_effects.clone();
    let propagation = apply_human_step(&mut state, &step);

    serde_json::to_string(&StepResponse {
        ok: true,
        strategy: step.strategy,
        description: step.description,
        immediate_effects: to_effect_dto(&immediate),
        propagation_effects: to_effect_dto(&propagation),
    })
    .unwrap_or_else(|_| json_error("serialization failure"))
}

#[wasm_bindgen]
pub fn compute_hint(board: &str, candidates_json: &str) -> String {
    human_single_step(board, candidates_json)
}

#[wasm_bindgen]
pub fn apply_hint(board: &str, candidates_json: &str) -> String {
    human_single_step(board, candidates_json)
}

#[wasm_bindgen]
pub fn creator_validate_board(board: &str) -> String {
    let simple = match board::SimpleStr8ts::from_str(board) {
        Ok(v) => v,
        Err(e) => {
            return serde_json::to_string(&CreatorValidationResponse {
                ok: false,
                valid: false,
                unique: false,
                message: format!("Invalid board string: {e}"),
                solution: None,
            })
            .unwrap_or_else(|_| json_error("serialization failure"));
        }
    };

    let status = match puzzle_status_sat(&simple) {
        Ok(v) => v,
        Err(e) => {
            return serde_json::to_string(&CreatorValidationResponse {
                ok: false,
                valid: false,
                unique: false,
                message: format!("SAT validation failed: {e}"),
                solution: None,
            })
            .unwrap_or_else(|_| json_error("serialization failure"));
        }
    };

    let (valid, unique, message) = match status {
        SatPuzzleStatus::Invalid => (false, false, "Invalid puzzle (no solution)".to_string()),
        SatPuzzleStatus::NonUnique => (true, false, "Valid but non-unique".to_string()),
        SatPuzzleStatus::Unique => (true, true, "Valid and unique".to_string()),
    };

    let solution = if valid {
        let mut solved = simple.clone();
        match solve_sat(&mut solved) {
            Ok(true) => Some(solved.solution_string()),
            _ => None,
        }
    } else {
        None
    };

    serde_json::to_string(&CreatorValidationResponse {
        ok: true,
        valid,
        unique,
        message,
        solution,
    })
    .unwrap_or_else(|_| json_error("serialization failure"))
}

#[derive(Serialize)]
struct GenerateResponse {
    ok: bool,
    puzzle: String,
    solution: String,
}

/// Generate a puzzle via WASM.
/// `difficulty` is one of: "easy", "medium", "hard", "diabolic", "cruel", "extreme".
/// `symmetric` controls 180° rotational symmetry of the black-tile pattern.
/// Returns a JSON object: `{ok, puzzle, solution}` or `{ok:false, error}`.
#[wasm_bindgen]
pub fn generate_puzzle_wasm(difficulty_str: &str, symmetric: bool) -> String {
    let difficulty = match Difficulty::from_str(difficulty_str) {
        Some(d) => d,
        None => {
            return json_error(format!(
                "Unknown difficulty '{}'; use: easy medium hard diabolic cruel extreme",
                difficulty_str
            ));
        }
    };

    // Use a single RNG seeded from entropy (getrandom with js feature handles WASM)
    let mut rng = rand::rngs::StdRng::from_entropy();

    match generator::generate_puzzle_with_rng(&mut rng, difficulty, symmetric, 300) {
        Ok(Some((puzzle, solution))) => {
            serde_json::to_string(&GenerateResponse { ok: true, puzzle, solution })
                .unwrap_or_else(|_| json_error("serialization failure"))
        }
        Ok(None) => json_error("Failed to generate puzzle after 300 attempts"),
        Err(e) => json_error(format!("Generator error: {e}")),
    }
}
