use crate::pallet::WeightInfo;
use crate::types::Task;
use crate::Config;
use frame_support::weights::Weight;
use pallet_offchain_task_scheduler::tasks::TaskScheduler;
use pallet_offchain_task_scheduler::tasks::TaskV2;
use sp_runtime::traits::UniqueSaturatedInto;

pub(crate) fn migrate<T: Config>() -> Weight {
	let mut n = 0u32;
	for (i, (k1, _, v)) in crate::PendingTasks::<T>::drain().enumerate() {
		n = i.unique_saturated_into();
		let id: T::Hash = match &v {
			Task::CollectCoins(pending) => TaskV2::<T>::to_id(pending),
			Task::VerifyTransfer(pending) => TaskV2::<T>::to_id(pending),
		};

		T::TaskScheduler::insert(&k1, &id, v);
	}
	crate::weights::WeightInfo::<T>::migration_v7(n)
}

#[cfg(feature = "try-runtime")]
pub(crate) fn pre_upgrade<T: Config>() -> Result<(), &'static str> {
	Ok(())
}

#[cfg(feature = "try-runtime")]
pub(crate) fn post_upgrade<T: Config>() -> Result<(), &'static str> {
	ensure!(
		StorageVersion::get::<crate::Pallet<T>>() == 7,
		"expected storage version to be 7 after migrations complete"
	);

	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::helpers::extensions::IntoBounded;
	use crate::mock::ExtBuilder;
	use crate::mock::Test;
	use crate::ocw::tests::make_unverified_transfer;
	use crate::tests::TestInfo;
	use crate::types;
	use crate::CollectedCoinsId;
	use crate::{TransferId, TransferKind};

	#[test]
	fn migrate_pending_tasks() {
		ExtBuilder::default().build_and_execute(|| {
			let pending = types::UnverifiedCollectedCoins {
				to: [0u8; 256].into_bounded(),
				tx_id: [0u8; 256].into_bounded(),
				contract: Default::default(),
			};
			let pending_id = TaskV2::<Test>::to_id(&pending);

			crate::PendingTasks::<Test>::insert(
				1u64,
				crate::TaskId::from(CollectedCoinsId::from(pending_id)),
				Task::from(pending.clone()),
			);

			let test_info = TestInfo::new_defaults();
			let (deal_order_id, deal_order) = test_info.create_deal_order();
			let (_, transfer) = test_info.make_transfer(
				&test_info.lender,
				&test_info.borrower,
				deal_order.terms.amount,
				&deal_order_id,
				"0xfafafa",
				None::<TransferKind>,
			);

			let unverified = make_unverified_transfer(transfer);
			let unverified_id = TaskV2::<Test>::to_id(&unverified);

			crate::PendingTasks::<Test>::insert(
				2u64,
				crate::TaskId::from(TransferId::new::<Test>(
					&unverified.transfer.blockchain,
					&unverified.transfer.tx_id,
				)),
				Task::from(unverified),
			);

			migrate::<Test>();

			let migrated_pending = {
				if let Task::CollectCoins(pending) =
					pallet_offchain_task_scheduler::pallet::PendingTasks::<Test>::get(1, pending_id)
						.unwrap()
				{
					pending
				} else {
					unreachable!()
				}
			};
			assert_eq!(pending, migrated_pending);

			let migrated_unverified = {
				if let Task::VerifyTransfer(unverified) =
					pallet_offchain_task_scheduler::pallet::PendingTasks::<Test>::get(2, unverified_id)
						.unwrap()
				{
					pending
				} else {
					unreachable!()
				}
			};
			assert_eq!(unverified, migrated_unverified);
		});
	}
}
