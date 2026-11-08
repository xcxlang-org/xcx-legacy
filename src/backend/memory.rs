use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

pub(crate) trait Managed: 'static {
    fn trace(&self, _visit: &mut dyn FnMut(i64)) {}
}

impl Managed for Vec<u8> {}
impl Managed for serde_json::Value {}

#[derive(Clone, Copy, Default)]
pub(crate) struct Statistics {
    pub live: usize,
    pub peak: usize,
    pub collections: usize,
}

#[derive(Default)]
struct Heap {
    objects: HashMap<i64, Box<dyn Managed>>,
    frames: Vec<HashMap<i64, i64>>,
    allocations: usize,
    bytes: usize,
    statistics: Statistics,
}

thread_local! {
    static HEAP: RefCell<Heap> = RefCell::new(Heap::default());
    static LAST_STATISTICS: RefCell<Statistics> = RefCell::new(Statistics::default());
}

pub(crate) fn allocate<T: Managed>(value: T) -> i64 {
    let mut value = Box::new(value);
    let pointer = (&mut *value as *mut T) as i64;
    register(pointer, value, std::mem::size_of::<T>());
    pointer
}

pub(crate) fn allocate_bytes(bytes: Vec<u8>) -> i64 {
    let size = bytes.capacity();
    let pointer = bytes.as_ptr() as i64;
    if bytes.is_empty() {
        return pointer;
    }
    register(pointer, Box::new(bytes), size);
    pointer
}

fn register(pointer: i64, value: Box<dyn Managed>, bytes: usize) {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        assert!(heap.objects.insert(pointer, value).is_none());
        heap.allocations += 1;
        heap.bytes += bytes;
        heap.statistics.live = heap.objects.len();
        heap.statistics.peak = heap.statistics.peak.max(heap.objects.len());
    });
}

pub(crate) fn finish() {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        heap.objects.clear();
        heap.frames.clear();
        heap.statistics.live = 0;
        LAST_STATISTICS.with(|last| *last.borrow_mut() = heap.statistics);
        *heap = Heap::default();
    });
}

pub(crate) fn collect(force: bool) {
    let external_roots = super::tables::memory_roots();
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        if !force && heap.allocations < 256 && heap.bytes < 1_048_576 {
            return;
        }
        let mut pending = external_roots;
        for frame in &heap.frames {
            pending.extend(frame.values().copied());
        }
        let mut marked = HashSet::new();
        while let Some(pointer) = pending.pop() {
            if !marked.insert(pointer) {
                continue;
            }
            if let Some(object) = heap.objects.get(&pointer) {
                object.trace(&mut |child| pending.push(child));
            }
        }
        heap.objects.retain(|pointer, _| marked.contains(pointer));
        heap.allocations = 0;
        heap.bytes = 0;
        heap.statistics.live = heap.objects.len();
        heap.statistics.collections += 1;
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_frame_enter() {
    HEAP.with(|heap| heap.borrow_mut().frames.push(HashMap::new()));
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_frame_leave() {
    HEAP.with(|heap| {
        heap.borrow_mut().frames.pop().expect("generated calls have a runtime frame");
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_root_set(slot: i64, pointer: i64) {
    HEAP.with(|heap| {
        heap.borrow_mut().frames.last_mut()
            .expect("generated roots belong to a runtime frame")
            .insert(slot, pointer);
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn xcx_collect() {
    collect(false);
}

#[cfg(test)]
pub(crate) fn last_statistics() -> Statistics {
    LAST_STATISTICS.with(|last| *last.borrow())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    struct Tracked {
        drops: Rc<Cell<usize>>,
        child: Option<i64>,
    }

    impl Managed for Tracked {
        fn trace(&self, visit: &mut dyn FnMut(i64)) {
            if let Some(child) = self.child {
                visit(child);
            }
        }
    }

    impl Drop for Tracked {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }

    #[test]
    fn collection_keeps_reachable_children_and_releases_replaced_roots() {
        super::super::runtime::xcx_runtime_finish();
        let drops = Rc::new(Cell::new(0));
        xcx_frame_enter();
        let child = allocate(Tracked { drops: drops.clone(), child: None });
        let parent = allocate(Tracked { drops: drops.clone(), child: Some(child) });
        allocate(Tracked { drops: drops.clone(), child: None });
        xcx_root_set(0, parent);
        collect(true);
        assert_eq!(drops.get(), 1);
        xcx_root_set(0, 0);
        collect(true);
        assert_eq!(drops.get(), 3);
        xcx_frame_leave();
        super::super::runtime::xcx_runtime_finish();
    }

    #[test]
    fn repeated_collections_reclaim_objects_and_finish_releases_the_heap() {
        super::super::runtime::xcx_runtime_finish();
        xcx_frame_enter();
        for index in 0..20_000 {
            let pointer = allocate_bytes(format!("item-{index}").into_bytes());
            xcx_root_set(0, pointer);
            xcx_collect();
        }
        xcx_frame_leave();
        super::super::runtime::xcx_runtime_finish();
        let statistics = last_statistics();
        assert_eq!(statistics.live, 0);
        assert!(statistics.collections >= 50, "{} collections", statistics.collections);
        assert!(statistics.peak < 512, "{} simultaneous objects", statistics.peak);
    }
}
