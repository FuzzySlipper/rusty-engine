#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Program<Predicate, Operation> {
    Sequence {
        steps: Vec<Self>,
    },
    When {
        predicate: Predicate,
        then_program: Box<Self>,
        otherwise_program: Option<Box<Self>>,
    },
    Operation(Operation),
}
