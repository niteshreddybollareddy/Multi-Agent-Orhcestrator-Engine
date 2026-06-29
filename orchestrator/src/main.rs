mod engine;
mod nsw_graph;
mod ring_buffer;
mod tensor;

use engine::{AeroEngine, AgentMessage};
use nsw_graph::NswGraph;
use ring_buffer::RingBuffer;
use std::sync::Arc;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting Striped SPSC Aero Engine (10M Ops/Sec HFT-Grade)...");

    // 1. Fetch available physical CPU cores
    let core_ids = core_affinity::get_core_ids().unwrap();
    if core_ids.len() < 2 {
        eprintln!("Error: This benchmark requires a multi-core CPU.");
        std::process::exit(1);
    }
    
    let consumer_core = core_ids[0];
    let producer_cores = &core_ids[1..];
    println!("Detected {} CPU cores. Consumer mapped to Core {}.", core_ids.len(), consumer_core.id);

    // 2. Setup Navigable Small World Graph
    let mut nsw_graph = NswGraph::new();
    for _ in 0..10_000 {
        let mut vector_data = vec![0.0; 128];
        for v in vector_data.iter_mut() {
            *v = rand::random::<f32>() * 2.0 - 1.0;
        }
        nsw_graph.nodes.push(vector_data);
        
        let num_nodes = nsw_graph.nodes.len();
        let mut edges = Vec::new();
        if num_nodes > 1 { edges.push(num_nodes - 2); }
        if num_nodes > 5 { edges.push(num_nodes - 6); }
        nsw_graph.edges.push(edges);
    }

    // 3. Setup STRIPED SPSC Ring Buffers (One per Producer)
    let num_producers = 10;
    let mut ring_buffers = Vec::new();
    for _ in 0..num_producers {
        ring_buffers.push(Arc::new(RingBuffer::new(65536)));
    }

    // 4. Setup AeroEngine
    let engine = AeroEngine::new(ring_buffers, nsw_graph);
    let engine_ref = Arc::new(engine);

    // 5. Spawn OS thread consumer (Pinned to Core 0)
    let _worker_handle = engine_ref.spawn_worker(consumer_core);

    // 6. Spawn Producer OS Threads (Pinned to remaining cores)
    let agents_per_producer = 1_000_000;
    let total_agents = num_producers * agents_per_producer; // 10,000,000 agents!
    
    println!("Starting burst of {} agents across {} striped producer queues...", total_agents, num_producers);
    let start_time = Instant::now();

    let mut send_threads = Vec::new();

    for thread_index in 0..num_producers {
        let engine_clone = Arc::clone(&engine_ref);
        let core = producer_cores[thread_index % producer_cores.len()];
        
        let handle = std::thread::spawn(move || {
            core_affinity::set_for_current(core);
            
            let start_id = thread_index * agents_per_producer;
            let end_id = start_id + agents_per_producer;

            let mut dummy_vector = vec![0.0; 128];
            for v in dummy_vector.iter_mut() {
                *v = rand::random::<f32>() * 2.0 - 1.0;
            }
            let shared_vector = Arc::new(dummy_vector);

            for i in start_id..end_id {
                let mut current_msg = AgentMessage {
                    agent_id: i,
                    vector_data: Arc::clone(&shared_vector),
                };

                // Push exclusively to THIS producer's dedicated queue (thread_index)
                loop {
                    match engine_clone.push(thread_index, current_msg) {
                        Ok(_) => break,
                        Err(returned_msg) => {
                            current_msg = returned_msg;
                            std::hint::spin_loop();
                        }
                    }
                }
            }
        });
        
        send_threads.push(handle);
    }

    for handle in send_threads {
        let _ = handle.join();
    }
    
    while engine_ref.get_processed_count() < total_agents {
        std::hint::spin_loop();
    }

    let elapsed = start_time.elapsed();
    println!("Orchestration loop completed for {} agents in {:?}", total_agents, elapsed);
    println!("Average latency per agent: {:?}", elapsed / total_agents as u32);
    println!("Throughput: {:.2} agents/sec", total_agents as f64 / elapsed.as_secs_f64());

    Ok(())
}
