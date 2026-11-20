//! The alias table the paper draws its lights and environment-map texels from (148:9 §5, "Candidate
//! Generation", after Walker 1974), built by Vose's method (Vose, "A linear algorithm for generating random
//! numbers with a given distribution", IEEE TSE 1991). The shaders draw from the same tables
//! (restir_shading.wgsl `restirAlias`, restir_passes.wgsl `restirDrawEnvironment`).

/// One column of the table: the share of it its own entry keeps (of a column's height 1), and the entry that has
/// the rest.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AliasColumn {
    pub own: f32,
    pub alias: usize,
}

/// The table for `weights` (any scale): each weight scaled so they average 1; a column under 1 is topped up by an
/// entry over 1, which keeps what is left.
pub fn table(weights: &[f32]) -> Vec<AliasColumn> {
    let n = weights.len();
    let total: f64 = weights.iter().map(|&w| f64::from(w)).sum();
    let mut scaled: Vec<f64> = weights
        .iter()
        .map(|&w| f64::from(w) * n as f64 / total)
        .collect();
    let mut table = vec![AliasColumn { own: 1.0, alias: 0 }; n];
    let (mut small, mut large): (Vec<usize>, Vec<usize>) = (0..n).partition(|&i| scaled[i] < 1.0);
    while let (Some(&l), Some(&g)) = (small.last(), large.last()) {
        // Column l keeps its scaled share and takes the rest from g, which moves to the list its remainder fits.
        small.pop();
        large.pop();
        table[l] = AliasColumn {
            own: scaled[l] as f32,
            alias: g,
        };
        scaled[g] -= 1.0 - scaled[l];
        if scaled[g] < 1.0 {
            small.push(g);
        } else {
            large.push(g);
        }
    }
    for i in small.into_iter().chain(large) {
        table[i] = AliasColumn { own: 1.0, alias: i };
    }
    table
}

/// The entry a uniform number `u` in [0, 1) draws (`restirAlias`): its column from `u` times the count, then the
/// column's own entry if what is left of `u` falls under its share, else its alias.
pub fn draw(table: &[AliasColumn], u: f32) -> usize {
    let x = u * table.len() as f32;
    let i = (x as usize).min(table.len() - 1);
    if x - (i as f32) < table[i].own {
        i
    } else {
        table[i].alias
    }
}

/// The entry a column `i` and a second uniform number `v` draw: what `restirDrawEnvironment` reads, with a number
/// of its own for the column.
pub fn draw_in(table: &[AliasColumn], i: usize, v: f32) -> usize {
    if v < table[i].own {
        i
    } else {
        table[i].alias
    }
}

/// The probability the table gives each entry, of the whole: its own column's part plus the parts it tops up.
pub fn probabilities(table: &[AliasColumn]) -> Vec<f64> {
    let n = table.len() as f64;
    let mut out = vec![0.0; table.len()];
    for (i, c) in table.iter().enumerate() {
        out[i] += f64::from(c.own) / n;
        out[c.alias] += (1.0 - f64::from(c.own)) / n;
    }
    out
}
