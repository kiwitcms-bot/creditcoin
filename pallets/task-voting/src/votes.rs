use super::{Config, Error};
use frame_support::{traits::Get, BoundedBTreeMap, BoundedBTreeSet};
use scale_info::TypeInfo;
use sp_runtime::codec::{Decode, Encode, MaxEncodedLen};

pub type Power = u64;

#[derive(Encode, Decode, MaxEncodedLen, TypeInfo)]
#[scale_info(skip_type_params(Max))]
pub struct Votes<AccountId, Id, Max> {
	pub votes: BoundedBTreeMap<Id, Data<AccountId, Max>, Max>,
	pub vote_total: Power,
}

impl<AccountId: Ord, MaxVoters: Get<u32>> Data<AccountId, MaxVoters> {
	pub fn add_voter<T: Config>(&mut self, voter: AccountId, power: Power) -> Result<(), Error<T>> {
		if !self.voters.try_insert(voter).map_err(|_| Error::TooManyVoters)? {
			return Err(Error::DuplicateVoter);
		}

		self.total_voting_power += power;

		Ok(())
	}

	pub fn new() -> Self {
		Self { total_voting_power: 0, voters: BoundedBTreeSet::new() }
	}
}

#[derive(Encode, Decode, MaxEncodedLen, TypeInfo)]
#[scale_info(skip_type_params(Max))]
pub struct Data<AccountId, Max> {
	pub total_voting_power: Power,
	pub voters: BoundedBTreeSet<AccountId, Max>,
}

#[derive(Encode, Decode, MaxEncodedLen, TypeInfo)]
pub struct Summary<DataId> {
	vote_total: Power,
	winning_vote_total: Power,
	winning_data: DataId,
	runner_up_total: Power,
}

impl<AccountId, DataId: Ord, MaxVoters> Votes<AccountId, DataId, MaxVoters> {
	pub fn iter(
		&self,
	) -> sp_std::collections::btree_map::Iter<'_, DataId, Data<AccountId, MaxVoters>> {
		self.votes.iter()
	}
}

impl<AccountId, DataId: Ord + Clone, MaxVoters> Votes<AccountId, DataId, MaxVoters> {
	pub fn tally_votes<T: Config>(&self) -> Result<Summary<DataId>, Error<T>> {
		let mut best_data = None;
		let mut best_power = 0;

		let mut second_best_power = 0;
		for (data, vote) in self.iter() {
			if vote.total_voting_power > best_power {
				second_best_power = best_power;
				best_power = vote.total_voting_power;
				best_data = Some(data);
			} else if vote.total_voting_power > second_best_power {
				second_best_power = vote.total_voting_power;
			}
		}
		let best_data = best_data.ok_or(Error::NoWinner)?;

		let summary = Summary {
			vote_total: self.vote_total,
			winning_vote_total: best_power,
			winning_data: (*best_data).clone(),
			runner_up_total: second_best_power,
		};

		Ok(summary)
	}
}
