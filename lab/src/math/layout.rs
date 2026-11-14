//! Graph layouts for math scenes (`rl_runtime/graph-layout`): where each node of a graph goes, computed from its
//! edges, never placed by hand. Every layout is deterministic (a spring layout starts from a seeded placement), so
//! a frame depends only on its scene's values; positions are in [-1, 1] on both axes, +y up.

use glam::Vec2;
use std::collections::{HashMap, VecDeque};
use std::f32::consts::TAU;
use std::sync::{Arc, Mutex, OnceLock};

/// The layouts a scene may ask for.
pub const LAYOUTS: [&str; 6] = ["circular", "grid", "bipartite", "tree", "layered", "spring"];

/// Where each of `count` nodes goes in the layout `name`, given the edges (from, to); cached by the whole input.
pub fn layout(
    count: usize,
    edges: &[(usize, usize)],
    name: &str,
    seed: u64,
) -> Result<Arc<Vec<Vec2>>, String> {
    type Key = (usize, Vec<(usize, usize)>, String, u64);
    static CACHE: OnceLock<Mutex<HashMap<Key, Arc<Vec<Vec2>>>>> = OnceLock::new();
    if let Some(&(a, b)) = edges.iter().find(|(a, b)| *a >= count || *b >= count) {
        return Err(format!(
            "the edge ({a}, {b}) names a node past the graph's {count}"
        ));
    }
    let key = (count, edges.to_vec(), name.to_string(), seed);
    let cache = CACHE.get_or_init(Default::default);
    if let Some(p) = cache.lock().unwrap().get(&key) {
        return Ok(p.clone());
    }
    let points = match name {
        "circular" => circular(count),
        "grid" => grid(count),
        "bipartite" => bipartite(count, edges),
        "tree" => tree(count, edges),
        "layered" => layered(count, edges),
        "spring" => spring(count, edges, seed),
        other => {
            return Err(format!(
                "no graph layout `{other}` (the layouts: {})",
                LAYOUTS.join(", ")
            ));
        }
    };
    let points = Arc::new(normalized(points));
    cache.lock().unwrap().insert(key, points.clone());
    Ok(points)
}

/// The points scaled evenly and centred into [-1, 1] (a single point at the centre).
fn normalized(points: Vec<Vec2>) -> Vec<Vec2> {
    if points.is_empty() {
        return points;
    }
    let lo = points
        .iter()
        .fold(Vec2::splat(f32::INFINITY), |a, p| a.min(*p));
    let hi = points
        .iter()
        .fold(Vec2::splat(f32::NEG_INFINITY), |a, p| a.max(*p));
    let center = (lo + hi) * 0.5;
    let half = (hi - lo) * 0.5;
    // Each axis fills [-1, 1] on its own, so a row stays a row and a tall tree stays tall.
    points
        .iter()
        .map(|p| {
            let d = *p - center;
            Vec2::new(
                if half.x > 1e-6 {
                    d.x / half.x
                } else {
                    0.0
                },
                if half.y > 1e-6 {
                    d.y / half.y
                } else {
                    0.0
                },
            )
        })
        .collect()
}

/// On a circle, the first at the top, clockwise.
fn circular(count: usize) -> Vec<Vec2> {
    (0..count)
        .map(|i| {
            let a = TAU * i as f32 / count.max(1) as f32;
            Vec2::new(a.sin(), a.cos())
        })
        .collect()
}

/// In rows of as many as the square root rounds up to, left to right, top to bottom.
fn grid(count: usize) -> Vec<Vec2> {
    let columns = (count as f32).sqrt().ceil().max(1.0) as usize;
    (0..count)
        .map(|i| Vec2::new((i % columns) as f32, -((i / columns) as f32)))
        .collect()
}

/// Each node's neighbours, both ways.
fn neighbours(count: usize, edges: &[(usize, usize)]) -> Vec<Vec<usize>> {
    let mut n = vec![Vec::new(); count];
    for &(a, b) in edges {
        n[a].push(b);
        n[b].push(a);
    }
    n
}

/// In two columns, the two sides of the graph's edges (a two-colouring by breadth-first search from each node
/// not yet coloured; a node without edges on the left), each column top to bottom in node order.
fn bipartite(count: usize, edges: &[(usize, usize)]) -> Vec<Vec2> {
    let n = neighbours(count, edges);
    let mut side: Vec<Option<bool>> = vec![None; count];
    for start in 0..count {
        if side[start].is_some() {
            continue;
        }
        side[start] = Some(false);
        let mut queue = VecDeque::from([start]);
        while let Some(a) = queue.pop_front() {
            // Each uncoloured neighbour takes the other side.
            for &b in &n[a] {
                if side[b].is_none() {
                    side[b] = Some(!side[a].unwrap());
                    queue.push_back(b);
                }
            }
        }
    }
    let mut rows = [0usize; 2];
    side.iter()
        .map(|s| {
            let column = usize::from(s.unwrap_or(false));
            rows[column] += 1;
            Vec2::new(column as f32, -(rows[column] as f32))
        })
        .collect()
}

/// The layer of each node: a source (no edge into it) at 0, and each other node one past the deepest node with an
/// edge into it (the longest path; a cycle's edges back are ignored).
fn layers(count: usize, edges: &[(usize, usize)]) -> Vec<usize> {
    let mut layer = vec![0usize; count];
    // Relaxing every edge `count` times settles the longest paths of a DAG.
    for _ in 0..count {
        let mut changed = false;
        for &(a, b) in edges {
            if a != b && layer[b] < layer[a] + 1 && layer[a] + 1 < count {
                layer[b] = layer[a] + 1;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    layer
}

/// A tree from its roots (nodes no edge reaches) down: each leaf takes the next place across, a parent sits over
/// the middle of its children.
fn tree(count: usize, edges: &[(usize, usize)]) -> Vec<Vec2> {
    let mut children = vec![Vec::new(); count];
    let mut has_parent = vec![false; count];
    for &(a, b) in edges {
        if !has_parent[b] && a != b {
            children[a].push(b);
            has_parent[b] = true;
        }
    }
    let mut x = vec![0.0f32; count];
    let mut depth = vec![0usize; count];
    let mut placed = vec![false; count];
    let mut next = 0.0f32;
    for root in (0..count).filter(|&i| !has_parent[i]) {
        place(
            root,
            0,
            &children,
            &mut x,
            &mut depth,
            &mut placed,
            &mut next,
        );
    }
    // A node left over (in a cycle) goes after the rest.
    for i in 0..count {
        if !placed[i] {
            place(i, 0, &children, &mut x, &mut depth, &mut placed, &mut next);
        }
    }
    (0..count)
        .map(|i| Vec2::new(x[i], -(depth[i] as f32)))
        .collect()
}

/// Places `node` at `d` and its subtree under it (depth first); its x is its children's middle, or the next place
/// across for a leaf.
fn place(
    node: usize,
    d: usize,
    children: &[Vec<usize>],
    x: &mut [f32],
    depth: &mut [usize],
    placed: &mut [bool],
    next: &mut f32,
) {
    placed[node] = true;
    depth[node] = d;
    let kids: Vec<usize> = children[node]
        .iter()
        .copied()
        .filter(|&c| !placed[c])
        .collect();
    if kids.is_empty() {
        x[node] = *next;
        *next += 1.0;
        return;
    }
    for &c in &kids {
        place(c, d + 1, children, x, depth, placed, next);
    }
    x[node] = kids.iter().map(|&c| x[c]).sum::<f32>() / kids.len() as f32;
}

/// A directed graph in layers, top to bottom (`layers`), each layer ordered to cross few edges: four sweeps down
/// and up, each ordering a layer by the mean place of its nodes' neighbours in the layer before (barycentres).
fn layered(count: usize, edges: &[(usize, usize)]) -> Vec<Vec2> {
    let layer = layers(count, edges);
    let depth = layer.iter().copied().max().unwrap_or(0);
    let mut order: Vec<Vec<usize>> = vec![Vec::new(); depth + 1];
    for (i, &l) in layer.iter().enumerate() {
        order[l].push(i);
    }
    let n = neighbours(count, edges);
    let mut at = vec![0.0f32; count];
    let set = |order: &Vec<Vec<usize>>, at: &mut [f32]| {
        for row in order {
            for (k, &i) in row.iter().enumerate() {
                at[i] = k as f32 - (row.len() as f32 - 1.0) / 2.0;
            }
        }
    };
    set(&order, &mut at);
    for sweep in 0..8 {
        // Even sweeps go down (each layer by the one above), odd ones up.
        let rows: Vec<usize> = if sweep % 2 == 0 {
            (1..=depth).collect()
        } else {
            (0..depth).rev().collect()
        };
        for l in rows {
            let other = if sweep % 2 == 0 {
                l - 1
            } else {
                l + 1
            };
            let key = |i: usize| {
                let near: Vec<f32> = n[i]
                    .iter()
                    .filter(|&&j| layer[j] == other)
                    .map(|&j| at[j])
                    .collect();
                if near.is_empty() {
                    at[i]
                } else {
                    near.iter().sum::<f32>() / near.len() as f32
                }
            };
            let mut keyed: Vec<(f32, usize)> = order[l].iter().map(|&i| (key(i), i)).collect();
            keyed.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            order[l] = keyed.into_iter().map(|(_, i)| i).collect();
            set(&order, &mut at);
        }
    }
    (0..count)
        .map(|i| Vec2::new(at[i], -(layer[i] as f32)))
        .collect()
}

/// A force layout (Fruchterman and Reingold 1991): nodes repel each other, edges pull their ends together, for a
/// fixed number of steps that cool, from a placement the seed scatters.
fn spring(count: usize, edges: &[(usize, usize)], seed: u64) -> Vec<Vec2> {
    let mut state = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407)
        | 1;
    let mut random = || {
        // xorshift64*, in [-1, 1).
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        (state.wrapping_mul(2685821657736338717) >> 40) as f32 / (1u64 << 23) as f32 - 1.0
    };
    let mut p: Vec<Vec2> = (0..count).map(|_| Vec2::new(random(), random())).collect();
    let k = (4.0 / count.max(1) as f32).sqrt();
    const STEPS: usize = 300;
    for step in 0..STEPS {
        let mut force = vec![Vec2::ZERO; count];
        for i in 0..count {
            for j in (i + 1)..count {
                // Every pair repels by k² over their distance.
                let d = p[i] - p[j];
                let len = d.length().max(0.01);
                let f = d / len * (k * k / len);
                force[i] += f;
                force[j] -= f;
            }
        }
        for &(a, b) in edges {
            // Every edge pulls by its length squared over k.
            let d = p[a] - p[b];
            let len = d.length().max(0.01);
            let f = d / len * (len * len / k);
            force[a] -= f;
            force[b] += f;
        }
        let temperature = 0.1 * (1.0 - step as f32 / STEPS as f32) + 0.002;
        for i in 0..count {
            let len = force[i].length();
            if len > 0.0 {
                p[i] += force[i] / len * len.min(temperature);
            }
        }
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    fn distinct(points: &[Vec2]) -> bool {
        points
            .iter()
            .enumerate()
            .all(|(i, a)| points[i + 1..].iter().all(|b| a.distance(*b) > 0.05))
    }

    #[test]
    fn every_layout_places_every_node_apart_and_the_same_each_time() {
        let edges = [(0, 1), (0, 2), (1, 3), (1, 4), (2, 5), (2, 6)];
        for name in LAYOUTS {
            let a = layout(7, &edges, name, 3).unwrap();
            assert_eq!(a.len(), 7, "{name}");
            assert!(distinct(&a), "{name}: {a:?}");
            assert!(
                a.iter().all(|p| p.x.abs() <= 1.0001 && p.y.abs() <= 1.0001),
                "{name}"
            );
            let fresh = normalized(match name {
                "spring" => spring(7, &edges, 3),
                _ => a.to_vec(),
            });
            assert_eq!(*a, fresh, "{name}");
        }
    }

    #[test]
    fn a_tree_and_layers_run_top_down_and_a_bipartite_graph_splits() {
        let edges = [(0, 1), (0, 2), (1, 3)];
        let t = layout(4, &edges, "tree", 0).unwrap();
        assert!(t[0].y > t[1].y && t[1].y > t[3].y && (t[1].y - t[2].y).abs() < 1e-6);
        let l = layout(4, &edges, "layered", 0).unwrap();
        assert!(l[0].y > l[1].y && l[1].y > l[3].y);
        let b = layout(4, &[(0, 2), (0, 3), (1, 2)], "bipartite", 0).unwrap();
        assert!(b[0].x == b[1].x && b[2].x == b[3].x && b[0].x != b[2].x);
        assert!(layout(2, &[(0, 5)], "tree", 0).is_err());
        assert!(layout(2, &[], "nope", 0).is_err());
    }
}
