// Game State Variables
let selectedCell = null;
let notesMode = false;
let puzzle = new Array(9).fill(null).map(() => new Array(9).fill(null));  // 9x9 array holding cell entries
let notes = new Array(9).fill(null).map(() => 
    new Array(9).fill(null).map(() => new Set())  // 9x9 array holding a set of noted candidate for each cell
);
let currentSolution = '';
let moveHistory = [];
let hasCelebratedCurrentPuzzle = false;
let hintApiModule = null;
let pendingHint = null;
let currentMode = 'puzzle';
let solverValues = new Array(9).fill(null).map(() => new Array(9).fill(null));
let solverNotes = new Array(9).fill(null).map(() => new Array(9).fill(null).map(() => new Set()));
let solverStateInitialized = false;
let solverHintUsed = false;  // true after the first Hint press; suppresses red trivial-candidate display
let solverBasePuzzle = null; // snapshot of puzzle at last solver-state init
let solverBaseNotes = null;  // snapshot of notes at last solver-state init
let creatorValidationRequestId = 0;
let creatorValidatedBoardString = '';
let creatorValidatedSolution = '';

const NOTES_ICON_PENCIL = `
<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    <path d="M15.232 5.232l3.536 3.536m-2.036-5.036a2.5 2.5 0 113.536 3.536L6.5 21.036H3v-3.572L16.732 3.732z"/>
</svg>`;

const NOTES_ICON_BLACK_TILE = `
<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
    <rect x="4" y="4" width="16" height="16" rx="2" ry="2"></rect>
</svg>`;

function createEmptyNotesGrid() {
    return new Array(9).fill(null).map(() => new Array(9).fill(null).map(() => new Set()));
}

function createEmptyValuesGrid() {
    return new Array(9).fill(null).map(() => new Array(9).fill(null));
}

function cloneValuesGrid(values) {
    return values.map(row => row.slice());
}

function cloneNotesGrid(noteGrid) {
    return noteGrid.map(row => row.map(set => new Set(set)));
}

function serializeValuesGrid(values) {
    return values.map(row => row.map(v => (v === null || v === undefined ? null : v)));
}

function serializeNotesGrid(noteGrid) {
    return noteGrid.map(row => row.map(set => Array.from(set).sort((a, b) => a - b)));
}

function deserializeValuesGrid(raw) {
    if (!Array.isArray(raw) || raw.length !== 9) {
        return null;
    }
    const values = createEmptyValuesGrid();
    for (let r = 0; r < 9; r++) {
        if (!Array.isArray(raw[r]) || raw[r].length !== 9) {
            return null;
        }
        for (let c = 0; c < 9; c++) {
            const v = raw[r][c];
            if (v === null || v === undefined) {
                values[r][c] = null;
            } else if (Number.isInteger(v) && v >= 1 && v <= 9) {
                values[r][c] = v;
            } else {
                return null;
            }
        }
    }
    return values;
}

function deserializeNotesGrid(raw) {
    if (!Array.isArray(raw) || raw.length !== 9) {
        return null;
    }
    const out = createEmptyNotesGrid();
    for (let r = 0; r < 9; r++) {
        if (!Array.isArray(raw[r]) || raw[r].length !== 9) {
            return null;
        }
        for (let c = 0; c < 9; c++) {
            const arr = raw[r][c];
            if (!Array.isArray(arr)) {
                return null;
            }
            const set = new Set();
            for (const d of arr) {
                if (!Number.isInteger(d) || d < 1 || d > 9) {
                    return null;
                }
                set.add(d);
            }
            out[r][c] = set;
        }
    }
    return out;
}

function areSetsEqual(a, b) {
    if (a.size !== b.size) {
        return false;
    }
    for (const v of a) {
        if (!b.has(v)) {
            return false;
        }
    }
    return true;
}

// Initialization Functions
window.onload = async function() {
    createGrid();
    document.addEventListener('keydown', handleKeyPress);
    const creatorBoardInput = document.getElementById('creatorBoardInput');
    if (creatorBoardInput) {
        creatorBoardInput.addEventListener('blur', handleCreatorInputBlur);
    }
    updateModeControls();
    // Try to load saved state first
    const savedState = loadPuzzleState();

    // If no saved state, initialize first puzzle
    if (!savedState) {
        await initializeFirstPuzzle();
    }
}

async function initializeFirstPuzzle() {
    const difficulty = document.getElementById('difficulty').value;
    showLoadingOverlay('Generating puzzle…');
    try {
        const puzzleData = await generatePuzzleWasm(difficulty);
        if (puzzleData) {
            loadPuzzle(puzzleData.puzzle, puzzleData.solution);
        } else {
            console.error('Failed to generate initial puzzle');
        }
    } finally {
        hideLoadingOverlay();
    }
}

function createGrid() {
    const grid = document.getElementById('puzzle-grid');
    grid.innerHTML = '';

    for (let i = 0; i < 9; i++) {
        for (let j = 0; j < 9; j++) {
            const cell = document.createElement('div');
            cell.className = 'cell';
            cell.dataset.row = i;
            cell.dataset.col = j;
            
            cell.addEventListener('click', () => selectCell(cell));
            
            const notesContainer = document.createElement('div');
            notesContainer.className = 'notes';
            for (let k = 0; k < 9; k++) {
                notesContainer.appendChild(document.createElement('span'));
            }
            cell.appendChild(notesContainer);
            
            grid.appendChild(cell);
        }
    }
}

// Puzzle Loading and Management

// Maps UI difficulty values to WASM generator arguments.
function wasmGeneratorArgs(difficulty) {
    const symCheck = document.getElementById('symmetricCheck');
    const symmetric = symCheck ? symCheck.checked : true;
    switch (difficulty) {
        case 'easy':     return { difficultyStr: 'easy',     symmetric };
        case 'medium':   return { difficultyStr: 'medium',   symmetric };
        case 'hard':     return { difficultyStr: 'hard',     symmetric };
        case 'diabolic': return { difficultyStr: 'diabolic', symmetric };
        case 'cruel':    return { difficultyStr: 'cruel',    symmetric };
        case 'extreme':  return { difficultyStr: 'extreme',  symmetric };
        default: return null;
    }
}

function showLoadingOverlay(message) {
    const overlay = document.getElementById('loadingOverlay');
    const msg = document.getElementById('loadingMessage');
    if (msg) msg.textContent = message || 'Generating puzzle…';
    if (overlay) overlay.style.display = 'flex';
}

function hideLoadingOverlay() {
    const overlay = document.getElementById('loadingOverlay');
    if (overlay) overlay.style.display = 'none';
}

async function generatePuzzleWasm(difficulty) {
    const args = wasmGeneratorArgs(difficulty);
    if (!args) return null;
    try {
        const api = await loadHintApi();
        if (typeof api.generate_puzzle_wasm !== 'function') {
            console.error('generate_puzzle_wasm not found in WASM module — rebuild with wasm-pack');
            return null;
        }
        // Generation is synchronous and CPU-heavy; yield to the browser first so the
        // loading overlay can render, then run the computation.
        await new Promise(resolve => setTimeout(resolve, 30));
        const responseText = api.generate_puzzle_wasm(args.difficultyStr, args.symmetric);
        const response = JSON.parse(responseText);
        if (!response.ok) {
            console.warn('WASM generator failed:', response.error);
            return null;
        }
        return { puzzle: response.puzzle, solution: response.solution };
    } catch (error) {
        console.error('WASM generator error:', error);
        return null;
    }
}

async function loadPuzzle(puzzleStr, solutionStr) {
    const grid = document.getElementById('puzzle-grid');
    const cells = grid.children;
    let idx = 0;

    puzzleStr = puzzleStr.replace(/\s/g, '');
    currentSolution = solutionStr;

    for (let i = 0; i < puzzleStr.length; i++) {
        const char = puzzleStr[i];
        const cell = cells[idx];
        
        if (char === '#') {
            cell.classList.add('black');
        } else if (char >= '1' && char <= '9') {
            cell.textContent = char;
            cell.classList.add('hint');
            puzzle[Math.floor(idx/9)][idx%9] = parseInt(char);
        } else if (char >= 'a' && char <= 'i') {
            cell.classList.add('black');
            cell.textContent = (char.charCodeAt(0) - 96).toString();
        }
        idx++;
    }

    solverValues = cloneValuesGrid(puzzle);
    solverNotes = createEmptyNotesGrid();
    solverStateInitialized = false;
    pendingHint = null;
    clearHintVisuals();
    setHintDescription('');
    currentMode = 'puzzle';
    updateModeControls();
    renderBoardForCurrentMode();

    // Save the initial puzzle state
    savePuzzleState();
}

async function newPuzzle() {
    showConfirmDialog('Start a new puzzle? This will clear your current progress.', async () => {
        // Clear saved state
        localStorage.removeItem('str8tsPuzzleState');

        moveHistory = [];
        const difficulty = document.getElementById('difficulty').value;

        showLoadingOverlay('Generating puzzle…');
        try {
            const puzzleData = await generatePuzzleWasm(difficulty);
            if (puzzleData) {
                clearCurrentPuzzle();
                loadPuzzle(puzzleData.puzzle, puzzleData.solution);
            } else {
                console.error('Failed to generate puzzle');
            }
        } finally {
            hideLoadingOverlay();
        }
    });
}

// Cell Selection and Input Handling
function selectCell(cell) {
    if (currentMode === 'creator') {
        handleCreatorCellClick(cell);
        return;
    }

    if (currentMode === 'solver') {
        return;
    }
    if (cell.classList.contains('hint') || cell.classList.contains('black')) {
        return;
    }

    if (selectedCell) {
        selectedCell.classList.remove('selected');
        selectedCell.classList.remove('selected-strong');
    }
    cell.classList.add('selected');
    selectedCell = cell;
    refreshSelectedCellStyle();
}

function refreshSelectedCellStyle() {
    if (!selectedCell) {
        return;
    }
    selectedCell.classList.toggle('selected-strong', !notesMode || currentMode === 'creator');
}

function resetPrimaryActionButtonVisual() {
    const actionBtn = document.getElementById('primaryActionButton');
    if (!actionBtn) {
        return;
    }
    actionBtn.style.background = '#2196f3';
}

// Move selection to the next non-black cell in the given direction, skipping over black tiles.
// Does NOT trigger cell-click actions (safe to call in any mode).
function navigateSelection(dr, dc) {
    if (!selectedCell) return false;
    let row = parseInt(selectedCell.dataset.row, 10);
    let col = parseInt(selectedCell.dataset.col, 10);
    while (true) {
        row += dr;
        col += dc;
        if (row < 0 || row >= 9 || col < 0 || col >= 9) return false;
        const target = getCellAt(row, col);
        if (currentMode === 'creator' || !target.classList.contains('black')) {
            selectedCell.classList.remove('selected', 'selected-strong');
            target.classList.add('selected');
            selectedCell = target;
            refreshSelectedCellStyle();
            return true;
        }
    }
}

function handleKeyPress(e) {
    // Arrow-key navigation (puzzle & creator modes; solver has no selection)
    if (selectedCell && ['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].includes(e.key)
            && currentMode !== 'solver') {
        const dr = e.key === 'ArrowUp' ? -1 : e.key === 'ArrowDown' ? 1 : 0;
        const dc = e.key === 'ArrowLeft' ? -1 : e.key === 'ArrowRight' ? 1 : 0;
        navigateSelection(dr, dc);
        e.preventDefault();
        return;
    }

    if (currentMode === 'creator') {
        if (!selectedCell && e.key !== 'n') return;

        if (e.key === 'n') {
            toggleNotes();
            return;
        }

        const row = parseInt(selectedCell.dataset.row, 10);
        const col = parseInt(selectedCell.dataset.col, 10);

        if (e.key >= '1' && e.key <= '9') {
            handleCreatorNumberInput(row, col, parseInt(e.key, 10));
        } else if (e.key === 'Backspace' || e.key === 'Delete') {
            handleCreatorClearCell(row, col);
        }
        return;
    }

    if (currentMode === 'solver') {
        if (e.key === 'n' || (e.key >= '1' && e.key <= '9') || e.key === 'Backspace' || e.key === 'Delete') {
            cancelPendingHintPreview();
        }
        return;
    }

    if (e.key === 'n' || (selectedCell && ((e.key >= '1' && e.key <= '9') || e.key === 'Backspace' || e.key === 'Delete'))) {
        cancelPendingHintPreview();
    }

    if (!selectedCell && e.key !== 'n') return;
    
    if (e.key === 'n') {
        toggleNotes();
        return;
    }
    
    const row = parseInt(selectedCell.dataset.row);
    const col = parseInt(selectedCell.dataset.col);

    const previousValue = puzzle[row][col];
    const previousNotes = new Set(notes[row][col]);

    if (e.key >= '1' && e.key <= '9') {
        // disallow notes for already filled tiles
        if (notesMode && previousValue !== null) {
            return;
        }

        handleNumberInput(row, col, parseInt(e.key));
    } else if (e.key === 'Backspace' || e.key === 'Delete') {
        handleClearCell(row, col);
    }

    saveMove(selectedCell, previousValue, previousNotes);
    savePuzzleState();
    checkPuzzleComplete();
}

function handleNumpadInput(value) {
    if (currentMode === 'creator') {
        if (!selectedCell) return;
        if (value === 'clear') {
            handleKeyPress({ key: 'Backspace' });
        } else {
            handleKeyPress({ key: value.toString() });
        }
        return;
    }

    if (currentMode === 'solver') {
        cancelPendingHintPreview();
        return;
    }
    cancelPendingHintPreview();
    if (!selectedCell) return;
    
    if (value === 'clear') {
        handleKeyPress({ key: 'Backspace' });
    } else {
        handleKeyPress({ key: value.toString() });
    }
}

// Notes Management
function toggleNotes() {
    if (currentMode === 'solver') {
        return;
    }
    cancelPendingHintPreview();
    notesMode = !notesMode;
    const notesButton = document.getElementById('notesButton');
    if (notesButton) {
        notesButton.classList.toggle('notes-active', notesMode);
    }
    updateNotesButtonIcon();
    refreshSelectedCellStyle();
}

function toggleNote(row, col, num) {
    if (notes[row][col].has(num)) {
        notes[row][col].delete(num);
    } else {
        notes[row][col].add(num);
    }
}

function updateNotes(cell, noteSet, conflictDigits = []) {
    let notesContainer = cell.querySelector('.notes');
    if (!notesContainer) {
        notesContainer = createNotesContainer();
        cell.appendChild(notesContainer);
    }
    
    const spans = notesContainer.children;
    Array.from(spans).forEach(span => {
        span.textContent = '';
        span.classList.remove('hint-removed');
    });
    noteSet.forEach(num => {
        spans[num - 1].textContent = num;
    });

    conflictDigits.forEach(num => {
        if (num < 1 || num > 9) return;
        spans[num - 1].textContent = num;
        spans[num - 1].classList.add('hint-removed');
    });
}

// Move History Management
function saveMove(cell, previousValue, previousNotes) {
    moveHistory.push({
        row: cell.dataset.row,
        col: cell.dataset.col,
        value: previousValue,
        notes: new Set(previousNotes)
    });
}

function saveBoardSnapshotMove() {
    moveHistory.push({
        kind: 'snapshot',
        puzzle: cloneValuesGrid(puzzle),
        notes: cloneNotesGrid(notes),
    });
}

function createCreatorSnapshotMove() {
    return {
        kind: 'creator_snapshot',
        board: buildBoardStringFromGrid(),
        solverValues: cloneValuesGrid(solverValues),
        solverNotes: cloneNotesGrid(solverNotes),
        solverStateInitialized,
    };
}

function pushCreatorSnapshotIfChanged(snapshot) {
    if (!snapshot || snapshot.kind !== 'creator_snapshot') {
        return;
    }
    if (snapshot.board !== buildBoardStringFromGrid()) {
        moveHistory.push(snapshot);
    }
}

function undoLastMove() {
    if (currentMode === 'solver') {
        return;
    }
    cancelPendingHintPreview();
    if (moveHistory.length === 0) return;
    
    const lastMove = moveHistory.pop();

    if (lastMove.kind === 'snapshot') {
        puzzle = cloneValuesGrid(lastMove.puzzle);
        notes = cloneNotesGrid(lastMove.notes);
        hasCelebratedCurrentPuzzle = false;
        renderBoardForCurrentMode();
        savePuzzleState();
        return;
    }

    if (lastMove.kind === 'creator_snapshot') {
        applyBoardStringToGrid(lastMove.board);
        if (lastMove.solverValues) {
            solverValues = cloneValuesGrid(lastMove.solverValues);
        }
        if (lastMove.solverNotes) {
            solverNotes = cloneNotesGrid(lastMove.solverNotes);
        }
        solverStateInitialized = !!lastMove.solverStateInitialized;
        hasCelebratedCurrentPuzzle = false;

        if (selectedCell) {
            selectedCell.classList.remove('selected');
            selectedCell.classList.remove('selected-strong');
            selectedCell = null;
        }

        renderBoardForCurrentMode();
        if (currentMode === 'creator') {
            syncCreatorPanelFromBoard();
            validateCreatorBoardString(buildBoardStringFromGrid());
        }
        savePuzzleState();
        return;
    }

    const cell = document.querySelector(
        `.cell[data-row="${lastMove.row}"][data-col="${lastMove.col}"]`
    );
    
    const row = parseInt(lastMove.row);
    const col = parseInt(lastMove.col);
    
    puzzle[row][col] = lastMove.value;
    notes[row][col] = new Set(lastMove.notes);
    hasCelebratedCurrentPuzzle = false;
    
    updateCellDisplay(cell, lastMove.value, lastMove.notes);
    savePuzzleState();
}

// Helper Functions
function handleNumberInput(row, col, number) {
    selectedCell.classList.remove('wrong-number');
    hasCelebratedCurrentPuzzle = false;
    if (notesMode) {
        toggleNote(row, col, number);
        updateNotes(selectedCell, notes[row][col]);
    } else {
        puzzle[row][col] = number;
        selectedCell.textContent = number;
        notes[row][col].clear();
        updateNotes(selectedCell, notes[row][col]);
    }
}

function handleClearCell(row, col) {
    // Clear both the number and notes
    hasCelebratedCurrentPuzzle = false;
    puzzle[row][col] = null;
    selectedCell.textContent = '';
    notes[row][col].clear();
    ensureNotesContainer(selectedCell);
    updateNotes(selectedCell, new Set());
}

function updateCellDisplay(cell, value, noteSet) {
    cell.classList.remove('wrong-number');

    if (value === null) {
        cell.textContent = '';
    } else {
        cell.textContent = value;cell.classList.remove('wrong-number');
    }
    updateNotes(cell, noteSet);
}

function createNotesContainer() {
    const container = document.createElement('div');
    container.className = 'notes';
    for (let k = 0; k < 9; k++) {
        container.appendChild(document.createElement('span'));
    }
    return container;
}

function ensureNotesContainer(cell) {
    if (!cell.querySelector('.notes')) {
        cell.appendChild(createNotesContainer());
    }
}

function updateNotesButtonIcon() {
    const notesButton = document.getElementById('notesButton');
    if (!notesButton) {
        return;
    }
    notesButton.innerHTML = currentMode === 'creator' ? NOTES_ICON_BLACK_TILE : NOTES_ICON_PENCIL;
}

function normalizeBoardString(text) {
    return (text || '').replace(/\s/g, '');
}

function isValidBoardStringSyntax(boardStr) {
    return boardStr.length === 81 && /^[#.a-i1-9]+$/.test(boardStr);
}

function setCreatorValidation(message, kind = '') {
    const el = document.getElementById('creatorValidation');
    if (!el) {
        return;
    }
    el.textContent = message || '';
    el.classList.remove('valid', 'warn', 'error');
    if (kind) {
        el.classList.add(kind);
    }
}

function buildBoardStringFromGrid() {
    const chars = [];
    const grid = document.getElementById('puzzle-grid');
    Array.from(grid.children).forEach(cell => {
        const row = parseInt(cell.dataset.row, 10);
        const col = parseInt(cell.dataset.col, 10);
        if (cell.classList.contains('black')) {
            const text = cell.textContent.trim();
            if (/^[1-9]$/.test(text)) {
                chars.push(String.fromCharCode('a'.charCodeAt(0) + parseInt(text, 10) - 1));
            } else {
                chars.push('#');
            }
        } else {
            if (cell.classList.contains('hint') && Number.isInteger(puzzle[row][col])) {
                chars.push(puzzle[row][col].toString());
            } else {
                chars.push('.');
            }
        }
    });
    return chars.join('');
}

function applyBoardStringToGrid(boardStr) {
    const normalized = normalizeBoardString(boardStr);
    if (!isValidBoardStringSyntax(normalized)) {
        return false;
    }

    const grid = document.getElementById('puzzle-grid');
    puzzle = createEmptyValuesGrid();
    notes = createEmptyNotesGrid();

    Array.from(grid.children).forEach((cell, idx) => {
        const row = Math.floor(idx / 9);
        const col = idx % 9;
        const ch = normalized[idx];

        cell.className = 'cell';
        cell.textContent = '';
        ensureNotesContainer(cell);
        updateNotes(cell, new Set());

        if (ch === '#') {
            cell.classList.add('black');
            return;
        }
        if (ch >= 'a' && ch <= 'i') {
            cell.classList.add('black');
            cell.textContent = (ch.charCodeAt(0) - 96).toString();
            return;
        }
        if (ch >= '1' && ch <= '9') {
            cell.classList.add('hint');
            cell.textContent = ch;
            puzzle[row][col] = parseInt(ch, 10);
        }
    });

    solverValues = cloneValuesGrid(puzzle);
    solverNotes = createEmptyNotesGrid();
    solverStateInitialized = false;
    currentSolution = '';
    pendingHint = null;
    clearHintVisuals();
    return true;
}

function syncCreatorPanelFromBoard() {
    const input = document.getElementById('creatorBoardInput');
    if (!input) {
        return;
    }
    input.value = buildBoardStringFromGrid();
}

async function validateCreatorBoardString(boardStr) {
    const requestId = ++creatorValidationRequestId;

    if (boardStr.length < 81) {
        creatorValidatedBoardString = '';
        creatorValidatedSolution = '';
        setCreatorValidation(`Editing board string (${boardStr.length}/81)`, '');
        return false;
    }

    if (!isValidBoardStringSyntax(boardStr)) {
        creatorValidatedBoardString = '';
        creatorValidatedSolution = '';
        setCreatorValidation('Board string must be 81 chars with . # a-i 1-9 only.', 'error');
        return false;
    }

    try {
        const api = await loadHintApi();
        const responseText = api.creator_validate_board(boardStr);
        const response = JSON.parse(responseText);

        if (requestId !== creatorValidationRequestId) {
            return false;
        }

        if (!response.ok) {
            creatorValidatedBoardString = '';
            creatorValidatedSolution = '';
            setCreatorValidation(response.message || response.error || 'Validation failed', 'error');
            return false;
        }

        creatorValidatedBoardString = boardStr;
        creatorValidatedSolution = typeof response.solution === 'string' && response.solution.length === 81
            ? response.solution
            : '';

        if (!response.valid) {
            setCreatorValidation(response.message || 'Invalid puzzle (no solution)', 'error');
        } else if (!response.unique) {
            setCreatorValidation(response.message || 'Valid but non-unique', 'warn');
        } else {
            setCreatorValidation(response.message || 'Valid and unique', 'valid');
        }
        return true;
    } catch (error) {
        console.error('Error validating creator board:', error);
        creatorValidatedBoardString = '';
        creatorValidatedSolution = '';
        setCreatorValidation('Validation API error', 'error');
        return false;
    }
}

async function handleCreatorInputChanged() {
    if (currentMode !== 'creator') {
        return;
    }
    setCreatorValidation('');
}

async function handleCreatorInputBlur() {
    if (currentMode !== 'creator') {
        return;
    }
    const input = document.getElementById('creatorBoardInput');
    if (!input) {
        return;
    }

    const normalized = normalizeBoardString(input.value);
    input.value = normalized;

    if (!isValidBoardStringSyntax(normalized)) {
        setCreatorValidation('Board string must be 81 chars with . # a-i 1-9 only.', 'error');
        return;
    }

    const creatorSnapshot = createCreatorSnapshotMove();
    if (!applyBoardStringToGrid(normalized)) {
        setCreatorValidation('Failed to apply board string.', 'error');
        return;
    }

    pushCreatorSnapshotIfChanged(creatorSnapshot);
    renderBoardForCurrentMode();
    syncCreatorPanelFromBoard();
    await validateCreatorBoardString(normalized);
    savePuzzleState();
}

function finalizeCreatorEditState() {
    hasCelebratedCurrentPuzzle = false;
    pendingHint = null;
    setHintDescription('');
    clearHintVisuals();
    syncCreatorPanelFromBoard();
    validateCreatorBoardString(buildBoardStringFromGrid());
    savePuzzleState();
}

function handleCreatorCellClick(cell) {
    const creatorSnapshot = createCreatorSnapshotMove();

    if (selectedCell) {
        selectedCell.classList.remove('selected');
        selectedCell.classList.remove('selected-strong');
    }

    const row = parseInt(cell.dataset.row, 10);
    const col = parseInt(cell.dataset.col, 10);

    if (notesMode) {
        if (!cell.classList.contains('black')) {
            // White/hint cell → convert to black tile
            const digitText = cell.classList.contains('hint') && Number.isInteger(puzzle[row][col])
                ? puzzle[row][col].toString()
                : '';
            cell.classList.remove('hint');
            cell.classList.add('black');
            cell.textContent = digitText;
            puzzle[row][col] = null;
            notes[row][col].clear();
            updateNotes(cell, new Set());
            selectedCell = null;
            pushCreatorSnapshotIfChanged(creatorSnapshot);
            finalizeCreatorEditState();
        } else {
            // Black tile → convert back to white, preserving any digit as a white hint
            const blackDigitText = cell.textContent.trim();
            const blackDigit = (blackDigitText.length === 1 && blackDigitText >= '1' && blackDigitText <= '9')
                ? parseInt(blackDigitText, 10) : null;
            cell.classList.remove('black');
            cell.textContent = blackDigit !== null ? blackDigit.toString() : '';
            if (blackDigit !== null) {
                cell.classList.add('hint');
                puzzle[row][col] = blackDigit;
            } else {
                puzzle[row][col] = null;
            }
            notes[row][col].clear();
            ensureNotesContainer(cell);
            updateNotes(cell, new Set());
            selectedCell = null;
            pushCreatorSnapshotIfChanged(creatorSnapshot);
            finalizeCreatorEditState();
        }
        return;
    }

    cell.classList.add('selected');
    selectedCell = cell;
    refreshSelectedCellStyle();
}

function handleCreatorNumberInput(row, col, number) {
    const creatorSnapshot = createCreatorSnapshotMove();
    const cell = getCellAt(row, col);
    if (notesMode) {
        cell.classList.remove('hint');
        cell.classList.add('black');
        cell.textContent = number.toString();
        puzzle[row][col] = null;
        notes[row][col].clear();
        updateNotes(cell, new Set());
        selectedCell = null;
    } else {
        if (cell.classList.contains('black')) {
            cell.textContent = number.toString();
            puzzle[row][col] = null;
        } else {
            cell.classList.remove('black');
            cell.classList.add('hint');
            cell.textContent = number.toString();
            puzzle[row][col] = number;
        }
        notes[row][col].clear();
        updateNotes(cell, new Set());
    }
    pushCreatorSnapshotIfChanged(creatorSnapshot);
    finalizeCreatorEditState();
}

function handleCreatorClearCell(row, col) {
    const creatorSnapshot = createCreatorSnapshotMove();
    const cell = getCellAt(row, col);
    cell.classList.remove('black');
    cell.classList.remove('hint');
    cell.textContent = '';
    puzzle[row][col] = null;
    notes[row][col].clear();
    updateNotes(cell, new Set());
    pushCreatorSnapshotIfChanged(creatorSnapshot);
    finalizeCreatorEditState();
}

// Board Management
function clearCurrentPuzzle() {
    cancelPendingHintPreview();
    const grid = document.getElementById('puzzle-grid');
    Array.from(grid.children).forEach(cell => {
        // Remove all class names except 'cell'
        cell.className = 'cell';
        // Clear text content
        cell.textContent = '';
        // Reset data attributes
        cell.dataset.row = cell.dataset.row;
        cell.dataset.col = cell.dataset.col;
        // Clear notes
        const row = parseInt(cell.dataset.row);
        const col = parseInt(cell.dataset.col);
        puzzle[row][col] = null;
        notes[row][col] = new Set();
        
        // Ensure notes container exists and is empty
        let notesContainer = cell.querySelector('.notes');
        if (notesContainer) {
            cell.removeChild(notesContainer);
        }
        notesContainer = createNotesContainer();
        cell.appendChild(notesContainer);
    });

    // Reset game state
    selectedCell = null;
    notesMode = false;
    hasCelebratedCurrentPuzzle = false;
    puzzle = new Array(9).fill(null).map(() => new Array(9).fill(null));
    notes = new Array(9).fill(null).map(() => 
        new Array(9).fill(null).map(() => new Set())
    );
    solverValues = createEmptyValuesGrid();
    solverNotes = createEmptyNotesGrid();
    solverStateInitialized = false;
    currentMode = 'puzzle';
    updateModeControls();
    currentSolution = '';
}

function clearBoard() {
    cancelPendingHintPreview();

    if (currentMode === 'creator') {
        showConfirmDialog('Empty the whole board? This removes hints and black tiles too.', () => {
            localStorage.removeItem('str8tsPuzzleState');
            moveHistory = [];
            const grid = document.getElementById('puzzle-grid');
            Array.from(grid.children).forEach(cell => {
                const row = parseInt(cell.dataset.row, 10);
                const col = parseInt(cell.dataset.col, 10);

                cell.className = 'cell';
                cell.textContent = '';
                puzzle[row][col] = null;
                notes[row][col].clear();
                solverValues[row][col] = null;
                solverNotes[row][col].clear();

                let notesContainer = cell.querySelector('.notes');
                if (notesContainer) notesContainer.remove();
                cell.appendChild(createNotesContainer());
            });

            if (selectedCell) {
                selectedCell.classList.remove('selected', 'selected-strong');
                selectedCell = null;
            }

            notesMode = false;
            hasCelebratedCurrentPuzzle = false;
            solverStateInitialized = false;
            currentSolution = '';
            setHintDescription('');
            setCreatorValidation('');
            updateModeControls();
            renderBoardForCurrentMode();
            syncCreatorPanelFromBoard();
            validateCreatorBoardString(buildBoardStringFromGrid());
            savePuzzleState();
        });
        return;
    }

    // Puzzle mode / Solver mode: restore original puzzle (clear user input and notes only)
    showConfirmDialog('Reset the puzzle? This clears all your entries and notes.', () => {
        moveHistory = [];
        const grid = document.getElementById('puzzle-grid');
        Array.from(grid.children).forEach(cell => {
            const row = parseInt(cell.dataset.row, 10);
            const col = parseInt(cell.dataset.col, 10);

            if (cell.classList.contains('black') || cell.classList.contains('hint')) {
                return; // leave black tiles and given hints untouched
            }

            cell.textContent = '';
            puzzle[row][col] = null;
            notes[row][col].clear();
            solverValues[row][col] = null;
            solverNotes[row][col].clear();

            let notesContainer = cell.querySelector('.notes');
            if (notesContainer) notesContainer.remove();
            cell.appendChild(createNotesContainer());
        });

        if (selectedCell) {
            selectedCell.classList.remove('selected', 'selected-strong');
            selectedCell = null;
        }

        notesMode = false;
        hasCelebratedCurrentPuzzle = false;
        solverStateInitialized = false;
        setHintDescription('');
        updateModeControls();
        renderBoardForCurrentMode();
        clearSolverValidation();
        if (currentMode === 'solver') {
            initializeSolverStateFromPuzzle();
            syncSolverCandidates();
            refreshSolverModeValidation();
        } else {
            checkPuzzleComplete();
        }
        savePuzzleState();
    });
}

// Solution Checking
function checkSolution(options = {}) {
    const { showSuccessFeedback = true } = options;
    const grid = document.getElementById('puzzle-grid');
    const checkButton = document.getElementById('primaryActionButton');
    let allCorrect = true;

    if (!currentSolution || currentSolution.length !== 81) {
        provideFeedback(checkButton, false);
        return false;
    }

    const cells = Array.from(grid.children);
    cells.forEach(cell => {
        cell.classList.remove('error');
        cell.classList.remove('wrong-number');
    });

    for (let row = 0; row < 9; row++) {
        for (let col = 0; col < 9; col++) {
            const idx = row * 9 + col;
            const cell = grid.children[idx];
            if (cell.classList.contains('black') || cell.classList.contains('hint')) {
                continue;
            }

            const solutionValue = parseInt(currentSolution[idx], 10);
            const userValue = puzzle[row][col];
            const candidateSet = notes[row][col] || new Set();

            if (userValue !== null && userValue !== solutionValue) {
                cell.classList.add('error');
                allCorrect = false;
                continue;
            }

            if (candidateSet.size > 0 && !candidateSet.has(solutionValue)) {
                cell.classList.add('error');
                allCorrect = false;
            }
        }
    }
    if (!allCorrect) {
        provideFeedback(checkButton, false);
    } else if (showSuccessFeedback) {
        provideFeedback(checkButton, true);
    }
    return allCorrect;
}

function boardHasUserErrors(markErrors = false) {
    const grid = document.getElementById('puzzle-grid');
    let allCorrect = true;

    if (!currentSolution || currentSolution.length !== 81) {
        if (markErrors) {
            Array.from(grid.children).forEach(cell => cell.classList.remove('error'));
        }
        return false;
    }

    if (markErrors) {
        Array.from(grid.children).forEach(cell => cell.classList.remove('error'));
    }

    for (let row = 0; row < 9; row++) {
        for (let col = 0; col < 9; col++) {
            const idx = row * 9 + col;
            const cell = grid.children[idx];
            if (cell.classList.contains('black') || cell.classList.contains('hint')) {
                continue;
            }

            const solutionValue = parseInt(currentSolution[idx], 10);
            const userValue = puzzle[row][col];
            const candidateSet = notes[row][col] || new Set();

            if ((userValue !== null && userValue !== solutionValue) || (candidateSet.size > 0 && !candidateSet.has(solutionValue))) {
                allCorrect = false;
                if (markErrors) {
                    cell.classList.add('error');
                }
            }
        }
    }

    return !allCorrect;
}

function updateModeControls() {
    const puzzleBtn = document.getElementById('puzzleModeBtn');
    const solverBtn = document.getElementById('solverModeBtn');
    const creatorBtn = document.getElementById('creatorModeBtn');
    const actionBtn = document.getElementById('primaryActionButton');
    const hintDescription = document.getElementById('hintDescription');
    const creatorPanel = document.getElementById('creatorPanel');
    const clearButton = document.getElementById('clearButton');
    const notesButton = document.getElementById('notesButton');

    if (puzzleBtn) puzzleBtn.classList.toggle('active', currentMode === 'puzzle');
    if (solverBtn) solverBtn.classList.toggle('active', currentMode === 'solver');
    if (creatorBtn) creatorBtn.classList.toggle('active', currentMode === 'creator');

    if (actionBtn) {
        if (currentMode === 'solver') {
            actionBtn.textContent = pendingHint ? 'Apply' : 'Hint';
            actionBtn.disabled = false;
        } else if (currentMode === 'creator') {
            actionBtn.textContent = 'Check';
            actionBtn.disabled = true;
            actionBtn.classList.remove('solver-action-width');
        } else {
            actionBtn.textContent = 'Check';
            actionBtn.disabled = false;
            actionBtn.classList.remove('solver-action-width');
        }
    }

    if (hintDescription) {
        hintDescription.style.display = currentMode === 'creator' ? 'none' : '';
    }
    if (creatorPanel) {
        creatorPanel.style.display = currentMode === 'creator' ? 'block' : 'none';
    }
    if (clearButton) {
        clearButton.textContent = currentMode === 'creator' ? 'Empty' : 'Reset';
    }

    if (notesButton) {
        notesButton.classList.toggle('notes-active', notesMode && currentMode !== 'solver');
        notesButton.title = currentMode === 'creator' ? 'Black Tile Mode' : 'Notes Mode';
    }

    updateNotesButtonIcon();
    resetPrimaryActionButtonVisual();
}

function setMode(mode) {
    if (mode === currentMode) {
        return;
    }

    if (currentMode === 'creator' && mode !== 'creator') {
        const creatorInput = document.getElementById('creatorBoardInput');
        const candidate = normalizeBoardString(creatorInput ? creatorInput.value : '');
        if (!isValidBoardStringSyntax(candidate)) {
            setCreatorValidation('Cannot leave Creator Mode: board string is invalid.', 'error');
            return;
        }
        const creatorSnapshot = createCreatorSnapshotMove();
        if (!applyBoardStringToGrid(candidate)) {
            setCreatorValidation('Cannot leave Creator Mode: failed to apply board string.', 'error');
            return;
        }
        if (creatorValidatedBoardString === candidate && creatorValidatedSolution.length === 81) {
            currentSolution = creatorValidatedSolution;
        }
        pushCreatorSnapshotIfChanged(creatorSnapshot);
    }

    if (mode === 'solver') {
        cancelPendingHintPreview();
        if (selectedCell) {
            selectedCell.classList.remove('selected');
            selectedCell.classList.remove('selected-strong');
            selectedCell = null;
        }
        if (puzzleValuesChangedSinceSolverInit()) {
            // Puzzle values changed: full re-initialization.
            initializeSolverStateFromPuzzle();
        } else if (notesChangedSinceSolverInit()) {
            // Only notes changed: remove user-deleted candidates from solver state.
            updateSolverStateFromNoteChanges();
        }
        // Otherwise solver state is unchanged; keep it as-is.
        currentMode = 'solver';
        solverHintUsed = false;
        syncSolverCandidates();
    } else if (mode === 'creator') {
        showConfirmDialog('Switch to Creator Mode? This clears all your entries and notes.', () => {
            cancelPendingHintPreview();
            // Reset all user progress and solver state.
            moveHistory = [];
            const grid = document.getElementById('puzzle-grid');
            Array.from(grid.children).forEach(cell => {
                if (cell.classList.contains('black') || cell.classList.contains('hint')) return;
                cell.textContent = '';
                const row = parseInt(cell.dataset.row, 10);
                const col = parseInt(cell.dataset.col, 10);
                puzzle[row][col] = null;
                notes[row][col].clear();
                updateNotes(cell, new Set());
            });
            solverValues = cloneValuesGrid(puzzle);
            solverNotes = createEmptyNotesGrid();
            solverStateInitialized = false;
            solverBasePuzzle = null;
            solverBaseNotes = null;
            if (selectedCell) {
                selectedCell.classList.remove('selected', 'selected-strong');
                selectedCell = null;
            }
            notesMode = false;
            hasCelebratedCurrentPuzzle = false;
            currentMode = 'creator';
            updateModeControls();
            renderBoardForCurrentMode();
            resetPrimaryActionButtonVisual();
            clearSolverValidation();
            syncCreatorPanelFromBoard();
            validateCreatorBoardString(buildBoardStringFromGrid());
            savePuzzleState();
        });
        return;
    } else {
        cancelPendingHintPreview();
        if (currentMode === 'solver') {
            transferSolverProgressToPuzzle();
        }
        currentMode = 'puzzle';
    }

    updateModeControls();
    renderBoardForCurrentMode();
    resetPrimaryActionButtonVisual();
    if (currentMode === 'solver') {
        refreshSolverModeValidation();
    } else if (currentMode === 'creator') {
        clearSolverValidation();
    } else {
        clearSolverValidation();
    }
    savePuzzleState();
}

function initializeSolverStateFromPuzzle() {
    solverValues = cloneValuesGrid(puzzle);
    solverNotes = createEmptyNotesGrid();
    // Only honour user-note intersections when the board is error-free.  If the
    // user has contradictory values placed, trivial candidates can be empty (a
    // digit already appears elsewhere in the row/col), and intersecting with that
    // empty set would wipe out all solver candidates.
    const boardIsClean = !boardHasUserErrors();
    for (let row = 0; row < 9; row++) {
        for (let col = 0; col < 9; col++) {
            const cell = getCellAt(row, col);
            if (cell.classList.contains('black') || cell.classList.contains('hint') || solverValues[row][col] !== null) {
                solverNotes[row][col].clear();
                continue;
            }
            const trivial = computeTrivialCandidates(row, col, solverValues);
            if (boardIsClean && notes[row][col] && notes[row][col].size > 0) {
                // Intersect trivial candidates with user's notes: user-removed candidates
                // stay removed; candidates the user has that trivial rules would eliminate
                // will be shown as red strikethrough by renderBoardForCurrentMode.
                const intersection = new Set(Array.from(trivial).filter(n => notes[row][col].has(n)));
                solverNotes[row][col] = intersection.size > 0 ? intersection : trivial;
            } else {
                solverNotes[row][col] = trivial;
            }
        }
    }
    solverStateInitialized = true;
    solverBasePuzzle = cloneValuesGrid(puzzle);
    solverBaseNotes = cloneNotesGrid(notes);
}

// Returns true if any puzzle cell value has changed since the last solver init.
function puzzleValuesChangedSinceSolverInit() {
    if (!solverStateInitialized || !solverBasePuzzle) return true;
    for (let r = 0; r < 9; r++) {
        for (let c = 0; c < 9; c++) {
            if (puzzle[r][c] !== solverBasePuzzle[r][c]) return true;
        }
    }
    return false;
}

// Returns true if any puzzle-mode notes have changed since the last solver init.
function notesChangedSinceSolverInit() {
    if (!solverBaseNotes) return false;
    for (let r = 0; r < 9; r++) {
        for (let c = 0; c < 9; c++) {
            if (!areSetsEqual(notes[r][c], solverBaseNotes[r][c])) return true;
        }
    }
    return false;
}

// When re-entering Solver Mode without value changes: remove from solverNotes any
// candidates the user explicitly deleted from their puzzle notes since last time.
function updateSolverStateFromNoteChanges() {
    // Skip candidate removal when the board is in an error state – trivial candidates
    // may already be empty, so removing further would blank out the solver.
    if (boardHasUserErrors()) {
        solverBasePuzzle = cloneValuesGrid(puzzle);
        solverBaseNotes = cloneNotesGrid(notes);
        return;
    }
    for (let row = 0; row < 9; row++) {
        for (let col = 0; col < 9; col++) {
            const cell = getCellAt(row, col);
            if (cell.classList.contains('black') || cell.classList.contains('hint') || solverValues[row][col] !== null) continue;
            if (!solverBaseNotes) continue;
            const baseNotes = solverBaseNotes[row][col];
            const currentNotes = notes[row][col];
            // Remove from solver any candidate the user removed from their notes.
            for (const n of Array.from(solverNotes[row][col])) {
                if (baseNotes.has(n) && !currentNotes.has(n)) {
                    solverNotes[row][col].delete(n);
                }
            }
        }
    }
    solverBasePuzzle = cloneValuesGrid(puzzle);
    solverBaseNotes = cloneNotesGrid(notes);
}

function seedSolverCandidatesFromCurrentValues() {
    for (let row = 0; row < 9; row++) {
        for (let col = 0; col < 9; col++) {
            const cell = getCellAt(row, col);
            if (cell.classList.contains('black') || cell.classList.contains('hint') || solverValues[row][col] !== null) {
                solverNotes[row][col].clear();
                continue;
            }
            if (notes[row][col] && notes[row][col].size > 0) {
                solverNotes[row][col] = new Set(notes[row][col]);
            } else {
                solverNotes[row][col] = computeTrivialCandidates(row, col, solverValues);
            }
        }
    }
}

function normalizeSolverState() {
    for (let row = 0; row < 9; row++) {
        for (let col = 0; col < 9; col++) {
            const cell = getCellAt(row, col);
            if (cell.classList.contains('black')) {
                solverValues[row][col] = null;
                solverNotes[row][col].clear();
                continue;
            }
            if (cell.classList.contains('hint')) {
                let given = puzzle[row][col];
                if (given === null || given === undefined) {
                    const fromSolver = solverValues[row][col];
                    if (Number.isInteger(fromSolver) && fromSolver >= 1 && fromSolver <= 9) {
                        given = fromSolver;
                    } else {
                        const fromCell = parseInt(cell.textContent.trim(), 10);
                        if (Number.isInteger(fromCell) && fromCell >= 1 && fromCell <= 9) {
                            given = fromCell;
                        }
                    }
                }
                if (Number.isInteger(given) && given >= 1 && given <= 9) {
                    puzzle[row][col] = given;
                    solverValues[row][col] = given;
                } else {
                    solverValues[row][col] = null;
                }
                solverNotes[row][col].clear();
            }
        }
    }
}

function transferSolverProgressToPuzzle() {
    let willChange = false;

    for (let row = 0; row < 9 && !willChange; row++) {
        for (let col = 0; col < 9; col++) {
            const cell = getCellAt(row, col);
            if (cell.classList.contains('black') || cell.classList.contains('hint')) {
                continue;
            }

            const solverValue = solverValues[row][col];
            if (solverValue !== null && solverValue !== undefined && puzzle[row][col] !== solverValue) {
                willChange = true;
                break;
            }

            // For cells without a solver-found value: transfer refined candidates
            // back only to cells where the user had notes when entering solver mode.
            if ((solverValue === null || solverValue === undefined)
                    && solverBaseNotes && solverBaseNotes[row][col] && solverBaseNotes[row][col].size > 0
                    && !areSetsEqual(notes[row][col], solverNotes[row][col])) {
                willChange = true;
                break;
            }
        }
    }

    if (!willChange) {
        return;
    }

    saveBoardSnapshotMove();

    for (let row = 0; row < 9; row++) {
        for (let col = 0; col < 9; col++) {
            const cell = getCellAt(row, col);
            if (cell.classList.contains('black') || cell.classList.contains('hint')) {
                continue;
            }

            const solverValue = solverValues[row][col];
            if (solverValue !== null && solverValue !== undefined) {
                // Transfer solver-found value as a regular (non-hint) puzzle value.
                puzzle[row][col] = solverValue;
                notes[row][col].clear();
            } else if (solverBaseNotes && solverBaseNotes[row][col] && solverBaseNotes[row][col].size > 0) {
                // User had notes when entering solver → update notes with solver's
                // refined candidate set (may be a strict subset after hint steps).
                notes[row][col] = new Set(solverNotes[row][col]);
            }
            // Cells with no prior notes and no solver value: leave notes untouched.
        }
    }

    // After pushing solver progress back into puzzle/notes, update the
    // baseline snapshot so that re-entering Solver mode doesn't look like
    // the puzzle changed (the difference was just the transfer itself).
    solverBasePuzzle = cloneValuesGrid(puzzle);
    solverBaseNotes  = cloneNotesGrid(notes);
}

function clearSolverValidation() {
    const grid = document.getElementById('puzzle-grid');
    Array.from(grid.children).forEach(cell => cell.classList.remove('error'));
    const actionBtn = document.getElementById('primaryActionButton');
    if (actionBtn && currentMode === 'puzzle') {
        actionBtn.disabled = false;
    }
    resetPrimaryActionButtonVisual();
}

function refreshSolverModeValidation() {
    if (currentMode !== 'solver') {
        return;
    }
    const hasErrors = boardHasUserErrors(true);
    const actionBtn = document.getElementById('primaryActionButton');
    if (actionBtn) {
        actionBtn.disabled = hasErrors;
    }
}

function renderBoardForCurrentMode() {
    const grid = document.getElementById('puzzle-grid');
    Array.from(grid.children).forEach(cell => {
        const row = parseInt(cell.dataset.row);
        const col = parseInt(cell.dataset.col);
        if (cell.classList.contains('black') || cell.classList.contains('hint')) {
            return;
        }

        const valueSource = currentMode === 'solver' ? solverValues : puzzle;
        const candidateSource = currentMode === 'solver' ? solverNotes : notes;
        const value = valueSource[row][col];

        if (value !== null && value !== undefined) {
            cell.textContent = value.toString();
            updateNotes(cell, new Set());
            return;
        }

        cell.textContent = '';
        if (currentMode === 'solver') {
            const userConflict = solverHintUsed ? [] : Array.from(notes[row][col]).filter(d => !candidateSource[row][col].has(d));
            updateNotes(cell, candidateSource[row][col], userConflict);
        } else {
            updateNotes(cell, candidateSource[row][col]);
        }
    });
}

function getCellAt(row, col) {
    const grid = document.getElementById('puzzle-grid');
    return grid.children[row * 9 + col];
}

function getPlacedValueAt(row, col, valuesSource = puzzle) {
    const cell = getCellAt(row, col);
    if (cell.classList.contains('black')) {
        const text = cell.textContent.trim();
        if (text >= '1' && text <= '9') {
            return parseInt(text, 10);
        }
        return null;
    }
    return valuesSource[row][col];
}

function getCompartmentCellsInDirection(row, col, dr, dc) {
    const cell = getCellAt(row, col);
    if (cell.classList.contains('black')) {
        return [];
    }

    let rr = row;
    let cc = col;
    while (rr - dr >= 0 && rr - dr < 9 && cc - dc >= 0 && cc - dc < 9) {
        const prev = getCellAt(rr - dr, cc - dc);
        if (prev.classList.contains('black')) {
            break;
        }
        rr -= dr;
        cc -= dc;
    }

    const cells = [];
    while (rr >= 0 && rr < 9 && cc >= 0 && cc < 9) {
        const cur = getCellAt(rr, cc);
        if (cur.classList.contains('black')) {
            break;
        }
        cells.push([rr, cc]);
        rr += dr;
        cc += dc;
    }
    return cells;
}

function getCompartmentsForCell(row, col) {
    const rowComp = getCompartmentCellsInDirection(row, col, 0, 1);
    const colComp = getCompartmentCellsInDirection(row, col, 1, 0);
    return [rowComp, colComp];
}

function passesCompartmentDistanceRule(row, col, candidate, valuesSource = puzzle) {
    const compartments = getCompartmentsForCell(row, col);

    for (const comp of compartments) {
        if (comp.length <= 1) {
            continue;
        }

        const m = comp.length;
        const fixed = [];
        for (const [rr, cc] of comp) {
            if (rr === row && cc === col) {
                continue;
            }
            const v = getPlacedValueAt(rr, cc, valuesSource);
            if (v !== null) {
                fixed.push(v);
            }
        }

        for (const c of fixed) {
            if (candidate >= c + m || candidate <= c - m) {
                return false;
            }
        }
    }

    return true;
}

function isBlockedInRowCol(row, col, value) {
    for (let c = 0; c < 9; c++) {
        if (c === col) continue;
        if (getPlacedValueAt(row, c, currentMode === 'solver' ? solverValues : puzzle) === value) {
            return true;
        }
    }
    for (let r = 0; r < 9; r++) {
        if (r === row) continue;
        if (getPlacedValueAt(r, col, currentMode === 'solver' ? solverValues : puzzle) === value) {
            return true;
        }
    }
    return false;
}

function computeTrivialCandidates(row, col, valuesSource) {
    const trivial = new Set();
    for (let n = 1; n <= 9; n++) {
        let blocked = false;
        for (let c = 0; c < 9; c++) {
            if (c !== col && getPlacedValueAt(row, c, valuesSource) === n) {
                blocked = true;
                break;
            }
        }
        if (blocked) continue;
        for (let r = 0; r < 9; r++) {
            if (r !== row && getPlacedValueAt(r, col, valuesSource) === n) {
                blocked = true;
                break;
            }
        }
        if (blocked) continue;
        if (!passesCompartmentDistanceRule(row, col, n, valuesSource)) {
            continue;
        }
        trivial.add(n);
    }
    return trivial;
}

function syncSolverCandidates() {
    normalizeSolverState();

    for (let row = 0; row < 9; row++) {
        for (let col = 0; col < 9; col++) {
            const cell = getCellAt(row, col);
            if (cell.classList.contains('black') || cell.classList.contains('hint') || solverValues[row][col] !== null) {
                solverNotes[row][col].clear();
                continue;
            }

            let candidateSet = new Set(solverNotes[row][col]);

            candidateSet = new Set(
                Array.from(candidateSet).filter(n => {
                    if (!passesCompartmentDistanceRule(row, col, n, solverValues)) {
                        return false;
                    }
                    for (let c = 0; c < 9; c++) {
                        if (c !== col && getPlacedValueAt(row, c, solverValues) === n) return false;
                    }
                    for (let r = 0; r < 9; r++) {
                        if (r !== row && getPlacedValueAt(r, col, solverValues) === n) return false;
                    }
                    return true;
                })
            );

            solverNotes[row][col] = candidateSet;
        }
    }

    if (currentMode === 'solver') {
        renderBoardForCurrentMode();
    }
}

function buildBoardString() {
    if (currentMode === 'creator') {
        return buildBoardStringFromGrid();
    }

    const valueSource = currentMode === 'solver' ? solverValues : puzzle;
    const chars = [];
    for (let row = 0; row < 9; row++) {
        for (let col = 0; col < 9; col++) {
            const cell = getCellAt(row, col);
            if (cell.classList.contains('black')) {
                const text = cell.textContent.trim();
                if (text >= '1' && text <= '9') {
                    chars.push(String.fromCharCode('a'.charCodeAt(0) + parseInt(text, 10) - 1));
                } else {
                    chars.push('#');
                }
            } else {
                const v = valueSource[row][col];
                if (v === null) {
                    chars.push('.');
                } else {
                    chars.push(v.toString());
                }
            }
        }
    }
    return chars.join('');
}

function buildCandidatesPayload() {
    const valueSource = currentMode === 'solver' ? solverValues : puzzle;
    const candidateSource = currentMode === 'solver' ? solverNotes : notes;
    const out = [];
    for (let row = 0; row < 9; row++) {
        const line = [];
        for (let col = 0; col < 9; col++) {
            const cell = getCellAt(row, col);
            if (cell.classList.contains('black') || valueSource[row][col] !== null) {
                line.push([]);
            } else {
                line.push(Array.from(candidateSource[row][col]).sort((a, b) => a - b));
            }
        }
        out.push(line);
    }
    return JSON.stringify(out);
}

function setHintDescription(text) {
    const el = document.getElementById('hintDescription');
    if (el) {
        el.textContent = text || '';
    }
}

function clearHintVisuals() {
    const grid = document.getElementById('puzzle-grid');
    Array.from(grid.children).forEach(cell => {
        cell.classList.remove('hint-affected');
        const notesContainer = cell.querySelector('.notes');
        if (!notesContainer) {
            return;
        }
        Array.from(notesContainer.children).forEach(span => span.classList.remove('hint-removed'));
    });
}

function markRemovedCandidate(row, col, digit) {
    const cell = getCellAt(row, col);
    let notesContainer = cell.querySelector('.notes');
    if (!notesContainer) {
        notesContainer = createNotesContainer();
        cell.appendChild(notesContainer);
    }
    const span = notesContainer.children[digit - 1];
    if (span) {
        span.classList.add('hint-removed');
    }
}

function previewHint(hintData) {
    clearHintVisuals();
    const immediateEffects = hintData.immediate_effects || [];
    const valueSource = currentMode === 'solver' ? solverValues : puzzle;
    for (const effect of immediateEffects) {
        const row = effect.row;
        const col = effect.col;
        const cell = getCellAt(row, col);
        if (valueSource[row][col] !== null && valueSource[row][col] !== undefined) {
            continue;
        }
        if (!cell.classList.contains('black')) {
            cell.classList.add('hint-affected');
        }
        const removed = effect.removed || [];
        for (const digit of removed) {
            markRemovedCandidate(row, col, digit);
        }
    }
}

function applyHintEffectsToSolver(effects) {
    for (const effect of effects || []) {
        const row = effect.row;
        const col = effect.col;

        if (effect.set_value !== null && effect.set_value !== undefined) {
            solverValues[row][col] = effect.set_value;
            solverNotes[row][col].clear();
        }

        for (const digit of (effect.removed || [])) {
            solverNotes[row][col].delete(digit);
        }
    }

    syncSolverCandidates();
    renderBoardForCurrentMode();
}

function resetHintButton() {
    const actionButton = document.getElementById('primaryActionButton');
    if (actionButton) {
        actionButton.textContent = currentMode === 'solver' ? 'Hint' : 'Check';
    }
}

function cancelPendingHintPreview() {
    if (!pendingHint) {
        return;
    }
    pendingHint = null;
    clearHintVisuals();
    setHintDescription('');
    resetHintButton();
    updateModeControls();
}

async function loadHintApi() {
    if (hintApiModule) {
        return hintApiModule;
    }
    const wasmModule = await import(/* @vite-ignore */ import.meta.env.BASE_URL + 'pkg/str8ts.js');
    await wasmModule.default();
    hintApiModule = wasmModule;
    return hintApiModule;
}

async function handleHintButton() {
    const hintButton = document.getElementById('primaryActionButton');
    if (pendingHint) {
        applyHintEffectsToSolver(pendingHint.immediate_effects);
        applyHintEffectsToSolver(pendingHint.propagation_effects);
        pendingHint = null;
        clearHintVisuals();
        setHintDescription('');
        resetHintButton();
        hasCelebratedCurrentPuzzle = false;
        renderBoardForCurrentMode();
        refreshSolverModeValidation();
        return;
    }

    refreshSolverModeValidation();
    if (boardHasUserErrors(true)) {
        if (hintButton) {
            hintButton.disabled = true;
        }
        return;
    }

    syncSolverCandidates();

    try {
        const api = await loadHintApi();
        const responseText = api.human_single_step(buildBoardString(), buildCandidatesPayload());
        const response = JSON.parse(responseText);
        if (!response.ok) {
            // Check if the board is already fully solved (no empty white cells)
            const boardStr = buildBoardString();
            const isSolved = !boardStr.includes('.');
            const msg = isSolved ? 'Puzzle solved!' : (response.error || 'No hint available');
            hintButton.style.background = isSolved ? '#4caf50' : '#f44336';
            setTimeout(() => {
                hintButton.style.background = '#2196f3';
            }, 2000);
            setHintDescription(msg);
            return;
        }

        pendingHint = response;
        solverHintUsed = true;
        renderBoardForCurrentMode(); // re-render first so trivial-eliminated candidates disappear before previewHint adds its markings
        previewHint(response);
        hintButton.textContent = 'Apply';
        setHintDescription(response.description || response.strategy || 'Hint ready');
        if (hintButton) {
            hintButton.disabled = false;
        }
    } catch (error) {
        console.error('Error getting hint:', error);
        hintButton.style.background = '#f44336';
        setTimeout(() => {
            hintButton.style.background = '#2196f3';
        }, 2000);
    }
}

function handlePrimaryAction() {
    if (currentMode === 'creator') {
        return;
    }
    if (currentMode === 'solver') {
        handleHintButton();
        return;
    }
    checkSolution();
}

function checkPuzzleComplete() {
    if (currentMode !== 'puzzle') {
        return;
    }
    const grid = document.getElementById('puzzle-grid');
    const cells = Array.from(grid.children);
    
    const isComplete = cells.every(cell => {
        return cell.classList.contains('black') || cell.textContent.trim() !== '';
    });
    
    //if (isComplete) {
        verifySolution(cells);
    //}
}

function verifySolution() {
    if (hasCelebratedCurrentPuzzle) {
        return;
    }

    const grid = document.getElementById('puzzle-grid');
    let solved = true;
    for (let row = 0; row < 9; row++) {
        for (let col = 0; col < 9; col++) {
            const idx = row * 9 + col;
            const cell = grid.children[idx];
            if (!cell.classList.contains('black') && !cell.classList.contains('hint')) {
                const userValue = puzzle[row][col];
                const solutionValue = currentSolution[idx];
                if (userValue == null || userValue.toString() !== solutionValue) {
                    console.log(`Cell at (${row}, ${col}) is incorrect: user=${userValue}, solution=${solutionValue}`);
                    // either a note left or wrong number
                    return;
                }
            }
        }
    }
    hasCelebratedCurrentPuzzle = true;
    showSuccessModal();
}

function provideFeedback(button, isCorrect) {
    const feedbackDuration = 2000;
    button.style.background = isCorrect ? '#4caf50' : '#f44336';
    
    setTimeout(() => {
        const cells = document.querySelectorAll('.cell');
        cells.forEach(cell => {
            cell.classList.remove('wrong-number');
            cell.classList.remove('error');
        });
        button.style.background = '#2196f3';
    }, feedbackDuration);
}

// Modal Management
function showSuccessModal() {
    createConfetti();
    setTimeout(() => {
        document.getElementById('successModal').style.display = 'flex';
    }, 2000);
}

function closeSuccessModal() {
    document.getElementById('successModal').style.display = 'none';
}

function showConfirmDialog(message, onConfirm) {
    const modal = document.getElementById('confirmModal');
    const confirmButton = document.getElementById('modalConfirm');
    document.getElementById('modalMessage').textContent = message;
    
    modal.style.display = 'flex';
    
    confirmButton.replaceWith(confirmButton.cloneNode(true));
    
    document.getElementById('modalConfirm').addEventListener('click', () => {
        onConfirm();
        closeModal();
    });
}

function closeModal() {
    document.getElementById('confirmModal').style.display = 'none';
}

// Persistence Functions using localStorage
function savePuzzleState() {
    try {
        // Save complete puzzle state
        const gridState = [];
        const grid = document.getElementById('puzzle-grid');
        Array.from(grid.children).forEach(cell => {
            const row = parseInt(cell.dataset.row);
            const col = parseInt(cell.dataset.col);
            
            const cellState = {
                r: row,  // Use shorter key names to reduce storage size
                c: col,
                b: cell.classList.contains('black'),
                h: cell.classList.contains('hint'),
                v: (cell.classList.contains('black') || cell.classList.contains('hint'))
                    ? (cell.textContent.trim() || null)
                    : (puzzle[row][col] !== null && puzzle[row][col] !== undefined ? puzzle[row][col].toString() : null),
                n: cell.classList.contains('black') || cell.classList.contains('hint') 
                    ? [] 
                    : Array.from(notes[row][col] || [])
            };
            
            gridState.push(cellState);
        });

        // Save additional game state
        const gameState = {
            d: document.getElementById('difficulty').value,
            g: gridState,
            curr: currentSolution,
            mode: currentMode,
            sv: serializeValuesGrid(solverValues),
            sn: serializeNotesGrid(solverNotes)
        };

        // Convert to JSON and save in localStorage
        localStorage.setItem('str8tsPuzzleState', JSON.stringify(gameState));
    } catch (error) {
        console.error('Error saving puzzle state:', error);
    }
}

function loadPuzzleState() {
    try {
        // Retrieve from localStorage
        const savedStateJson = localStorage.getItem('str8tsPuzzleState');
        if (!savedStateJson) return null;

        const savedState = JSON.parse(savedStateJson);

        // Set difficulty dropdown
        const difficultySelect = document.getElementById('difficulty');
        if (savedState.d) {
            difficultySelect.value = savedState.d;
        }

        // Clear current grid
        clearCurrentPuzzle();

        // Restore solution
        if (savedState.curr) {
            currentSolution = savedState.curr;
        }

        const restoredSolverValues = deserializeValuesGrid(savedState.sv);
        const restoredSolverNotes = deserializeNotesGrid(savedState.sn);

        if (savedState.mode === 'solver' || savedState.mode === 'creator') {
            currentMode = savedState.mode;
        } else {
            currentMode = 'puzzle';
        }

        // Restore grid state
        if (savedState.g) {
            const grid = document.getElementById('puzzle-grid');
            savedState.g.forEach(cellState => {
                const cell = grid.children[cellState.r * 9 + cellState.c];

                // Restore cell classes
                if (cellState.b) {
                    cell.classList.add('black');
                }
                if (cellState.h) {
                    cell.classList.add('hint');
                }

                // Restore cell value
                if (!(cellState.n && cellState.n.length > 0) && cellState.v) {
                    const normalizedValue = String(cellState.v).trim();
                    const isSingleDigit = /^[1-9]$/.test(normalizedValue);
                    if (isSingleDigit) {
                        cell.textContent = normalizedValue;
                    }
                    if (isSingleDigit) {
                        const parsed = parseInt(normalizedValue, 10);
                        if (cellState.h) {
                            puzzle[cellState.r][cellState.c] = parsed;
                        } else if (!cellState.b) {
                            puzzle[cellState.r][cellState.c] = parsed;
                        }
                    }
                }

                // Restore notes (only for non-black, non-hint cells)
                if (!cellState.b && !cellState.h) {
                    if (cellState.n && cellState.n.length > 0) {
                        notes[cellState.r][cellState.c] = new Set(cellState.n);
                        updateNotes(cell, notes[cellState.r][cellState.c]);
                        
                        // Notes always mean no placed value in the user model
                        cell.textContent = '';
                        puzzle[cellState.r][cellState.c] = null;
                    }
                }
            });
        }
        if (restoredSolverValues) {
            solverValues = restoredSolverValues;
        } else {
            solverValues = cloneValuesGrid(puzzle);
        }

        if (restoredSolverNotes) {
            solverNotes = restoredSolverNotes;
        } else {
            solverNotes = createEmptyNotesGrid();
        }

        normalizeSolverState();
        solverStateInitialized = !!(restoredSolverValues || restoredSolverNotes);
        if (!restoredSolverNotes && currentMode === 'solver') {
            seedSolverCandidatesFromCurrentValues();
            solverStateInitialized = true;
        }

        if (currentMode === 'solver') {
            if (selectedCell) {
                selectedCell.classList.remove('selected');
                selectedCell.classList.remove('selected-strong');
            }
            selectedCell = null;
            syncSolverCandidates();
        } else if (currentMode === 'creator') {
            notesMode = false;
            syncCreatorPanelFromBoard();
            validateCreatorBoardString(buildBoardStringFromGrid());
        }

        updateModeControls();
        renderBoardForCurrentMode();
        if (currentMode === 'solver') {
            refreshSolverModeValidation();
        }

        return savedState;
    } catch (error) {
        console.error('Error loading puzzle state:', error);
        return null;
    }
}

// Add confetti function
function createConfetti() {
    const viewportArea = window.innerWidth * window.innerHeight;
    const pieceCount = Math.max(250, Math.min(1400, Math.floor(viewportArea / 1800)));
    const colors = ['#2196f3', '#4caf50', '#ff9800', '#e91e63', '#9c27b0'];
    const fragment = document.createDocumentFragment();

    for (let i = 0; i < pieceCount; i++) {
        const confetti = document.createElement('div');
        confetti.className = 'confetti';
        const size = 3 + Math.random() * 8;
        confetti.style.left = Math.random() * window.innerWidth + 'px';
        confetti.style.width = `${size}px`;
        confetti.style.height = `${size * (0.6 + Math.random() * 1.2)}px`;
        confetti.style.backgroundColor = colors[Math.floor(Math.random() * colors.length)];
        confetti.style.setProperty('--drift-x', `${-120 + Math.random() * 240}px`);
        confetti.style.setProperty('--spin', `${360 + Math.random() * 1080}deg`);
        confetti.style.animationDuration = `${2 + Math.random() * 2.5}s`;
        confetti.style.animationDelay = `${Math.random() * 1.2}s`;
        fragment.appendChild(confetti);
        
        // Clean up confetti after animation
        confetti.addEventListener('animationend', () => {
            confetti.remove();
        });
    }

    document.body.appendChild(fragment);
}

// Initialization is handled in window.onload