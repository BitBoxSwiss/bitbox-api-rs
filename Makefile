example:
	cargo run --example connect --features=usb,tokio/rt,tokio/macros
example-btc-signtx:
	cargo run --example btc_signtx --features=usb,tokio/rt,tokio/macros
example-btc-psbt:
	cargo run --example btc_sign_psbt --features=usb,tokio/rt,tokio/macros,bitcoin
example-btc-miniscript:
	cargo run --example btc_miniscript --features=usb,tokio/rt,tokio/macros,bitcoin
wasm:
	wasm-pack build --release --features=wasm
	cp webhid.js pkg/
	du -sh pkg/bitbox_api_bg.wasm
run-sandbox:
	cd sandbox && npm i && npm run dev
build-sandbox:
	cd sandbox && npm i && npm run build
