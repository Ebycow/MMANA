use thiserror::Error;

/// Reasons a calculation cannot run. Messages are user-facing.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum CalcError {
    #[error("frequency must be a positive number (got {0})")]
    InvalidFrequency(f64),
    #[error("the antenna has no wires")]
    NoWires,
    #[error("the antenna has no sources")]
    NoSources,
    #[error("every source has a voltage of 0")]
    NoExcitation,
    #[error("wire {wire} has zero length")]
    ZeroLengthWire { wire: usize },
    #[error("wire {wire}: no combined-wire definition with R = {radius}")]
    CombinedWireNotFound { wire: usize, radius: f64 },
    #[error("too many segment wires (maximum {max})")]
    TooManySegmentWires { max: usize },
    #[error("too many pulses (maximum {max}); reduce segmentation")]
    TooManyPulses { max: usize },
    #[error("the model has no pulses (every wire is a single unconnected segment)")]
    NoPulses,
    #[error("auto-segmentation: {0}")]
    InvalidAutoSegmentation(&'static str),
    #[error("wire {wire}: the segment count must be at least 1 (got {count})")]
    InvalidSegmentCount { wire: usize, count: i32 },
    #[error("wire {wire}: custom tapering DM values must be between 1 and {max}")]
    InvalidWireSegmentation { wire: usize, max: i32 },
    #[error("{what} pulse '{name}' does not exist (valid pulses: 1-{max})")]
    PulseOutOfRange { what: &'static str, name: String, max: usize },
    #[error("sources {first} and {second} are on the same pulse ({pulse})")]
    DuplicateSource { first: usize, second: usize, pulse: usize },
    #[error("wires {first} and {second} lie exactly on top of each other")]
    OverlappingWires { first: usize, second: usize },
    #[error("sweep point {index} does not exist (the sweep has {points} points)")]
    SweepPointOutOfRange { index: usize, points: usize },
    #[error("the calculation has no finite solution (the impedance matrix is singular)")]
    NoFiniteSolution,
    #[error("{field} must be {rule} (got {value})")]
    OutOfRange { field: String, rule: &'static str, value: f64 },
    #[error("load {load}: {reason}")]
    InvalidLoad { load: usize, reason: &'static str },
    #[error("a real ground needs at least one ground medium")]
    NoGroundMedia,
    #[error("the input power is not positive ({power} W), so the near field cannot be scaled to a power")]
    NonPositivePower { power: f64 },
    #[error("too many field points (maximum {max})")]
    TooManyFieldPoints { max: usize },
    #[error("a sweep has 1 to {max} points")]
    InvalidSweepPoints { max: usize },
}
