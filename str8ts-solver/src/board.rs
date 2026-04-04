use std::fmt::{Display, Formatter};

pub const N: usize = 9;
pub type Compartment = Vec<(usize, usize)>;

#[derive(Clone, Debug)]
pub struct SimpleStr8ts {
    pub numbers: [[u8; N]; N],
    pub is_black: [[bool; N]; N],
}

#[derive(Clone, Debug)]
pub struct HumanStr8ts {
    pub solved: [[bool; N]; N],
    pub numbers: [[u8; N]; N],
    pub is_black: [[bool; N]; N],
    pub candidates: [[u16; N]; N],
    pub row_compartments: Vec<Compartment>,
    pub col_compartments: Vec<Compartment>,
    pub cell_to_row_compartment: [[usize; N]; N],
    pub cell_to_col_compartment: [[usize; N]; N],
}

fn parse_board(board: &str) -> anyhow::Result<([[u8; N]; N], [[bool; N]; N], [[bool; N]; N])> {
    let compact: String = board.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.len() != 81 {
        anyhow::bail!("Board string must be exactly 81 characters long");
    }

    let mut numbers = [[0u8; N]; N];
    let mut is_black = [[false; N]; N];
    let mut solved = [[false; N]; N];

    for (idx, ch) in compact.chars().enumerate() {
        let r = idx / N;
        let c = idx % N;
        match ch {
            '#' => is_black[r][c] = true,
            '.' => {}
            'a'..='i' => {
                is_black[r][c] = true;
                numbers[r][c] = (ch as u8 - b'a') + 1;
            }
            '1'..='9' => {
                numbers[r][c] = (ch as u8 - b'0') as u8;
                solved[r][c] = true;
            }
            _ => anyhow::bail!("Invalid character: {ch}"),
        }
    }

    Ok((numbers, is_black, solved))
}

impl SimpleStr8ts {
    pub fn from_str(board: &str) -> anyhow::Result<Self> {
        let (numbers, is_black, _) = parse_board(board)?;
        Ok(Self { numbers, is_black })
    }

    pub fn solution_string(&self) -> String {
        let mut out = String::with_capacity(81);
        for r in 0..N {
            for c in 0..N {
                if self.is_black[r][c] {
                    let n = self.numbers[r][c];
                    if n == 0 {
                        out.push('#');
                    } else {
                        out.push((b'a' + n - 1) as char);
                    }
                } else {
                    out.push((b'0' + self.numbers[r][c]) as char);
                }
            }
        }
        out
    }
}

impl HumanStr8ts {
    pub fn from_str(board: &str) -> anyhow::Result<Self> {
        let (numbers, is_black, solved) = parse_board(board)?;

        let mut row_compartments = Vec::new();
        let mut col_compartments = Vec::new();
        let mut cell_to_row_compartment = [[0usize; N]; N];
        let mut cell_to_col_compartment = [[0usize; N]; N];

        for r in 0..N {
            let mut c = 0;
            while c < N {
                if !is_black[r][c] {
                    let mut comp = Vec::new();
                    while c < N && !is_black[r][c] {
                        comp.push((r, c));
                        c += 1;
                    }
                    let idx = row_compartments.len();
                    for &(rr, cc) in &comp {
                        cell_to_row_compartment[rr][cc] = idx;
                    }
                    row_compartments.push(comp);
                } else {
                    c += 1;
                }
            }
        }

        for c in 0..N {
            let mut r = 0;
            while r < N {
                if !is_black[r][c] {
                    let mut comp = Vec::new();
                    while r < N && !is_black[r][c] {
                        comp.push((r, c));
                        r += 1;
                    }
                    let idx = col_compartments.len();
                    for &(rr, cc) in &comp {
                        cell_to_col_compartment[rr][cc] = idx;
                    }
                    col_compartments.push(comp);
                } else {
                    r += 1;
                }
            }
        }

        let mut candidates = [[0u16; N]; N];
        for r in 0..N {
            for c in 0..N {
                if is_black[r][c] {
                    continue;
                }
                candidates[r][c] = if solved[r][c] {
                    1u16 << (numbers[r][c] - 1)
                } else {
                    0x01FF
                };
            }
        }

        for r in 0..N {
            for c in 0..N {
                if is_black[r][c] && numbers[r][c] != 0 {
                    let num = numbers[r][c];
                    let bit = !(1u16 << (num - 1));
                    for cc in 0..N {
                        if !is_black[r][cc] {
                            candidates[r][cc] &= bit;
                        }
                    }
                    for rr in 0..N {
                        if !is_black[rr][c] {
                            candidates[rr][c] &= bit;
                        }
                    }
                }
            }
        }

        for r in 0..N {
            for c in 0..N {
                if solved[r][c] {
                    let num = numbers[r][c];
                    let bit = !(1u16 << (num - 1));
                    for cc in 0..N {
                        if cc != c && !is_black[r][cc] {
                            candidates[r][cc] &= bit;
                        }
                    }
                    for rr in 0..N {
                        if rr != r && !is_black[rr][c] {
                            candidates[rr][c] &= bit;
                        }
                    }
                }
            }
        }

        Ok(Self {
            solved,
            numbers,
            is_black,
            candidates,
            row_compartments,
            col_compartments,
            cell_to_row_compartment,
            cell_to_col_compartment,
        })
    }

    pub fn to_simple(&self) -> SimpleStr8ts {
        SimpleStr8ts {
            numbers: self.numbers,
            is_black: self.is_black,
        }
    }

    pub fn load_from_simple(&mut self, simple: &SimpleStr8ts) {
        self.numbers = simple.numbers;
        for r in 0..N {
            for c in 0..N {
                self.solved[r][c] = !self.is_black[r][c] && self.numbers[r][c] != 0;
            }
        }
    }

    pub fn solution_string(&self) -> String {
        self.to_simple().solution_string()
    }
}

fn fmt_board(f: &mut Formatter<'_>, numbers: &[[u8; N]; N], is_black: &[[bool; N]; N]) -> std::fmt::Result {
    const BLACK_BG: &str = "\x1b[40m";
    const WHITE_BG: &str = "\x1b[47m";
    const RESET: &str = "\x1b[0m";
    const WHITE_FG: &str = "\x1b[37m";
    const BLACK_FG: &str = "\x1b[30m";

    for r in 0..N {
        for c in 0..N {
            let bg = if is_black[r][c] { BLACK_BG } else { WHITE_BG };
            let num = numbers[r][c];
            if num == 0 {
                write!(f, "{bg}   {RESET}")?;
            } else {
                let fg = if is_black[r][c] { WHITE_FG } else { BLACK_FG };
                write!(f, "{bg}{fg} {num} {RESET}")?;
            }
        }
        writeln!(f)?;
    }
    Ok(())
}

impl Display for SimpleStr8ts {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        fmt_board(f, &self.numbers, &self.is_black)
    }
}

impl Display for HumanStr8ts {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        fmt_board(f, &self.numbers, &self.is_black)
    }
}
