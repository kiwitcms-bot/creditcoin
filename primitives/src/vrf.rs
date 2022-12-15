use num::rational::BigRational;
use num::ToPrimitive;
use parity_scale_codec::{Decode, Encode};
use sp_arithmetic::per_things::{PerThing, Perbill, Perquintill};
use sp_consensus_vrf::schnorrkel::SignatureError;
use sp_runtime_interface::pass_by::PassByCodec;
use sp_runtime_interface::runtime_interface;

#[derive(Encode, Decode, PassByCodec)]
pub struct Wrap(Perquintill);

impl From<Wrap> for f64 {
	fn from(w: Wrap) -> Self {
		w.0.to_sub_1_float()
	}
}

impl From<Perquintill> for Wrap {
	fn from(p: Perquintill) -> Self {
		Wrap(p)
	}
}

#[runtime_interface]
trait Pdf {
	fn probability_density_function(sample: Wrap, weight: Wrap) -> Wrap {
		let complement = 1f64 - sample.0.to_sub_1_float();
		let r: f64 = weight.into();
		let f: f64 = complement.powf(r);
		Perquintill::from_float(1f64 - f).into()
	}
}

pub mod sortition {
	use super::*;
	use schnorrkel::vrf::VRFInOut;
	use sp_consensus_vrf::schnorrkel::PublicKey;
	/// S is a hyperparameter representing participation rate.
	/// The higher the value, the higher the chances of being sampled.
	/// R is the prover's relative stake. the output is proportional to the stake.
	pub fn model(s: Perquintill, r: Perquintill) -> f64 {
		use pdf::probability_density_function;
		probability_density_function(s.into(), r.into()).into()
	}

	pub fn threshold(sample_size: Perquintill, stake: u128, total_stake: u128) -> u128 {
		let ratio = Perquintill::from_rational(stake, total_stake);
		let level = model(sample_size, ratio);
		let level = BigRational::from_float(level).expect("Model codomain [0,1)");
		let threshold = u128::MAX / level.denom() * level.numer();
		threshold.to_u128().expect("n * X, X ~ [0,1), n: u128; qed.")
	}

	pub fn is_selected(inout: &VRFInOut, threshold: u128) -> bool {
		u128::from_le_bytes(inout.make_bytes::<[u8; 16]>(b"creditcoin-vrf")) < threshold
	}

	pub fn prove_vrf(
		pubkey: PublicKey,
		pre_hash: H256,
		epoch: u64,
		task_id: H256,
		output: VRFOutput,
		proof: VRFProof,
	) -> Result<VRFInOut, SignatureError> {
		let transcript = make_transcript(transcript_data(pre_hash, epoch, task_id));
		pubkey
			.vrf_verify(transcript, &output, &proof)
			.map(|(inout, _proofbatchable)| inout)
	}

	#[cfg(test)]
	mod tests {}
}

// Convert fixed point to f64, Accuracy depends on [PerThing::Inner]
trait ToFloat: PerThing {
	fn to_sub_1_float(&self) -> f64
	where
		<Self as PerThing>::Inner: Into<u64>,
	{
		let c = self.deconstruct();
		c.into() as f64 / Self::ACCURACY.into() as f64
	}
}

impl ToFloat for Perquintill {}
impl ToFloat for Perbill {}

use sp_core::H256;
use sp_keystore::vrf::VRFTranscriptValue;
pub use sp_keystore::vrf::{make_transcript, VRFTranscriptData};

const ENGINE_ID: &[u8; 4] = b"COTS";

pub(super) fn transcript_data(pre_hash: H256, epoch: u64, task_id: H256) -> VRFTranscriptData {
	VRFTranscriptData {
		label: ENGINE_ID,
		items: vec![
			("epoch", VRFTranscriptValue::U64(epoch)),
			("task id", VRFTranscriptValue::Bytes(task_id.encode())),
			("pre hash", VRFTranscriptValue::Bytes(pre_hash.encode())),
		],
	}
}

use sp_consensus_vrf::schnorrkel::{VRFOutput, VRFProof};
use sp_core::crypto::KeyTypeId;
use sp_core::sr25519::Public;
#[cfg(feature = "std")]
use sp_externalities::ExternalitiesExt;
use sp_keystore::vrf::VRFSignature;
#[cfg(feature = "std")]
use sp_keystore::{KeystoreExt, SyncCryptoStore};
use tracing as log;

#[runtime_interface]
pub trait Vrf {
	fn generate_vrf(
		&mut self,
		key_type_id: KeyTypeId,
		pubkey: &Public,
		pre_hash: H256,
		epoch: u64,
		task_id: H256,
	) -> Option<(VRFOutput, VRFProof)> {
		let keystore = &***self
			.extension::<KeystoreExt>()
			.expect("No `keystore` associated for the current context!");
		let public_data = transcript_data(pre_hash, epoch, task_id);
		match SyncCryptoStore::sr25519_vrf_sign(keystore, key_type_id, pubkey, public_data) {
			Ok(Some(signature)) => {
				let VRFSignature { output, proof } = signature;
				Some((VRFOutput(output), VRFProof(proof)))
			},
			Ok(None) => {
				log::warn!(target = "VRF", "missing Public {pubkey} from {key_type_id:?}");
				None
			},
			Err(e) => {
				log::error!(target = "VRF", "TODO: VRF signing failed {e}!");
				None
			},
		}
	}
}

#[cfg(test)]
mod tests {
	use super::vrf::generate_vrf;
	use super::*;
	use runtime_utils::ExtBuilder;
	use sp_consensus_vrf::schnorrkel::PublicKey;
	use sp_core::blake2_256;

	#[test]
	fn distribution_works() {
		let r = Perquintill::from_float(0.000_001);
		let c = Perquintill::from_float(0.5);

		assert!(sortition::model(c, r) - 6.931469403334938544156E-7 < f64::EPSILON);
	}

	#[test]
	fn threshold_works() {
		let x = sortition::threshold(Perquintill::from_float(0.5), 1, 1);
		let y = u128::MAX / 2;
		assert_eq!(x, y)
	}

	#[test]
	fn sane_to_float() {
		use rand::Rng;
		use sp_arithmetic::Perbill;
		let mut rng = rand::thread_rng();
		for _ in 0..1000 {
			let r: f64 = rng.gen();

			let x = Perquintill::from_float(r).to_sub_1_float();
			assert!(r - x < f64::EPSILON, "{r} - {x} < ε");

			let x = Perbill::from_float(r).to_sub_1_float();
			assert!(r - x < f32::EPSILON.into(), "{r} - {x} < ε");
		}
	}

	struct PublicData {
		key_type_id: KeyTypeId,
		pre_hash: H256,
		epoch: u64,
		task_id: H256,
	}

	fn mocked_public_data() -> PublicData {
		let key_type_id = KeyTypeId(*b"gots");
		let pre_hash = blake2_256(b"predecessor hash").into();
		let epoch = 1u64;
		let task_id = blake2_256(b"task").into();
		PublicData { key_type_id, pre_hash, epoch, task_id }
	}

	fn add_random_key(key_type_id: KeyTypeId, builder: &ExtBuilder<()>) -> Public {
		builder
			.keystore
			.as_ref()
			.expect("A keystore")
			.sr25519_generate_new(key_type_id, None)
			.unwrap()
	}

	#[test]
	#[tracing_test::traced_test]
	fn generate_vrf_output() {
		let PublicData { key_type_id, pre_hash, epoch, task_id } = mocked_public_data();

		let builder = ExtBuilder::<()>::default().with_keystore();
		let pubkey = add_random_key(key_type_id, &builder);

		builder.build_sans_config().execute_with(|| {
			generate_vrf(key_type_id, &pubkey, pre_hash, epoch, task_id).unwrap();
		})
	}

	#[test]
	fn prove_vrf_output() {
		let PublicData { key_type_id, pre_hash, epoch, task_id } = mocked_public_data();

		let builder = ExtBuilder::<()>::default().with_keystore();
		let pubkey = add_random_key(key_type_id, &builder);

		builder.build_sans_config().execute_with(|| {
			let (output, proof) =
				generate_vrf(key_type_id, &pubkey, pre_hash, epoch, task_id).unwrap();

			sortition::prove_vrf(
				PublicKey::from_bytes(&pubkey.0).unwrap(),
				pre_hash,
				epoch,
				task_id,
				output,
				proof,
			)
			.unwrap();
		})
	}

	//inout from prove equals inout from generator
}
