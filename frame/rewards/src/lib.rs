pub use pallet::*;


#[frame_support::pallet]
pub mod pallet {
	use frame_support::{dispatch::DispatchResult, pallet_prelude::*, Hooks, traits::Currency};
	use frame_system::pallet_prelude::*;
	use sp_runtime::DigestItem;
	use sp_consensus_pow::POW_ENGINE_ID;

	#[pallet::config]
	pub trait Config: frame_system::Config + balances::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		// TODO: instead, store reward as a storage item.
		type Reward: Get<<Self as balances::Config>::Balance>;

		/// concrete currency implementataion
		type Currency: Currency<<Self as frame_system::Config>::AccountId>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::event]
	#[pallet::metadata(T::AccountId = "AccountId")]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A block author  has just been rewarded.
		/// [AccountId]
		AuthorRewarded(T::AccountId)
	}


	#[pallet::hooks]
	impl<T: Config> Hooks for Pallet<T> {
		fn on_initialize(_n: BlockNumber) -> Weight {
			let author_bytes = frame_system::Pallet::<T>::digest()
				.unwrap()
				.logs
				.iter()
				.find_map(|item| {
					match item {
						DigestItem::PreRuntime(POW_ENGINE_ID, author) => Some(author),
						_ => None,
					}
				});
			let account_id = T::AccountId::decode(&mut &author_bytes[..])
				.unwrap();
			T::Currency::deposit_creating(account_id, T::Reward::get());
			Self::deposit_event(Event::AuthorRewarded(account_id));
			0
		}
	}
}
