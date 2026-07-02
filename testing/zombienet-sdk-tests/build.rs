// Exposes the polkadot-sdk revision pinned in the workspace `Cargo.lock` as the
// `POLKADOT_SDK_SHORT_HASH` env var, used to tag the default node images.
use std::{env, fs, path::Path};

fn main() {
	let lock = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../Cargo.lock");
	println!("cargo:rerun-if-changed={}", lock.display());

	let contents = fs::read_to_string(&lock).expect("read workspace Cargo.lock");
	let rev = contents
		.split("polkadot-sdk?branch=master#")
		.nth(1)
		.and_then(|s| s.get(..40))
		.filter(|s| s.bytes().all(|b| b.is_ascii_hexdigit()))
		.expect("polkadot-sdk revision not found in Cargo.lock");
	println!("cargo:rustc-env=POLKADOT_SDK_SHORT_HASH={}", &rev[..8]);
}
