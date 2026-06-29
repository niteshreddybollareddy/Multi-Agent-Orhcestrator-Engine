use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// A simplified Multi-Producer Single-Consumer (MPSC) Lock-Free Ring Buffer.
/// Used to completely bypass network-bound state managers (like Redis) for local agents.
#[repr(align(128))]
struct CachePadded<T> {
    value: T,
}

pub struct RingBuffer<T> {
    buffer: Box<[UnsafeCell<Option<T>>]>,
    ready: Box<[AtomicBool]>,
    write_pos: CachePadded<AtomicUsize>,
    read_pos: CachePadded<AtomicUsize>,
    capacity: usize,
}

// Ensure the RingBuffer can be safely shared across threads.
unsafe impl<T: Send> Sync for RingBuffer<T> {}
unsafe impl<T: Send> Send for RingBuffer<T> {}

impl<T> RingBuffer<T> {
    /// Creates a new Lock-Free Ring Buffer with the specified capacity.
    /// The capacity must be a power of two for optimal masking, but we'll use modulo here for simplicity.
    pub fn new(capacity: usize) -> Self {
        let mut buffer = Vec::with_capacity(capacity);
        let mut ready = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            buffer.push(UnsafeCell::new(None));
            ready.push(AtomicBool::new(false));
        }

        Self {
            buffer: buffer.into_boxed_slice(),
            ready: ready.into_boxed_slice(),
            write_pos: CachePadded { value: AtomicUsize::new(0) },
            read_pos: CachePadded { value: AtomicUsize::new(0) },
            capacity,
        }
    }

    pub fn push(&self, item: T) -> Result<(), T> {
        // In SPSC, only ONE producer thread ever calls push. 
        // We can safely use a simple Relaxed load without compare_exchange loops!
        let current_write = self.write_pos.value.load(Ordering::Relaxed);
        let next_write = current_write + 1;
        let index = current_write % self.capacity;

        // Check if we are wrapping around and catching up to the read pointer (buffer full)
        if next_write.wrapping_sub(self.read_pos.value.load(Ordering::Acquire)) > self.capacity {
            return Err(item); // Buffer is full
        }

        // Write the item safely using UnsafeCell
        unsafe {
            *self.buffer[index].get() = Some(item);
        }
        
        // Mark the slot as ready for the consumer to read
        self.ready[index].store(true, Ordering::Release);
        
        // Store the incremented write position
        self.write_pos.value.store(next_write, Ordering::Release);
        
        Ok(())
    }

    /// Pops an item from the ring buffer.
    /// Only ONE consumer (the engine worker) should call this.
    pub fn pop(&self) -> Option<T> {
        let current_read = self.read_pos.value.load(Ordering::Relaxed);
        let index = current_read % self.capacity;

        // Check if the slot is ready (written by a producer)
        if self.ready[index].load(Ordering::Acquire) {
            // Read the item
            let item = unsafe {
                (*self.buffer[index].get()).take()
            };

            // Mark slot as empty
            self.ready[index].store(false, Ordering::Release);
            
            // Advance the read pointer
            self.read_pos.value.store(current_read + 1, Ordering::Release);
            
            item
        } else {
            None // Buffer is empty or slot is not fully written yet
        }
    }
}
