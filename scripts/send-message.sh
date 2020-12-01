#!/bin/bash

case "$1" in
	remark)
		RUST_LOG=runtime=trace,substrate-relay=trace,bridge=trace \
		./target/debug/substrate-relay submit-millau-to-rialto-message \
			--millau-host localhost \
			--millau-port 20044 \
			--millau-signer //Dave \
			--rialto-signer //Dave \
			--lane 00000000 \
			--fee 100000000 \
			--origin Target \
			remark \
		;;
	transfer)
		RUST_LOG=runtime=trace,substrate-relay=trace,bridge=trace \
		./target/debug/substrate-relay submit-millau-to-rialto-message \
			--millau-host localhost \
			--millau-port 20044 \
			--millau-signer //Dave \
			--rialto-signer //Dave \
			--lane 00000000 \
			--fee 1000000000 \
			--origin Source \
			transfer \
			--amount 1000 \
			--recipient 5DZvVvd1udr61vL7Xks17TFQ4fi9NiagYLaBobnbPCP14ewA \
		;;
	*) exit 1;;
esac
