#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Program<Predicate, Selector, Operation> {
    Sequence {
        steps: Vec<Self>,
    },
    When {
        predicate: Predicate,
        then_program: Box<Self>,
        otherwise_program: Option<Box<Self>>,
    },
    ForEach {
        selector: Selector,
        maximum: u16,
        body: Box<Self>,
    },
    Operation(Operation),
}
