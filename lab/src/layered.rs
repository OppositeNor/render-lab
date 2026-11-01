//! A layered (Sugiyama-style) layout of a directed acyclic graph, left to
//! right: ranks (columns) by the longest path from the sources, dummy points
//! on edges that skip ranks, a few barycentric sweeps to order each rank.
//! The knowledge panel (src/knowledge.rs) and the story's diagrams (src/diagram.rs)
//! use it.

use std::collections::BTreeMap;

/// Where each node is (its centre), and the edges' bends (layout units; x
/// grows from the first column on the left, y downward).
#[derive(Clone, Debug, Default)]
pub struct Layout {
    pub positions: Vec<(f32, f32)>,
    pub widths: Vec<f32>,
    pub heights: Vec<f32>,
    /// Per edge (from, to), in the order given: the points between them.
    pub bends: Vec<(usize, usize, Vec<(f32, f32)>)>,
    pub size: (f32, f32),
}

/// The space between columns, between the nodes of a column, and the slot of
/// a dummy point (layout units).
#[derive(Clone, Copy, Debug)]
pub struct Spacing {
    pub column: f32,
    pub row: f32,
    pub dummy: f32,
}

/// Each node's rank: one more than its highest predecessor's (sources 0).
/// `edges` (from, to) must be acyclic.
pub fn ranks(n: usize, edges: &[(usize, usize)]) -> Vec<usize> {
    fn visit(i: usize, before: &[Vec<usize>], rank: &mut [Option<usize>]) -> usize {
        if let Some(r) = rank[i] {
            return r;
        }
        rank[i] = Some(0);
        let r = before[i].iter().map(|&p| visit(p, before, rank) + 1).max().unwrap_or(0);
        rank[i] = Some(r);
        r
    }
    let mut before = vec![Vec::new(); n];
    for &(a, b) in edges {
        before[b].push(a);
    }
    let mut rank = vec![None; n];
    (0..n).map(|i| visit(i, &before, &mut rank)).collect()
}

/// Lays out `sizes.len()` nodes (width, height) joined by `edges` (from, to;
/// acyclic). `order` lists every node once: the first order of each column.
pub fn layout(sizes: &[(f32, f32)], edges: &[(usize, usize)], order: &[usize], spacing: Spacing) -> Layout {
    #[derive(Clone, Copy, PartialEq)]
    enum Item {
        Node(usize),
        /// Edge index, rank.
        Dummy(usize, usize),
    }
    let n = sizes.len();
    let rank = ranks(n, edges);
    let top = rank.iter().copied().max().unwrap_or(0);
    let mut rows: Vec<Vec<Item>> = vec![Vec::new(); top + 1];
    for &i in order {
        rows[rank[i]].push(Item::Node(i));
    }
    for (e, &(r, i)) in edges.iter().enumerate() {
        for k in rank[r] + 1..rank[i] {
            rows[k].push(Item::Dummy(e, k));
        }
    }
    // The items next to an item in the row below (or above).
    let neighbours = |item: Item, up: bool| -> Vec<Item> {
        let step = |e: usize, k: usize| -> Item {
            let (r, i) = edges[e];
            if up {
                if k + 1 < rank[i] { Item::Dummy(e, k + 1) } else { Item::Node(i) }
            } else if k - 1 > rank[r] {
                Item::Dummy(e, k - 1)
            } else {
                Item::Node(r)
            }
        };
        match item {
            Item::Node(i) => edges.iter().enumerate().filter(|(_, x)| if up { x.0 == i } else { x.1 == i }).map(|(e, _)| step(e, rank[i])).collect(),
            Item::Dummy(e, k) => vec![step(e, k)],
        }
    };
    for sweep in 0..8 {
        let up = sweep % 2 == 0;
        let ks: Vec<usize> = if up { (1..=top).collect() } else { (0..top).rev().collect() };
        for k in ks {
            let near_row = if up { k - 1 } else { k + 1 };
            let len = rows[near_row].len().max(1) as f32;
            let own_len = rows[k].len().max(1) as f32;
            let mut keyed: Vec<(f32, Item)> = rows[k]
                .iter()
                .enumerate()
                .map(|(p, &item)| {
                    let xs: Vec<f32> = neighbours(item, !up).iter().filter_map(|&m| rows[near_row].iter().position(|&x| x == m)).map(|q| q as f32 / len).collect();
                    (if xs.is_empty() { p as f32 / own_len } else { xs.iter().sum::<f32>() / xs.len() as f32 }, item)
                })
                .collect();
            keyed.sort_by(|a, b| a.0.total_cmp(&b.0));
            rows[k] = keyed.into_iter().map(|x| x.1).collect();
        }
    }
    // Ranks as columns from the left, each as wide as its widest node; a
    // column's items stacked and centred.
    let slot = |item: Item| match item {
        Item::Node(i) => sizes[i].1 + spacing.row,
        Item::Dummy(..) => spacing.dummy,
    };
    let column_width: Vec<f32> = rows.iter().map(|row| row.iter().map(|&x| if let Item::Node(i) = x { sizes[i].0 } else { spacing.dummy }).fold(spacing.dummy, f32::max)).collect();
    let column_height = |row: &Vec<Item>| row.iter().map(|&x| slot(x)).sum::<f32>();
    let height = rows.iter().map(column_height).fold(0.0, f32::max);
    let mut positions = vec![(0.0, 0.0); n];
    let mut dummy_at: BTreeMap<(usize, usize), (f32, f32)> = BTreeMap::new();
    let mut x = 0.0;
    for (k, row) in rows.iter().enumerate() {
        let mut y = (height - column_height(row)) / 2.0;
        for &item in row {
            let centre = (x + column_width[k] / 2.0, y + slot(item) / 2.0);
            match item {
                Item::Node(i) => positions[i] = centre,
                Item::Dummy(e, k) => {
                    dummy_at.insert((e, k), centre);
                }
            }
            y += slot(item);
        }
        x += column_width[k] + spacing.column;
    }
    let bends = edges.iter().enumerate().map(|(e, &(r, i))| (r, i, (rank[r] + 1..rank[i]).map(|k| dummy_at[&(e, k)]).collect())).collect();
    Layout {
        positions,
        widths: sizes.iter().map(|s| s.0).collect(),
        heights: sizes.iter().map(|s| s.1).collect(),
        bends,
        size: ((x - spacing.column).max(0.0), height),
    }
}
