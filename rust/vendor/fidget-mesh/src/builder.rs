//! Mesh builder data structure and implementation
use super::{
    Mesh, Octree,
    cell::{CellIndex, CellVertex},
    dc,
    frame::Frame,
};

/// Container used during construction of a [`Mesh`]
#[derive(Default)]
pub struct MeshBuilder {
    /// Map from indexes in [`Octree::verts`](super::Octree::verts) to
    /// `out.vertices`
    ///
    /// `usize::MAX` is used a marker for an unmapped vertex
    map: Vec<usize>,
    out: Mesh,
    face_arcs: std::collections::HashMap<(usize, usize, u8), usize>,
}

impl MeshBuilder {
    pub fn take(self) -> Mesh {
        self.out
    }

    pub(crate) fn cell(&mut self, octree: &Octree, cell: CellIndex<3>) {
        dc::dc_cell(octree, cell, self);
    }

    pub(crate) fn face<F: Frame>(
        &mut self,
        octree: &Octree,
        a: CellIndex<3>,
        b: CellIndex<3>,
    ) {
        dc::dc_face::<F>(octree, a, b, self)
    }

    /// Handles four cells that share a common edge aligned on axis `T`
    ///
    /// Cells positions are in the order `[0, U, U | V, U]`, i.e. a right-handed
    /// winding about `+T` (where `T, U, V` is a right-handed coordinate frame)
    pub(crate) fn edge<F: Frame>(
        &mut self,
        octree: &Octree,
        a: CellIndex<3>,
        b: CellIndex<3>,
        c: CellIndex<3>,
        d: CellIndex<3>,
    ) {
        dc::dc_edge::<F>(octree, a, b, c, d, self)
    }

    /// Keep the two contour arcs of a checkerboard face distinct. Both
    /// incident cells and both edge fans use the same canonical face owner.
    pub(crate) fn face_arc(
        &mut self,
        octree: &Octree,
        cells: [CellIndex<3>; 2],
        leaves: [super::cell::Leaf<3>; 2],
        edges: [super::types::Edge; 2],
    ) -> Option<usize> {
        use super::types::{Corner, Edge};
        let a = if cells[0].depth == cells[1].depth {
            usize::from(leaves[0].index > leaves[1].index)
        } else {
            usize::from(cells[0].depth < cells[1].depth)
        };
        let b = 1 - a;
        let axis = (0..3)
            .find(|&i| {
                let a = cells[a].bounds.bounds[i];
                let b = cells[b].bounds.bounds[i];
                a.upper() == b.lower() || a.lower() == b.upper()
            })
            .unwrap();
        let side = if cells[a].bounds.bounds[axis].midpoint()
            < cells[b].bounds.bounds[axis].midpoint()
        {
            1u8 << axis
        } else {
            0
        };
        let u = 1u8 << ((axis + 1) % 3);
        let v = 1u8 << ((axis + 2) % 3);
        let corners = [side, side | u, side | u | v, side | v];
        let filled = |c| leaves[a].mask & Corner::new(c);
        if !(0..4).all(|i| filled(corners[i]) != filled(corners[(i + 1) % 4])) {
            return None;
        }
        // All sampled leaves begin at the requested depth. A coarser leaf
        // requires collapse, which rejects any ambiguous child face; the
        // matching samples on its neighbor therefore keep both sides here.
        debug_assert_eq!(
            cells[0].depth, cells[1].depth,
            "collapse erased one side of an ambiguous shared face"
        );
        let (start, end) = edges[a].corners();
        let corner = if leaves[a].mask & start {
            start.get()
        } else {
            end.get()
        };
        let key = (leaves[a].index, axis, corner);
        if let Some(&index) = self.face_arcs.get(&key) {
            return Some(index);
        }
        let mut positions = [nalgebra::Vector3::zeros(); 2];
        let mut count = 0;
        for n in 0..12 {
            let edge = Edge::new(n);
            let (s, e) = edge.corners();
            if (s.get() == corner || e.get() == corner)
                && (s.get() & (1 << axis)) == side
                && (e.get() & (1 << axis)) == side
            {
                let intersection = leaves[a].edge(edge).unwrap();
                positions[count] = octree.verts
                    [leaves[a].index + intersection.edge.0 as usize]
                    .pos;
                count += 1;
            }
        }
        debug_assert_eq!(count, 2);
        let index = self.out.vertices.len();
        self.out.vertices.push((positions[0] + positions[1]) * 0.5);
        self.face_arcs.insert(key, index);
        self.out.face_arc_vertices.push(index);
        Some(index)
    }

    /// Record the given triangle
    ///
    /// Vertices are indices given by calls to [`Self::vertex`]
    ///
    /// The vertices are given in a clockwise winding with the intersection
    /// vertex (i.e. the one on the edge) always last.
    pub(crate) fn triangle(&mut self, a: usize, b: usize, c: usize) {
        self.out.triangles.push(nalgebra::Vector3::new(a, b, c))
    }

    /// Looks up the given vertex, localizing it within a cell
    ///
    /// `v` is an absolute offset into `verts`, which should be a reference to
    /// [`Octree::verts`](super::Octree::verts).
    pub(crate) fn vertex(
        &mut self,
        v: usize,
        verts: &[CellVertex<3>],
    ) -> usize {
        if v >= self.map.len() {
            self.map.resize(v + 1, usize::MAX);
        }
        match self.map[v] {
            usize::MAX => {
                let next_vert = self.out.vertices.len();
                self.out.vertices.push(verts[v].pos);
                self.map[v] = next_vert;

                next_vert
            }
            u => u,
        }
    }
}
