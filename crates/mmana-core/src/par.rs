//! Data parallelism that degrades to plain loops on WebAssembly.
//!
//! Every parallel section in the engine computes independent values and keeps
//! their original order, so results are bit-identical to the sequential
//! program whatever the number of threads.

#[cfg(not(target_arch = "wasm32"))]
use rayon::prelude::*;

/// `(0..n).map(f).collect()`, evaluated in parallel.
pub(crate) fn map<R: Send>(n: usize, f: impl Fn(usize) -> R + Sync + Send) -> Vec<R> {
    #[cfg(not(target_arch = "wasm32"))]
    return (0..n).into_par_iter().map(f).collect();
    #[cfg(target_arch = "wasm32")]
    return (0..n).map(f).collect();
}

/// Calls `f(row, a_row, b_row)` for each `width`-long row of two matrices of
/// the same shape, in parallel.
pub(crate) fn rows_mut<T: Send>(
    a: &mut [T],
    b: &mut [T],
    width: usize,
    f: impl Fn(usize, &mut [T], &mut [T]) + Sync + Send,
) {
    if width == 0 {
        return;
    }
    #[cfg(not(target_arch = "wasm32"))]
    a.par_chunks_mut(width).zip(b.par_chunks_mut(width)).enumerate().for_each(|(r, (a, b))| f(r, a, b));
    #[cfg(target_arch = "wasm32")]
    a.chunks_mut(width).zip(b.chunks_mut(width)).enumerate().for_each(|(r, (a, b))| f(r, a, b));
}
