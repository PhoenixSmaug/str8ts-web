/* tslint:disable */
/* eslint-disable */

export function apply_hint(board: string, candidates_json: string): string;

export function compute_hint(board: string, candidates_json: string): string;

export function creator_validate_board(board: string): string;

/**
 * Generate a puzzle via WASM.
 * `difficulty` is one of: "easy", "medium", "hard", "diabolic", "cruel", "extreme".
 * `symmetric` controls 180° rotational symmetry of the black-tile pattern.
 * Returns a JSON object: `{ok, puzzle, solution}` or `{ok:false, error}`.
 */
export function generate_puzzle_wasm(difficulty_str: string, symmetric: boolean): string;

export function human_single_step(board: string, candidates_json: string): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly apply_hint: (a: number, b: number, c: number, d: number) => [number, number];
    readonly creator_validate_board: (a: number, b: number) => [number, number];
    readonly generate_puzzle_wasm: (a: number, b: number, c: number) => [number, number];
    readonly compute_hint: (a: number, b: number, c: number, d: number) => [number, number];
    readonly human_single_step: (a: number, b: number, c: number, d: number) => [number, number];
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
