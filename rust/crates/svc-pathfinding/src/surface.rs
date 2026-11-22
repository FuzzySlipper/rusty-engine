//! Paths over surface cells: places on floors, walls and ceilings, each a
//! cell and a facing, that a surface crawler rests on. The graph knows each
//! place's cell, what entering it costs, and which places it joins; the
//! derivation that found them is the caller's.

use std::{cmp::Reverse, collections::BinaryHeap};

use core_space::VoxelCoord;

/// Surface places and the edges between them, by index.
#[derive(Debug, Clone, Default)]
pub struct SurfaceGraph {
    cells: Vec<VoxelCoord>,
    costs: Vec<u32>,
    /// Where each place's edges start in `targets`; one more than places.
    offsets: Vec<u32>,
    /// Each edge's destination and the caller's tag for it.
    targets: Vec<(u32, u8)>,
    components: Vec<u32>,
    component_count: u32,
    least_cost: u32,
}

impl SurfaceGraph {
    /// A graph of `places`, each its cell and the cost of entering it (at
    /// least one), and directed `edges` from place to place with a tag.
    pub fn new(
        places: impl IntoIterator<Item = (VoxelCoord, u32)>,
        edges: impl IntoIterator<Item = (u32, u32, u8)>,
    ) -> Self {
        let (cells, costs): (Vec<_>, Vec<_>) = places
            .into_iter()
            .map(|(cell, cost)| (cell, cost.max(1)))
            .unzip();
        let count = cells.len();
        let mut edges: Vec<_> = edges
            .into_iter()
            .filter(|&(from, to, _)| (from as usize) < count && (to as usize) < count)
            .collect();
        edges.sort_unstable();
        edges.dedup_by_key(|&mut (from, to, _)| (from, to));
        let mut offsets = vec![0_u32; count + 1];
        for &(from, _, _) in &edges {
            offsets[from as usize + 1] += 1;
        }
        for slot in 0..count {
            offsets[slot + 1] += offsets[slot];
        }
        let targets = edges.iter().map(|&(_, to, tag)| (to, tag)).collect();
        let least_cost = costs.iter().copied().min().unwrap_or(1);
        let mut graph = Self {
            cells,
            costs,
            offsets,
            targets,
            components: Vec::new(),
            component_count: 0,
            least_cost,
        };
        graph.label_components();
        graph
    }

    /// Groups of places joined by edges either way.
    fn label_components(&mut self) {
        let count = self.cells.len();
        let mut parent: Vec<u32> = (0..count as u32).collect();
        fn root(parent: &mut [u32], mut at: u32) -> u32 {
            while parent[at as usize] != at {
                let up = parent[parent[at as usize] as usize];
                parent[at as usize] = up;
                at = up;
            }
            at
        }
        for from in 0..count as u32 {
            for &(to, _) in self.edges(from) {
                let (a, b) = (root(&mut parent, from), root(&mut parent, to));
                if a != b {
                    parent[a.max(b) as usize] = a.min(b);
                }
            }
        }
        let mut labels = vec![u32::MAX; count];
        let mut next = 0;
        self.components = (0..count as u32)
            .map(|at| {
                let top = root(&mut parent, at) as usize;
                if labels[top] == u32::MAX {
                    labels[top] = next;
                    next += 1;
                }
                labels[top]
            })
            .collect();
        self.component_count = next;
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub fn edge_count(&self) -> usize {
        self.targets.len()
    }

    pub fn component_count(&self) -> u32 {
        self.component_count
    }

    pub fn component(&self, place: u32) -> u32 {
        self.components[place as usize]
    }

    pub fn cost(&self, place: u32) -> u32 {
        self.costs[place as usize]
    }

    /// The edges from `place`: destination and tag.
    pub fn edges(&self, place: u32) -> &[(u32, u8)] {
        let (start, end) = (
            self.offsets[place as usize] as usize,
            self.offsets[place as usize + 1] as usize,
        );
        &self.targets[start..end]
    }

    /// The tag of the edge from `from` to `to`, if there is one.
    pub fn edge(&self, from: u32, to: u32) -> Option<u8> {
        self.edges(from)
            .iter()
            .find(|&&(target, _)| target == to)
            .map(|&(_, tag)| tag)
    }

    /// No edge moves more than one cell along any axis and none costs less
    /// than the cheapest place, so this never overestimates.
    fn estimate(&self, from: u32, to: u32) -> u64 {
        let (a, b) = (self.cells[from as usize], self.cells[to as usize]);
        let cells = (a.x - b.x)
            .unsigned_abs()
            .max((a.y - b.y).unsigned_abs())
            .max((a.z - b.z).unsigned_abs());
        cells.saturating_mul(u64::from(self.least_cost))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfacePathOutcome {
    Reached,
    /// The goal is not reachable; `nearest` is the reachable place nearest it.
    NoPath,
    /// The search visited its budget first; `nearest` is the visited place
    /// estimated nearest the goal.
    BudgetExhausted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfacePath {
    pub outcome: SurfacePathOutcome,
    /// From start to goal when reached: each place and the tag of the edge
    /// it is entered by, the start's tag 0.
    pub places: Vec<(u32, u8)>,
    /// What entering each place after the start costs, summed.
    pub total_cost: u64,
    pub visited: u32,
    pub nearest: u32,
}

/// The least-cost path from `start` to `goal`, visiting at most
/// `max_visited` places. Across components it is `NoPath` without a search.
pub fn find_surface_path(
    graph: &SurfaceGraph,
    start: u32,
    goal: u32,
    max_visited: u32,
) -> SurfacePath {
    let unreached = |outcome, visited, nearest| SurfacePath {
        outcome,
        places: Vec::new(),
        total_cost: 0,
        visited,
        nearest,
    };
    if graph.component(start) != graph.component(goal) {
        let component = graph.component(start);
        let nearest = (0..graph.len() as u32)
            .filter(|&place| graph.component(place) == component)
            .min_by_key(|&place| (graph.estimate(place, goal), place))
            .unwrap_or(start);
        return unreached(SurfacePathOutcome::NoPath, 0, nearest);
    }
    let count = graph.len();
    let mut cost = vec![u64::MAX; count];
    let mut came: Vec<(u32, u8)> = vec![(u32::MAX, 0); count];
    let mut closed = vec![false; count];
    let mut open = BinaryHeap::new();
    cost[start as usize] = 0;
    open.push(Reverse((graph.estimate(start, goal), 0_u64, start)));
    let mut visited = 0_u32;
    let mut nearest = (graph.estimate(start, goal), start);
    while let Some(Reverse((_, so_far, at))) = open.pop() {
        if closed[at as usize] || so_far > cost[at as usize] {
            continue;
        }
        if visited >= max_visited {
            return unreached(SurfacePathOutcome::BudgetExhausted, visited, nearest.1);
        }
        closed[at as usize] = true;
        visited += 1;
        nearest = nearest.min((graph.estimate(at, goal), at));
        if at == goal {
            let mut places = vec![(goal, came[goal as usize].1)];
            let mut back = goal;
            while back != start {
                back = came[back as usize].0;
                places.push((back, came[back as usize].1));
            }
            places.reverse();
            places[0].1 = 0;
            return SurfacePath {
                outcome: SurfacePathOutcome::Reached,
                places,
                total_cost: so_far,
                visited,
                nearest: goal,
            };
        }
        for &(to, tag) in graph.edges(at) {
            let through = so_far + u64::from(graph.cost(to));
            if through < cost[to as usize] {
                cost[to as usize] = through;
                came[to as usize] = (at, tag);
                open.push(Reverse((through + graph.estimate(to, goal), through, to)));
            }
        }
    }
    unreached(SurfacePathOutcome::NoPath, visited, nearest.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A strip of `length` floor places at y = 0 and the same at y = 1 as a
    /// ceiling, the ends joined through walls: floors cost 1, the ceiling 3.
    fn strip(length: i64, gap: Option<i64>) -> SurfaceGraph {
        let mut places = Vec::new();
        for x in 0..length {
            places.push((VoxelCoord::new(x, 0, 0), 1));
        }
        for x in 0..length {
            places.push((VoxelCoord::new(x, 1, 0), 3));
        }
        let mut edges = Vec::new();
        let mut join = |a: u32, b: u32, tag: u8| {
            edges.push((a, b, tag));
            edges.push((b, a, tag));
        };
        let length = length as u32;
        for x in 0..length - 1 {
            if gap != Some(i64::from(x)) {
                join(x, x + 1, 0);
            }
            join(length + x, length + x + 1, 0);
        }
        join(0, length, 1);
        join(length - 1, 2 * length - 1, 1);
        SurfaceGraph::new(places, edges)
    }

    #[test]
    fn a_path_keeps_to_the_cheaper_floor_and_goes_over_the_ceiling_round_a_gap() {
        let graph = strip(6, None);
        let path = find_surface_path(&graph, 0, 5, 100);
        assert_eq!(path.outcome, SurfacePathOutcome::Reached);
        assert_eq!(
            path.places
                .iter()
                .map(|&(place, _)| place)
                .collect::<Vec<_>>(),
            [0, 1, 2, 3, 4, 5]
        );
        assert_eq!(path.total_cost, 5);
        let graph = strip(6, Some(2));
        assert_eq!(graph.component_count(), 1);
        let path = find_surface_path(&graph, 0, 5, 100);
        assert_eq!(path.outcome, SurfacePathOutcome::Reached);
        assert_eq!(
            path.places
                .iter()
                .map(|&(place, _)| place)
                .collect::<Vec<_>>(),
            [0, 6, 7, 8, 9, 10, 11, 5]
        );
        assert_eq!(path.places[1].1, 1);
        assert_eq!(path.total_cost, 6 * 3 + 1);
    }

    #[test]
    fn across_components_there_is_no_path_and_the_nearest_reachable_place() {
        let places = (0..4).map(|x| (VoxelCoord::new(x, 0, 0), 1));
        let graph = SurfaceGraph::new(places, [(0, 1, 0), (1, 0, 0), (2, 3, 0), (3, 2, 0)]);
        assert_eq!(graph.component_count(), 2);
        let path = find_surface_path(&graph, 0, 3, 100);
        assert_eq!(path.outcome, SurfacePathOutcome::NoPath);
        assert_eq!((path.visited, path.nearest), (0, 1));
    }

    #[test]
    fn a_search_stops_at_its_budget_with_the_nearest_visited_place() {
        let graph = strip(10, None);
        let path = find_surface_path(&graph, 0, 9, 3);
        assert_eq!(path.outcome, SurfacePathOutcome::BudgetExhausted);
        assert_eq!(path.visited, 3);
        assert_eq!(path.nearest, 2);
    }
}
