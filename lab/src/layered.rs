//! A layered (Sugiyama-style) layout of a directed acyclic graph, left to
//! right: ranks (columns) by the longest path from the sources, dummy points
//! on edges that skip ranks, a few barycentric sweeps to order each rank.
//! The lab's graph view (src/graph_view.rs) lays every graph out with it:
//! `layout`, or `clustered`, which keeps groups of nodes in boxes of their own.

use std::collections::BTreeMap;

/// An edge (from, to) and the points between them.
pub type Bend = (usize, usize, Vec<(f32, f32)>);

/// Where each node is (its centre), and the edges' bends (layout units; x
/// grows from the first column on the left, y downward).
#[derive(Clone, Debug, Default)]
pub struct Layout {
    pub positions: Vec<(f32, f32)>,
    pub widths: Vec<f32>,
    pub heights: Vec<f32>,
    /// Per edge, in the order given.
    pub bends: Vec<Bend>,
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
        let r = before[i]
            .iter()
            .map(|&p| visit(p, before, rank) + 1)
            .max()
            .unwrap_or(0);
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
pub fn layout(
    sizes: &[(f32, f32)],
    edges: &[(usize, usize)],
    order: &[usize],
    spacing: Spacing,
) -> Layout {
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
                if k + 1 < rank[i] {
                    Item::Dummy(e, k + 1)
                } else {
                    Item::Node(i)
                }
            } else if k - 1 > rank[r] {
                Item::Dummy(e, k - 1)
            } else {
                Item::Node(r)
            }
        };
        match item {
            Item::Node(i) => edges
                .iter()
                .enumerate()
                .filter(|(_, x)| {
                    if up {
                        x.0 == i
                    } else {
                        x.1 == i
                    }
                })
                .map(|(e, _)| step(e, rank[i]))
                .collect(),
            Item::Dummy(e, k) => vec![step(e, k)],
        }
    };
    for sweep in 0..8 {
        let up = sweep % 2 == 0;
        let ks: Vec<usize> = if up {
            (1..=top).collect()
        } else {
            (0..top).rev().collect()
        };
        for k in ks {
            let near_row = if up {
                k - 1
            } else {
                k + 1
            };
            let len = rows[near_row].len().max(1) as f32;
            let own_len = rows[k].len().max(1) as f32;
            let mut keyed: Vec<(f32, Item)> = rows[k]
                .iter()
                .enumerate()
                .map(|(p, &item)| {
                    let xs: Vec<f32> = neighbours(item, !up)
                        .iter()
                        .filter_map(|&m| rows[near_row].iter().position(|&x| x == m))
                        .map(|q| q as f32 / len)
                        .collect();
                    (
                        if xs.is_empty() {
                            p as f32 / own_len
                        } else {
                            xs.iter().sum::<f32>() / xs.len() as f32
                        },
                        item,
                    )
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
    let column_width: Vec<f32> = rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|&x| {
                    if let Item::Node(i) = x {
                        sizes[i].0
                    } else {
                        spacing.dummy
                    }
                })
                .fold(spacing.dummy, f32::max)
        })
        .collect();
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
    let bends = edges
        .iter()
        .enumerate()
        .map(|(e, &(r, i))| {
            (
                r,
                i,
                (rank[r] + 1..rank[i]).map(|k| dummy_at[&(e, k)]).collect(),
            )
        })
        .collect();
    Layout {
        positions,
        widths: sizes.iter().map(|s| s.0).collect(),
        heights: sizes.iter().map(|s| s.1).collect(),
        bends,
        size: ((x - spacing.column).max(0.0), height),
    }
}

/// One node set of `clustered`: its members, and whether it is a group
/// (boxed; `Clustered::boxes`) or a single loose node.
#[derive(Clone, Debug)]
pub struct Cluster {
    pub members: Vec<usize>,
    pub boxed: bool,
}

/// `clustered`'s result: the layout (edges without bends: they are drawn as
/// curves), and each boxed cluster's rectangle (min x, min y, max x, max y),
/// with room above it for a title.
#[derive(Clone, Debug, Default)]
pub struct Clustered {
    pub layout: Layout,
    pub boxes: Vec<(usize, (f32, f32, f32, f32))>,
}

/// Lays out nodes in clusters, left to right by rank as `layout` does: each
/// cluster is a band of its own across the columns it spans, and clusters
/// whose columns do not overlap share a track, so no two boxes overlap.
/// `clusters` covers every node once, in the order its tracks should come
/// (top to bottom). A box is its members' bounds widened by `pad`, with
/// `title` more above; tracks are `gap` apart.
pub fn clustered(
    sizes: &[(f32, f32)],
    edges: &[(usize, usize)],
    clusters: &[Cluster],
    spacing: Spacing,
    pad: f32,
    title: f32,
    gap: f32,
) -> Clustered {
    let n = sizes.len();
    let rank = ranks(n, edges);
    let top = rank.iter().copied().max().unwrap_or(0);
    // Each cluster's span of columns.
    let spans: Vec<(usize, usize)> = clusters
        .iter()
        .map(|c| {
            c.members.iter().fold((usize::MAX, 0), |(lo, hi), &i| {
                (lo.min(rank[i]), hi.max(rank[i]))
            })
        })
        .collect();
    // Tracks: the first one free over a cluster's span, in order.
    let mut tracks: Vec<Vec<usize>> = Vec::new();
    let mut track_of = vec![0usize; clusters.len()];
    for (c, &(lo, hi)) in spans.iter().enumerate() {
        let free = tracks
            .iter()
            .position(|t| t.iter().all(|&o| spans[o].1 < lo || spans[o].0 > hi));
        let t = free.unwrap_or_else(|| {
            tracks.push(Vec::new());
            tracks.len() - 1
        });
        tracks[t].push(c);
        track_of[c] = t;
    }
    // In each track and column, its nodes (one cluster's), ordered by sweeps
    // of the barycentre of their neighbours' positions.
    let mut cells: Vec<Vec<Vec<usize>>> = vec![vec![Vec::new(); top + 1]; tracks.len()];
    for (c, cluster) in clusters.iter().enumerate() {
        for &i in &cluster.members {
            cells[track_of[c]][rank[i]].push(i);
        }
    }
    let slot = |i: usize| sizes[i].1 + spacing.row;
    let place = |cells: &Vec<Vec<Vec<usize>>>| -> (Vec<(f32, f32)>, Vec<f32>, f32) {
        let heights: Vec<f32> = cells
            .iter()
            .map(|t| {
                t.iter()
                    .map(|col| col.iter().map(|&i| slot(i)).sum::<f32>())
                    .fold(0.0, f32::max)
            })
            .collect();
        let column_width: Vec<f32> = (0..=top)
            .map(|k| {
                (0..n)
                    .filter(|&i| rank[i] == k)
                    .map(|i| sizes[i].0)
                    .fold(spacing.dummy, f32::max)
            })
            .collect();
        let mut positions = vec![(0.0, 0.0); n];
        let mut y0 = title;
        for (t, track) in cells.iter().enumerate() {
            let mut x = 0.0;
            for (k, col) in track.iter().enumerate() {
                let h: f32 = col.iter().map(|&i| slot(i)).sum();
                let mut y = y0 + (heights[t] - h) / 2.0;
                for &i in col {
                    positions[i] = (x + column_width[k] / 2.0, y + slot(i) / 2.0);
                    y += slot(i);
                }
                x += column_width[k] + spacing.column;
            }
            y0 += heights[t] + gap + title;
        }
        let width = column_width.iter().sum::<f32>() + spacing.column * top as f32;
        (positions, heights, width)
    };
    for sweep in 0..8 {
        let (positions, _, _) = place(&cells);
        for track in cells.iter_mut() {
            for col in track.iter_mut() {
                let mut keyed: Vec<(f32, usize)> = col
                    .iter()
                    .map(|&i| {
                        let ys: Vec<f32> = edges
                            .iter()
                            .filter_map(|&(a, b)| {
                                if sweep % 2 == 0 && b == i {
                                    Some(positions[a].1)
                                } else if sweep % 2 == 1 && a == i {
                                    Some(positions[b].1)
                                } else {
                                    None
                                }
                            })
                            .collect();
                        (
                            if ys.is_empty() {
                                positions[i].1
                            } else {
                                ys.iter().sum::<f32>() / ys.len() as f32
                            },
                            i,
                        )
                    })
                    .collect();
                keyed.sort_by(|a, b| a.0.total_cmp(&b.0));
                *col = keyed.into_iter().map(|x| x.1).collect();
            }
        }
    }
    let (positions, heights, width) = place(&cells);
    let boxes = clusters
        .iter()
        .enumerate()
        .filter(|(_, c)| c.boxed && !c.members.is_empty())
        .map(|(c, cluster)| {
            let b =
                cluster
                    .members
                    .iter()
                    .fold((f32::MAX, f32::MAX, f32::MIN, f32::MIN), |b, &i| {
                        let (x, y) = positions[i];
                        let (w, h) = sizes[i];
                        (
                            b.0.min(x - w / 2.0),
                            b.1.min(y - h / 2.0),
                            b.2.max(x + w / 2.0),
                            b.3.max(y + h / 2.0),
                        )
                    });
            (c, (b.0 - pad, b.1 - pad - title, b.2 + pad, b.3 + pad))
        })
        .collect();
    let height = heights.iter().sum::<f32>() + (gap + title) * heights.len() as f32;
    Clustered {
        layout: Layout {
            positions,
            widths: sizes.iter().map(|s| s.0).collect(),
            heights: sizes.iter().map(|s| s.1).collect(),
            bends: edges.iter().map(|&(a, b)| (a, b, Vec::new())).collect(),
            size: (width, height),
        },
        boxes,
    }
}
