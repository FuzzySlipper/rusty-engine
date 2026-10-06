//! Cells held by X/Z column in flat arrays, for the planar searches.

use std::collections::BTreeMap;

use core_space::VoxelCoord;

/// One X/Z column.
type Column = (i64, i64);

/// The cells keep a box of columns while it has no more than this many
/// columns, plus `BOX_COLUMNS_PER_CELL` per cell; sparser cells keep only
/// their occupied columns. A box column costs a run whether or not it holds
/// a cell.
const BOX_ALLOWANCE: u128 = 65_536;
const BOX_COLUMNS_PER_CELL: u128 = 4;

/// Below this many positions that hold no cell, the table is not laid out
/// afresh.
const COMPACT_MINIMUM: usize = 1_024;

/// Values keyed by cell, held by X/Z column. Each column's cells take one run
/// of positions in flat arrays, so a position stands for its cell until the
/// table changes and a search can index its own arrays by position.
///
/// A column that outgrows its run moves it to the end of the arrays with
/// room to spare. When cells are added and the positions that hold no cell
/// outnumber the cells, the table is laid out afresh.
#[derive(Debug, Clone)]
pub(crate) struct ColumnTable<T> {
    index: ColumnIndex,
    /// Per column, where its cells lie.
    runs: Vec<Run>,
    cells: Vec<VoxelCoord>,
    values: Vec<T>,
    len: usize,
}

/// One column's positions: `count` cells from `start`, room for `capacity`.
#[derive(Debug, Clone, Copy, Default)]
struct Run {
    start: usize,
    capacity: usize,
    count: usize,
}

#[derive(Debug, Clone)]
enum ColumnIndex {
    /// Every column of a box, X-major.
    Box {
        min_x: i64,
        min_z: i64,
        width: u64,
        depth: u64,
    },
    /// The occupied columns only.
    Sparse(BTreeMap<Column, usize>),
}

impl ColumnIndex {
    /// The box from `min` to `max` inclusive, if `cells` fill enough of it.
    fn boxed(min: Column, max: Column, cells: usize) -> Option<Self> {
        let span = |low: i64, high: i64| (i128::from(high) - i128::from(low) + 1).max(0) as u128;
        let (width, depth) = (span(min.0, max.0), span(min.1, max.1));
        (width.saturating_mul(depth) <= BOX_ALLOWANCE + BOX_COLUMNS_PER_CELL * cells as u128)
            .then_some(Self::Box {
                min_x: min.0,
                min_z: min.1,
                width: width as u64,
                depth: depth as u64,
            })
    }

    /// Only `columns`, unique, in order.
    fn sparse(columns: impl Iterator<Item = Column>) -> Self {
        Self::Sparse(
            columns
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .enumerate()
                .map(|(slot, column)| (column, slot))
                .collect(),
        )
    }

    fn slots(&self) -> usize {
        match self {
            Self::Box { width, depth, .. } => (width * depth) as usize,
            Self::Sparse(slots) => slots.len(),
        }
    }

    fn slot(&self, (x, z): Column) -> Option<usize> {
        match self {
            Self::Box {
                min_x,
                min_z,
                width,
                depth,
            } => {
                let across = (x as u64).wrapping_sub(*min_x as u64);
                let along = (z as u64).wrapping_sub(*min_z as u64);
                (across < *width && along < *depth).then(|| (across * depth + along) as usize)
            }
            Self::Sparse(slots) => slots.get(&(x, z)).copied(),
        }
    }
}

/// The least and greatest X and Z of `columns`.
fn bounds(columns: impl Iterator<Item = Column>) -> (Column, Column) {
    columns.fold(
        ((i64::MAX, i64::MAX), (i64::MIN, i64::MIN)),
        |(min, max), (x, z)| ((min.0.min(x), min.1.min(z)), (max.0.max(x), max.1.max(z))),
    )
}

impl<T: Copy> ColumnTable<T> {
    /// A table of `entries`; of a repeated cell, the first value is kept.
    pub(crate) fn new(entries: impl IntoIterator<Item = (VoxelCoord, T)>) -> Self {
        let mut entries: Vec<_> = entries.into_iter().collect();
        entries.sort_by_key(|(cell, _)| (cell.x, cell.z, cell.y));
        entries.dedup_by_key(|(cell, _)| *cell);
        Self::laid_out(entries)
    }

    /// `entries` unique and by column, each column's run exactly full.
    fn laid_out(entries: Vec<(VoxelCoord, T)>) -> Self {
        let mut columns: Vec<(Column, Run)> = Vec::new();
        for (position, (cell, _)) in entries.iter().enumerate() {
            match columns.last_mut() {
                Some((column, run)) if *column == (cell.x, cell.z) => {
                    run.capacity += 1;
                    run.count += 1;
                }
                _ => columns.push((
                    (cell.x, cell.z),
                    Run {
                        start: position,
                        capacity: 1,
                        count: 1,
                    },
                )),
            }
        }
        let (min, max) = bounds(columns.iter().map(|&(column, _)| column));
        let index = ColumnIndex::boxed(min, max, entries.len())
            .unwrap_or_else(|| ColumnIndex::sparse(columns.iter().map(|&(column, _)| column)));
        let mut runs = vec![Run::default(); index.slots()];
        for (column, run) in columns {
            runs[index.slot(column).expect("a laid-out column has a slot")] = run;
        }
        let len = entries.len();
        let (cells, values) = entries.into_iter().unzip();
        Self {
            index,
            runs,
            cells,
            values,
            len,
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.len
    }

    /// How many positions a search over this table indexes.
    pub(crate) fn positions(&self) -> usize {
        self.cells.len()
    }

    /// The first position of a column and the cells it holds.
    pub(crate) fn column(&self, x: i64, z: i64) -> (usize, &[VoxelCoord]) {
        match self.index.slot((x, z)) {
            Some(slot) => {
                let run = self.runs[slot];
                (run.start, &self.cells[run.start..run.start + run.count])
            }
            None => (0, &[]),
        }
    }

    pub(crate) fn position(&self, cell: VoxelCoord) -> Option<usize> {
        let (start, cells) = self.column(cell.x, cell.z);
        cells
            .iter()
            .position(|held| held.y == cell.y)
            .map(|offset| start + offset)
    }

    pub(crate) fn cell(&self, position: usize) -> VoxelCoord {
        self.cells[position]
    }

    pub(crate) fn get(&self, cell: VoxelCoord) -> Option<&T> {
        self.position(cell).map(|position| &self.values[position])
    }

    pub(crate) fn get_mut(&mut self, cell: VoxelCoord) -> Option<&mut T> {
        self.position(cell)
            .map(|position| &mut self.values[position])
    }

    /// Every held cell, by column; within a column in no particular order.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (VoxelCoord, &T)> + '_ {
        self.held()
            .map(|(position, cell)| (cell, &self.values[position]))
    }

    /// Every held position and its cell, in the order of [`Self::iter`].
    pub(crate) fn held(&self) -> impl Iterator<Item = (usize, VoxelCoord)> + '_ {
        let slots: Box<dyn Iterator<Item = usize> + '_> = match &self.index {
            ColumnIndex::Box { .. } => Box::new(0..self.runs.len()),
            ColumnIndex::Sparse(slots) => Box::new(slots.values().copied()),
        };
        slots.flat_map(move |slot| {
            let run = self.runs[slot];
            (run.start..run.start + run.count).map(|position| (position, self.cells[position]))
        })
    }

    pub(crate) fn remove(&mut self, cell: VoxelCoord) -> Option<T> {
        let slot = self.index.slot((cell.x, cell.z))?;
        let run = &mut self.runs[slot];
        let last = run.start + run.count.checked_sub(1)?;
        let position = (run.start..=last).find(|&position| self.cells[position].y == cell.y)?;
        run.count -= 1;
        let value = self.values[position];
        self.cells.swap(position, last);
        self.values.swap(position, last);
        self.len -= 1;
        Some(value)
    }

    /// Adds cells the table does not hold. A column without room moves its
    /// run to the end, with room for twice its cells if it held some; a
    /// column outside a box widens the box, or the index turns sparse. As many cells as the table
    /// holds, or more, are laid out afresh with it.
    pub(crate) fn insert_new(&mut self, mut entries: Vec<(VoxelCoord, T)>) {
        if entries.len() >= self.len.max(1) {
            entries.extend(self.iter().map(|(cell, value)| (cell, *value)));
            entries.sort_by_key(|(cell, _)| (cell.x, cell.z, cell.y));
            *self = Self::laid_out(entries);
            return;
        }
        entries.sort_by_key(|(cell, _)| (cell.x, cell.z, cell.y));
        let added: Vec<(Column, &[(VoxelCoord, T)])> = entries
            .chunk_by(|(a, _), (b, _)| (a.x, a.z) == (b.x, b.z))
            .map(|run| ((run[0].0.x, run[0].0.z), run))
            .collect();
        let columns: Vec<Column> = added.iter().map(|&(column, _)| column).collect();
        self.admit_columns(&columns, entries.len());
        for (column, run) in added {
            let slot = self.index.slot(column).expect("every column was admitted");
            self.make_room(slot, run.len(), run[0]);
            let held = &mut self.runs[slot];
            for &(cell, value) in run {
                self.cells[held.start + held.count] = cell;
                self.values[held.start + held.count] = value;
                held.count += 1;
            }
            self.len += run.len();
        }
        if self.cells.len() - self.len > self.len.max(COMPACT_MINIMUM) {
            *self = Self::laid_out(self.iter().map(|(cell, value)| (cell, *value)).collect());
        }
    }

    /// Gives every one of `columns` a slot, for `adding` more cells. A box
    /// that lacks one is fitted to the columns that hold cells and these;
    /// runs keep their positions.
    fn admit_columns(&mut self, columns: &[Column], adding: usize) {
        if columns
            .iter()
            .all(|&column| self.index.slot(column).is_some())
        {
            return;
        }
        let held: Vec<(Column, Run)> = match &mut self.index {
            ColumnIndex::Sparse(slots) => {
                for &column in columns {
                    slots.entry(column).or_insert_with(|| {
                        self.runs.push(Run::default());
                        self.runs.len() - 1
                    });
                }
                return;
            }
            ColumnIndex::Box {
                min_x,
                min_z,
                depth,
                ..
            } => self
                .runs
                .iter()
                .enumerate()
                .filter(|(_, run)| run.count > 0)
                .map(|(slot, &run)| {
                    let (across, along) = (slot as u64 / *depth, slot as u64 % *depth);
                    let column = (
                        (*min_x as u64).wrapping_add(across) as i64,
                        (*min_z as u64).wrapping_add(along) as i64,
                    );
                    (column, run)
                })
                .collect(),
        };
        let every = || {
            held.iter()
                .map(|&(column, _)| column)
                .chain(columns.iter().copied())
        };
        let (min, max) = bounds(every());
        // Room beyond each side that grew, half the columns' extent along
        // it, so cells arriving one edge at a time widen the box rarely.
        let ColumnIndex::Box {
            min_x,
            min_z,
            width,
            depth,
        } = self.index
        else {
            unreachable!("a sparse index returned above")
        };
        let widened = |low: i64, high: i64, box_low: i64, extent: u64| {
            let (low, high, box_low) = (i128::from(low), i128::from(high), i128::from(box_low));
            let room = (high - low + 1) / 2;
            let low = if low < box_low { low - room } else { low };
            let high = if high >= box_low + i128::from(extent) {
                high + room
            } else {
                high
            };
            let clamp =
                |value: i128| value.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64;
            (clamp(low), clamp(high))
        };
        let (low_x, high_x) = widened(min.0, max.0, min_x, width);
        let (low_z, high_z) = widened(min.1, max.1, min_z, depth);
        let cells = self.len + adding;
        self.index = ColumnIndex::boxed((low_x, low_z), (high_x, high_z), cells)
            .or_else(|| ColumnIndex::boxed(min, max, cells))
            .unwrap_or_else(|| ColumnIndex::sparse(every()));
        self.runs = vec![Run::default(); self.index.slots()];
        for (column, run) in held {
            self.runs[self.index.slot(column).expect("a held column has a slot")] = run;
        }
    }

    /// Room in a column's run for `adding` more cells, moving the run to the
    /// end if need be; `filler` stands in any room left over.
    fn make_room(&mut self, slot: usize, adding: usize, filler: (VoxelCoord, T)) {
        let run = self.runs[slot];
        if run.capacity - run.count >= adding {
            return;
        }
        // A new column gets just its cells; a growing one twice as many.
        let capacity = (run.count + adding).max(run.count * 2);
        let start = self.cells.len();
        self.cells
            .extend_from_within(run.start..run.start + run.count);
        self.values
            .extend_from_within(run.start..run.start + run.count);
        self.cells.resize(start + capacity, filler.0);
        self.values.resize(start + capacity, filler.1);
        self.runs[slot] = Run {
            start,
            capacity,
            count: run.count,
        };
    }
}
