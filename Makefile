build-protos:
	rust-script scripts/build-protos.rs
example-singlethreaded:
	cargo run --example singlethreaded --features=usb,tokio/rt,tokio/macros
example-multithreaded:
	cargo run --example multithreaded --features=usb,tokio/rt,tokio/macros,tokio/rt-multi-thread,multithreaded
example-btc-signtx:
	cargo run --example btc_signtx --features=usb,tokio/rt,tokio/macros
example-btc-psbt:
	cargo run --example btc_sign_psbt --features=usb,tokio/rt,tokio/macros
example-btc-miniscript:
	cargo run --example btc_miniscript --features=usb,tokio/rt,tokio/macros
example-eth:
	cargo run --example eth --features=usb,tokio/rt,tokio/macros,rlp
example-cardano:
	cargo run --example cardano --features=usb,tokio/rt,tokio/macros
wasm:
	wasm-pack build --release --features=wasm
	cp webhid.js pkg/
	jq '.files += ["webhid.js"]' pkg/package.json > tmp.json && mv tmp.json pkg/package.json
	cp README-npm.md pkg/README.md
	du -sh pkg/bitbox_api_bg.wasm
run-sandbox:
	cd sandbox && npm i && npm run dev
build-sandbox:
	cd sandbox && npm i && npm run build
