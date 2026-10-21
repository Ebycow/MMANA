//! A segment count or stacking far over the limits must fail on the limit,
//! not allocate for that size first. Its own test binary, so the counting
//! allocator sees only these calculations; they take turns.
//!
//! These inputs are what a broken limit would turn into gigabytes and hours,
//! so the allocator also refuses any single allocation over [`CAP`]: the test
//! binary then stops at once ("memory allocation of … bytes failed") instead
//! of filling memory and swap. Inputs that big belong in this file only.

use mmana_core::{
    AxisRange, CalcError, CalcOptions, MAX_FIELD_POINTS, MAX_SWEEP_POINTS, NearFieldKind, NearFieldSpec, Project,
    Segmentation, calculate, maa, pulse_layout,
};
use std::alloc::{GlobalAlloc, Layout, System};
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Records the largest single allocation and refuses any over [`CAP`].
struct Largest;

static LARGEST: AtomicUsize = AtomicUsize::new(0);

/// Far above what any test here needs (they assert under 1 MB).
const CAP: usize = 256 << 20;

unsafe impl GlobalAlloc for Largest {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        LARGEST.fetch_max(layout.size(), Ordering::Relaxed);
        if layout.size() > CAP {
            return std::ptr::null_mut();
        }
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        LARGEST.fetch_max(layout.size(), Ordering::Relaxed);
        if layout.size() > CAP {
            return std::ptr::null_mut();
        }
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        LARGEST.fetch_max(new_size, Ordering::Relaxed);
        if new_size > CAP {
            return std::ptr::null_mut();
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Largest = Largest;

/// Held while measuring, so one test's allocations do not count for another.
static MEASURING: Mutex<()> = Mutex::new(());

fn dp20() -> Project {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/maa/DP20.MAA");
    maa::parse_bytes(&std::fs::read(path).unwrap()).project
}

#[test]
fn huge_segment_count_fails_without_allocating_for_it() {
    let mut project = dp20();
    project.antenna.wires[0].segmentation = Segmentation::Fixed { count: 10_000_000 };
    let _measuring = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    LARGEST.store(0, Ordering::Relaxed);
    let result = calculate(&project, &CalcOptions::default());
    assert_eq!(result.map(|_| ()), Err(CalcError::TooManyPulses { max: 4096 }));
    // sized by the segment count, the coordinate arrays alone are 80 MB each
    let largest = LARGEST.load(Ordering::Relaxed);
    assert!(largest < 1 << 20, "largest allocation {largest} bytes");
}

#[test]
fn tapering_a_wire_too_long_to_count_fails_on_the_limit() {
    // the segment count saturates at i32::MAX; one more must not wrap it
    for segmentation in [Segmentation::TaperStart, Segmentation::TaperEnd] {
        let mut project = dp20();
        let wire = &mut project.antenna.wires[0];
        wire.start.y = -1e12;
        wire.segmentation = segmentation;
        let _measuring = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
        let result = calculate(&project, &CalcOptions::default());
        assert_eq!(result.map(|_| ()), Err(CalcError::TooManyPulses { max: 4096 }));
    }
}

#[test]
fn huge_stack_fails_without_copying_the_wires() {
    // across, and across and up (counted as their product)
    for (horizontal, vertical) in [(100_000, 1), (1000, 1000)] {
        let mut project = dp20();
        project.antenna.stack.horizontal = horizontal;
        project.antenna.stack.vertical = vertical;
        let opts = CalcOptions { max_pulses: 1, ..CalcOptions::default() };
        let _measuring = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
        LARGEST.store(0, Ordering::Relaxed);
        let too_many = Err(CalcError::TooManySegmentWires { max: 4096 });
        assert_eq!(calculate(&project, &opts).map(|_| ()), too_many);
        assert_eq!(pulse_layout(&project, &opts).map(|_| ()), too_many);
        // the copies of the one wire alone are 9 MB
        let largest = LARGEST.load(Ordering::Relaxed);
        assert!(largest < 1 << 20, "{horizontal} × {vertical}: largest allocation {largest} bytes");
    }
}

#[test]
fn huge_field_scans_fail_without_allocating_for_them() {
    let project = dp20();
    let solved = mmana_core::solve(&project, &CalcOptions::default()).unwrap();
    let axis = |count| AxisRange { start: 0.0, step: 1.0, count };
    let spec = NearFieldSpec {
        kind: NearFieldKind::Electric,
        power_w: 100.0,
        x: axis(1 << 21),
        y: axis(1 << 21),
        z: axis(1 << 22),
    };
    let _measuring = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    LARGEST.store(0, Ordering::Relaxed);
    let max = Err(CalcError::TooManyFieldPoints { max: MAX_FIELD_POINTS });
    // 100 000² wrapped to 1 410 065 408 directions in i32: 68 GB of samples
    assert_eq!(solved.far_field((0.0, 1.0, 100_000), (0.0, 1.0, 100_000)).map(|_| ()), max);
    assert_eq!(solved.near_field(&spec).map(|_| ()), max);
    let largest = LARGEST.load(Ordering::Relaxed);
    assert!(largest < 1 << 20, "largest allocation {largest} bytes");
}

#[test]
fn huge_sweep_fails_without_allocating_for_it() {
    let project = dp20();
    let _measuring = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    LARGEST.store(0, Ordering::Relaxed);
    // the frequency list alone would be 8 × usize::MAX bytes
    let result = mmana_core::sweep(&project, &CalcOptions::default(), 14.0, 14.1, usize::MAX);
    assert_eq!(result.map(|_| ()), Err(CalcError::InvalidSweepPoints { max: MAX_SWEEP_POINTS }));
    let largest = LARGEST.load(Ordering::Relaxed);
    assert!(largest < 1 << 20, "largest allocation {largest} bytes");
}
