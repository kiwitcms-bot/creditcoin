#![cfg_attr(not(feature = "std"), no_std)]

use core::marker::PhantomData;
use frame_support::BoundedBTreeMap;
pub use pallet::*;
use parity_scale_codec::MaxEncodedLen;
#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod benchmarking;
mod votes;
use votes::{Data as VotingData, Power as VotingPower, Summary as VoteResultSummary, Votes};

#[allow(clippy::unnecessary_cast)]
pub mod weights;

pub type RoundOf<T> =
	Votes<<T as frame_system::Config>::AccountId, <T as Config>::DataId, <T as Config>::MaxVoters>;

pub trait QuorumMet<T: Config> {
	fn meets_quorum(task: &T::TaskId, votes: &RoundOf<T>) -> bool;
}

pub trait OnVoteConclusion<T: Config> {
	fn voting_concluded(
		task: &T::TaskId,
		summary: VoteResultSummary<T::DataId>,
		votes: &RoundOf<T>,
	);
}

pub trait VoterPower<T: Config> {
	fn voting_power_of(task: &T::TaskId, voter: &T::AccountId) -> Result<VotingPower, Error<T>>;
}

pub struct VotingProviderStrategy<P, Q>(PhantomData<(P, Q)>);

impl<T: Config, P: VoterPower<T>, Q> VoterPower<T> for VotingProviderStrategy<P, Q> {
	fn voting_power_of(task: &T::TaskId, voter: &T::AccountId) -> Result<VotingPower, Error<T>> {
		P::voting_power_of(task, voter)
	}
}

impl<T: Config, P, Q: QuorumMet<T>> QuorumMet<T> for VotingProviderStrategy<P, Q> {
	fn meets_quorum(task: &T::TaskId, votes: &RoundOf<T>) -> bool {
		Q::meets_quorum(task, votes)
	}
}

pub struct AtLeastOneVote;

impl<T: Config> QuorumMet<T> for AtLeastOneVote {
	fn meets_quorum(_task: &T::TaskId, votes: &RoundOf<T>) -> bool {
		votes.iter().any(|(_, vote)| vote.total_voting_power > 0)
	}
}

pub struct UniformVoterPower;

impl<T: Config> VoterPower<T> for UniformVoterPower {
	fn voting_power_of(_task: &T::TaskId, _voter: &T::AccountId) -> Result<VotingPower, Error<T>> {
		Ok(1)
	}
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{pallet_prelude::*, traits::Get, Identity, Parameter};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type TaskId: Parameter + Ord + MaxEncodedLen;

		type DataId: Parameter + Ord + MaxEncodedLen;

		type OnVoteConclusion: OnVoteConclusion<Self>;

		type VotingProvider: VoterPower<Self> + QuorumMet<Self>;

		#[pallet::constant]
		type MaxVoters: Get<u32>;

		// type WeightInfo: WeightInfo;
	}

	pub trait WeightInfo {
		fn on_finalize() -> Weight;
		fn on_initialize() -> Weight;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::event]
	//#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {}

	#[pallet::error]
	pub enum Error<T> {
		VoteInProgress,
		TooManyVoters,
		UnknownTask,
		UnknownVoter,
		DuplicateVoter,
		NoWinner,
	}

	#[pallet::storage]
	pub type Rounds<T: Config> = StorageMap<_, Identity, T::TaskId, RoundOf<T>>;

	impl<T: Config> Pallet<T> {
		pub fn submit_vote(
			task_id: T::TaskId,
			data: T::DataId,
			voter: T::AccountId,
		) -> Result<(), Error<T>> {
			let mut votes = Rounds::<T>::get(&task_id).ok_or(Error::UnknownTask)?;

			let power = T::VotingProvider::voting_power_of(&task_id, &voter)?;

			if let Some(vote) = votes.votes.get_mut(&data) {
				vote.add_voter(voter, power)?;
			} else {
				let mut vote = VotingData::new();
				vote.add_voter(voter, power)?;
				votes.votes.try_insert(data, vote).map_err(|_| Error::TooManyVoters)?;
			}

			Rounds::<T>::insert(task_id, votes);
			Ok(())
		}

		pub fn try_conclude_voting(task_id: T::TaskId) -> Result<(), Error<T>> {
			let votes = Rounds::<T>::get(&task_id).ok_or(Error::UnknownTask)?;

			if T::VotingProvider::meets_quorum(&task_id, &votes) {
				let summary = votes.tally_votes()?;
				T::OnVoteConclusion::voting_concluded(&task_id, summary, &votes);
				Rounds::<T>::remove(&task_id);
			}

			Ok(())
		}

		pub fn start_task_voting(
			task_id: T::TaskId,
			initial_vote: Option<(T::AccountId, T::DataId)>,
		) -> Result<(), Error<T>> {
			ensure!(!Rounds::<T>::contains_key(&task_id), Error::VoteInProgress);
			let mut votes = BoundedBTreeMap::new();
			let vote_total = if let Some((voter, data)) = initial_vote {
				let power = T::VotingProvider::voting_power_of(&task_id, &voter)?;
				let mut vote = VotingData::new();
				vote.add_voter(voter, power)?;
				votes.try_insert(data, vote).map_err(|_| Error::TooManyVoters)?;
				power
			} else {
				0
			};

			let votes = Votes { votes, vote_total };
			Rounds::<T>::insert(task_id, votes);
			Ok(())
		}
	}
}
