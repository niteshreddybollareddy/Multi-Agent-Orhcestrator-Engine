use crate::nsw_graph::NswGraph;
use crate::ring_buffer::RingBuffer;
use core_affinity::CoreId;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

pub struct AgentMessage {
    pub agent_id: usize,
    pub vector_data: Arc<Vec<f32>>,
}

pub struct AeroEngine {
    ring_buffers: Vec<Arc<RingBuffer<AgentMessage>>>,
    nsw_graph: Arc<NswGraph>,
    processed_count: Arc<AtomicUsize>,
}

impl AeroEngine {
    pub fn new(ring_buffers: Vec<Arc<RingBuffer<AgentMessage>>>, nsw_graph: NswGraph) -> Self {
        Self {
            ring_buffers,
            nsw_graph: Arc::new(nsw_graph),
            processed_count: Arc::new(AtomicUsize::new(0)),
        }
    }
    
    pub fn get_processed_count(&self) -> usize {
        self.processed_count.load(Ordering::Acquire)
    }

    pub fn push(&self, queue_index: usize, msg: AgentMessage) -> Result<(), AgentMessage> {
        self.ring_buffers[queue_index].push(msg)
    }

    /// Spawns a background worker OS thread that round-robin polls 
    /// a SPECIFIC SUBSET of the SPSC lock-free ring buffers.
    /// This enables Multi-Consumer scaling to blast past 10M ops/sec!
    pub fn spawn_worker(&self, core_id: CoreId, assigned_queues: Vec<usize>) -> JoinHandle<()> {
        let ring_buffers = self.ring_buffers.clone();
        let nsw_graph = Arc::clone(&self.nsw_graph);
        let processed_count = Arc::clone(&self.processed_count);

        std::thread::spawn(move || {
            core_affinity::set_for_current(core_id);

            let mut local_processed = 0;
            let num_assigned = assigned_queues.len();
            if num_assigned == 0 {
                return; // Nothing to do
            }
            
            let mut current_idx = 0;

            loop {
                let queue_id = assigned_queues[current_idx];
                
                // Round-Robin poll our assigned queues
                if let Some(msg) = ring_buffers[queue_id].pop() {
                    // Fast graph search (Math executed in parallel across consumers!)
                    let _results = nsw_graph.search(&msg.vector_data, 5);
                    
                    local_processed += 1;
                    if local_processed % 1000 == 0 {
                        processed_count.fetch_add(1000, Ordering::Release);
                    }
                }
                
                current_idx = (current_idx + 1) % num_assigned;
                if current_idx == 0 {
                    std::hint::spin_loop();
                }
            }
        })
    }
}
