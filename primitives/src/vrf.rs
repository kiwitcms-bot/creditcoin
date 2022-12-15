use num::rational::BigRational;
use num::ToPrimitive;
use parity_scale_codec::{Decode, Encode};
use sp_arithmetic::per_things::{PerThing, Perbill, Perquintill};
use sp_runtime_interface::pass_by::Codec;
use sp_runtime_interface::pass_by::PassBy;
use sp_runtime_interface::runtime_interface;

#[derive(Encode, Decode)]
pub struct Wrap(Perquintill);

impl From<Wrap> for f64 {
	fn from(w: Wrap) -> Self {
		w.0.to_float()
	}
}

impl From<Perquintill> for Wrap {
	fn from(p: Perquintill) -> Self {
		Wrap(p)
	}
}

impl PassBy for Wrap {
	type PassBy = Codec<Self>;
}

#[runtime_interface]
trait Pdf {
	fn probability_density_function(sample: Wrap, weight: Wrap) -> Wrap {
		let complement = 1f64 - sample.0.to_float();
		let r: f64 = weight.into();
		let f: f64 = complement.powf(r);
		Perquintill::from_float(1f64 - f).into()
	}
}

pub mod sortition {
	use super::*;
	/// S is a hyperparameter representing participation rate.
	/// The higher the value, the higher the chances of being sampled.
	/// R is the prover's relative stake. the output is proportional to the stake.
	pub(super) fn model(s: Perquintill, r: Perquintill) -> f64 {
		use crate::vrf::pdf::probability_density_function;
		probability_density_function(s.into(), r.into()).into()
	}

	pub(super) fn threshold(sample_size: Perquintill, stake: u128, total_stake: u128) -> u128 {
		let ratio = Perquintill::from_rational(stake, total_stake);
		let level = model(sample_size, ratio);
		let level = BigRational::from_float(level).expect("Model codomain [0,1)");
		let threshold = u128::MAX / level.denom() * level.numer();
		threshold.to_u128().expect("n * X, X ~ [0,1), n: u128; qed.")
	}
}

// Convert fixed point to f64, Accuracy depends on [PerThing::Inner]
trait ToFloat: PerThing {
	fn to_float(&self) -> f64
	where
		<Self as PerThing>::Inner: Into<u64>,
	{
		let c = self.deconstruct();
		(c.into() as f64) / Self::ACCURACY.into() as f64
	}
}

impl ToFloat for Perquintill {}
impl ToFloat for Perbill {}

#[cfg(test)]
mod tests {
	use super::*;

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

	//secret keys computed well in advance from seed_r
	//pick the seed as the last blocks hash + salt; protected by PoW

	#[test]
	fn sane_to_float() {
		use rand::Rng;
		use sp_arithmetic::Perbill;
		let mut rng = rand::thread_rng();
		for _ in 0..1000 {
			let r: f64 = rng.gen();

			let x = Perquintill::from_float(r).to_float();
			assert!(r - x < f64::EPSILON, "{r} - {x} < ε");

			let x = Perbill::from_float(r).to_float();
			assert!(r - x < f32::EPSILON.into(), "{r} - {x} < ε");
		}
	}
}
