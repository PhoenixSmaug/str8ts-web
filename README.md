# Str8ts Web

An interactive web app for playing, solving, and creating [Str8ts](https://en.wikipedia.org/wiki/Str8ts) puzzles, powered by a Rust solver and puzzle generator compiled to WebAssembly.

🌐 **Live Demo:** [phoenixsmaug.github.io/str8ts-web](https://phoenixsmaug.github.io/str8ts-web/)

---

## Legal Disclaimer

This project is an independent open source website for the puzzle game Str8ts, developed purely for educational and recreational purposes. Str8ts is a registered trademark of Syndicated Puzzles Inc. All rights to the name, branding, and associated intellectual property belong to them. The author of this project is not affiliated, associated, authorized, endorsed by, or in any way officially connected with Syndicated Puzzles Inc., or any of its subsidiaries or affiliates.

---

## Overview

This project is a modern, responsive web app built with vanilla JavaScript and Vite. The puzzle engine — including the generator, human-strategy solver, and SAT-based uniqueness checker — is written in Rust for performance reasons and compiled to WASM, running entirely in the browser with no server required.

### Features

- **Puzzle Mode** — Play generated puzzles across six difficulty levels (Easy → Extreme)
- **Solver Mode** — Request step-by-step hints based on the same human strategies used to rate puzzles
- **Creator Mode** — Build and export your own puzzles using a compact 81-character board string format
- **Responsive layout** — Works on desktop and mobile

### Puzzle Generator

The generator follows a five-step pipeline inspired by the [German Wikipedia article](https://de.wikipedia.org/wiki/Str8ts#Erzeugen_von_Str8ts-R%C3%A4tseln):

1. Sample a black-tile pattern from logit tables
2. Fill white cells with a valid solution via a SAT solver
3. Assign valid clue digits to black cells
4. Strip white and black cell clues while the puzzle remains uniquely solvable (SAT uniqueness gate)
5. Restore hints one-by-one with a spread heuristic until the puzzle lands in the target hardness band

### Human Strategies & Difficulty Rating

The solver and generator both rely on a catalogue of human solving strategies. A full explanation of all strategies is available in [this SlideShare presentation](https://www.slideshare.net/slideshow/str8ts-basic-and-advanced-strategies/8426761).

Each strategy is assigned a hardness score in [`str8ts-solver/src/human.rs`](str8ts-solver/src/human.rs) from 0 (easy) to 99 (hard). The overall puzzle hardness is the maximum single-move hardness across the entire solving process. To adjust how hard an individual strategy is considered, simply change its score in that file.

The hardness bands for each difficulty level are defined in [`str8ts-solver/src/generator.rs`](str8ts-solver/src/generator.rs) via `max_hardness` (the upper bound accepted after hint restoration) and `min_hardness` (the minimum required of the fully-stripped puzzle, acting as a quality floor). Edit these values to widen, narrow, or shift any difficulty tier then [rebuild the WASM](#rebuilding-the-wasm).

### Board String Format

Each puzzle is encoded as an 81-character string (row-major, 9×9):

| Character | Meaning |
|-----------|---------|
| `.` | Empty white cell |
| `1`–`9` | White cell with hint digit |
| `#` | Black cell without clue |
| `a`–`i` | Black cell with clue digit (a=1, …, i=9) |

---

## Repository Structure

```
str8ts-web/
├── index.html          # Vite entry point
├── src/
│   ├── game.js         # Vanilla JS frontend
│   └── style.css       # Styles
├── public/
│   └── pkg/            # Pre-built WASM package (committed)
│       ├── str8ts.js
│       ├── str8ts.d.ts
│       ├── str8ts_bg.wasm
│       └── str8ts_bg.wasm.d.ts
├── str8ts-solver/      # Rust source (compiled to WASM via wasm-pack)
│   ├── Cargo.toml
│   └── src/
│       ├── generator.rs
│       ├── human.rs
│       ├── sat.rs
│       └── ...
├── vite.config.js
└── package.json
```

---

## Local Development

### Prerequisites

- [Node.js](https://nodejs.org/) (v18 or higher)
- [npm](https://www.npmjs.com/)

### Installation

Clone the repository:

```bash
git clone https://github.com/PhoenixSmaug/str8ts-web.git
cd str8ts-web
```

Install dependencies:

```bash
npm install
```

### Running Locally

Start the development server:

```bash
npm run dev
```

Open [http://localhost:5173](http://localhost:5173) in your browser.

---

## Rebuilding the WASM

If you modify the Rust solver in `str8ts-solver/`, rebuild the WASM package with [wasm-pack](https://rustwasm.github.io/wasm-pack/):

```bash
cd str8ts-solver
wasm-pack build --target web --out-dir ../public/pkg
```

The updated files in `public/pkg/` should then be committed alongside your Rust changes.

---

## Deployment to GitHub Pages

Build and deploy to the `gh-pages` branch in one step:

```bash
npm run deploy
```

This runs `vite build` (with `/str8ts-web/` as the base path) and then publishes `dist/` via [gh-pages](https://www.npmjs.com/package/gh-pages).

(c) Mia Müßig
