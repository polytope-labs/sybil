
use super::*;

#[allow(unused)]
use crate::Pallet as Template;
use frame_benchmarking::{benchmarks, impl_benchmark_test_suite, whitelisted_caller};
use frame_system::RawOrigin;
use frame_support::traits::Currency;

fn assert_last_event<T: Config>(generic_event: <T as Config>::Event) {
	frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

benchmarks! {
	set_reward {
		let reward: <T::Currency as Currency<T::AccountId>>::Balance = 100u32.into();
	}: _(RawOrigin::Root, reward)
	verify {
		assert_last_event::<T>(Event::RewardUpdated(reward).into());
	}
}
