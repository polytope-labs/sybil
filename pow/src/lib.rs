use codec::{Decode, Encode};
use sc_consensus_pow::{Error, PowAlgorithm};
use sha3::{Digest, Sha3_256};
use sp_api::ProvideRuntimeApi;
use sp_consensus_pow::{DifficultyApi, Seal};
use sp_core::{H256, U256};
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use std::sync::Arc;

pub struct SybilPow<C> {
	// the client pointer
	client: Arc<C>,
}

impl<C> Clone for SybilPow<C> {
	fn clone(&self) -> Self {
		Self::new(Arc::clone(&self.client))
	}
}

#[derive(Encode, Decode)]
pub struct SybilSeal {
	pub nonce: H256,
	pub difficulty: U256,
}

#[derive(Encode, Decode)]
pub struct Compute<Hash> {
	pub pre_hash: Hash,
	pub nonce: H256,
	pub difficulty: U256,
}

impl<C> SybilPow<C> {
	pub fn new(client: Arc<C>) -> Self {
		Self { client }
	}
}

impl<C, B> PowAlgorithm<B> for SybilPow<C>
where
	C: ProvideRuntimeApi<B>,
	C::Api: DifficultyApi<B, U256>,
	B: BlockT,
{
	type Difficulty = U256;

	fn difficulty(&self, parent: B::Hash) -> Result<Self::Difficulty, Error<B>> {
		Ok(
			self.client.runtime_api().difficulty(&BlockId::hash(parent))
				.map_err(|err| Error::Other(format!("Error fetching difficulty, {:?}", err)))?
		)
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
			Err(_) => return Ok(false),
		};

		let compute = Compute::<B::Hash> { nonce: seal.nonce, difficulty, pre_hash: *pre_hash };

		let hash = Sha3_256::digest(&compute.encode()[..]);
		let work = U256::from(&*hash);

		let (_, overflowed) = work.overflowing_mul(difficulty);

		if overflowed {
			return Ok(false);
		}

		Ok(true)
	}
}
