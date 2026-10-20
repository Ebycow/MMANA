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
    #[error("wire {wire}: custom tapering DM values must be between 1 and {max}")]
    InvalidWireSegmentation { wire: usize, max: i32 },
    #[error("{what} pulse '{name}' does not exist (valid pulses: 1-{max})")]
    PulseOutOfRange { what: &'static str, name: String, max: usize },
}
