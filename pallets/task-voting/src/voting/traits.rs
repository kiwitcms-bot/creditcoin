use super::{Power, Summary};
use crate::{Config, Error, RoundOf};

pub trait QuorumMet<T: Config> {
	fn meets_quorum(task: &T::TaskId, votes: &RoundOf<T>) -> bool;
}

pub trait OnVoteConclusion<T: Config> {
	fn voting_concluded(task: &T::TaskId, summary: Summary<T::OutputId>, votes: &RoundOf<T>);
}

pub trait VoterPower<T: Config> {
	fn voting_power_of(task: &T::TaskId, voter: &T::AccountId) -> Result<Power, Error<T>>;
}
