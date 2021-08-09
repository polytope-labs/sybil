use sc_consensus_pow::{PowAlgorithm, Error};
use sp_consensus_pow::Seal;
use sp_runtime::{traits::Block as BlockT, generic::BlockId};
use sp_core::{U256, H256};
use codec::{Encode, Decode};
use sha3::{Sha3_256, Digest};

#[derive(Clone)]
pub struct SybilPow;

#[derive(Encode, Decode)]
pub struct SybilSeal {
	nonce: H256,
	difficulty: U256,
}

#[derive(Encode, Decode)]
struct Compute<Hash> {
	pre_hash: Hash,
	nonce: H256,
	difficulty: U256,
}

impl<B: BlockT> PowAlgorithm<B> for SybilPow {
	type Difficulty = U256;

	fn difficulty(&self, _parent: B::Hash) -> Result<Self::Difficulty, Error<B>> {
		unimplemented!()
	}

	fn verify(
		&self,
		_parent: &BlockId<B>,
		pre_hash: &B::Hash,
		_pre_digest: Option<&[u8]>,
		seal: &Seal,
		difficulty: Self::Difficulty,
	) -> Result<bool, Error<B>> {
		let seal = match SybilSeal::decode(&mut &seal[..]) {
			Ok(seal) => seal,
			Err(_) => return Ok(false)
		};

		let compute  = Compute::<B::Hash> {
			nonce: seal.nonce,
			difficulty,
			pre_hash: *pre_hash
		};

		let mut hasher = Sha3_256::new();
		hasher.update(compute.encode());
		let work = U256::from(&*hasher.finalize());

		let (_, overflowed) = work.overflowing_mul(difficulty);

		if overflowed {
			return Ok(false)
		}

		Ok(true)
	}
}