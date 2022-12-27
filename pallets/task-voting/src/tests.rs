use crate::voting::Summary;
use crate::{Config, OnVoteConclusion, RoundOf};

impl<T: Config> OnVoteConclusion<T> for () {
	fn voting_concluded(
		_task: &<T as Config>::TaskId,
		_summary: Summary<T::OutputId>,
		_votes: &RoundOf<T>,
	) {
	}
}
