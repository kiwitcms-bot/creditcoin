//use crate::mock::*;
use crate::{Config, OnVoteConclusion, RoundOf, VoteResultSummary};

impl<T: Config> OnVoteConclusion<T> for () {
	fn voting_concluded(
		_task: &<T as Config>::TaskId,
		_summary: VoteResultSummary<T::DataId>,
		_votes: &RoundOf<T>,
	) {
	}
}
