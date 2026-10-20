//! A segment count far over the pulse limit must fail on the limit, not
//! allocate for that count first. Its own test binary, so the counting
//! allocator sees only this calculation.

use mmana_core::{CalcError, CalcOptions, Segmentation, calculate, maa};
use std::alloc::{GlobalAlloc, Layout, System};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Records the largest single allocation.
struct Largest;

static LARGEST: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Largest {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        LARGEST.fetch_max(layout.size(), Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        LARGEST.fetch_max(layout.size(), Ordering::Relaxed);
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        LARGEST.fetch_max(new_size, Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Largest = Largest;

#[test]
fn huge_segment_count_fails_without_allocating_for_it() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/maa/DP20.MAA");
    let mut project = maa::parse_bytes(&std::fs::read(path).unwrap()).project;
    project.antenna.wires[0].segmentation = Segmentation::Fixed { count: 10_000_000 };
    LARGEST.store(0, Ordering::Relaxed);
    let result = calculate(&project, &CalcOptions::default());
    assert_eq!(result.map(|_| ()), Err(CalcError::TooManyPulses { max: 4096 }));
    // sized by the segment count, the coordinate arrays alone are 80 MB each
    let largest = LARGEST.load(Ordering::Relaxed);
    assert!(largest < 1 << 20, "largest allocation {largest} bytes");
}
