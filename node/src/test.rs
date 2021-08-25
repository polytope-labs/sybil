#[cfg(test)]
mod test {
	use codec::Encode;
	use rand::{self, RngCore};
	use sc_consensus_pow::MiningMetadata;
	use sha3::Digest;
	use sp_core::{self, Blake2Hasher, Hasher};
	use std::{
		self,
		sync::{Arc, Mutex},
		time::{Duration, Instant},
	};
	struct MockWorker;

	impl MockWorker {
		pub fn metadata(&self) -> Option<MiningMetadata<sp_core::H256, sp_core::U256>> {
			let mut rand = rand::thread_rng();
			let mut pre_hash_bytes = sp_core::H512::default();
			let mut best_hash_bytes = sp_core::H512::default();
			let mut pre_runtime = vec![0; 512];

			rand.fill_bytes(&mut pre_hash_bytes[..]);
			rand.fill_bytes(&mut best_hash_bytes[..]);
			rand.fill_bytes(&mut pre_runtime[..]);

			Some(MiningMetadata {
				best_hash: Blake2Hasher::hash(&best_hash_bytes[..]),

				pre_hash: Blake2Hasher::hash(&pre_hash_bytes[..]),

				pre_runtime: Some(pre_runtime),

				difficulty: 1_000_000.into(),
			})
		}
	}
	#[test]
	fn benchmark_sealing_task() {
		let worker = Arc::new(Mutex::new(MockWorker {}));

		let worker_clone = worker.clone();

		let handle = std::thread::spawn(move || {
			let start = std::time::Instant::now();
			let mut rand = rand::thread_rng();
            let mut results = String::new();

			loop {
				let elapsed_time = Instant::now().duration_since(start);

				if elapsed_time.as_secs() >= 300 {
                    let number = rand::random::<u8>();
                    let path = std::path::PathBuf::from(format!("test_benchmarks/sybil_single_thread_test_{}", number));
                    
                    let _ = std::fs::write(path, results.as_bytes());
					break;
				}

				if let Ok(worker) = worker_clone.lock() {
					let metadata = worker.metadata().unwrap();
					let start_work = Instant::now();
					loop {
                        let elapsed_time = Instant::now().duration_since(start_work);

                        if elapsed_time.as_secs() >= 60 {
                            break;
                        }

						let mut nonce = sp_core::H256::default();
						rand.fill_bytes(&mut nonce[..]);

						let compute = sybil_pow::Compute {
							pre_hash: metadata.pre_hash,
							difficulty: metadata.difficulty,
							nonce,
						};

						let work =
							sp_core::U256::from(&*sha3::Sha3_256::digest(&compute.encode()[..]));

						let (_, overflowed) = work.overflowing_mul(metadata.difficulty);

						if !overflowed {
							let _seal =
								sybil_pow::SybilSeal { nonce, difficulty: metadata.difficulty };

                            
							println!(
								"Computed correct seal in {}",
								Instant::now().duration_since(start_work).as_secs_f32()
							);

                            results.push_str(&format!(
								"Computed correct seal in {}\n",
								Instant::now().duration_since(start_work).as_secs_f32()
							)[..]);

							break;
						}
					}
				}
			}
		});

		handle.join().unwrap();
	}
}
