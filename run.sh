#!/bin/sh


RUST_LOG="brontes=warn,brontes_inspect::mev_inspectors::cex_dex::markout=info"

cargo run --release --features arbitrum -- \
run \
--with-metrics \
--behind-tip 50 \
--initial-pre 0.001 \
--initial-post 0.001 \
--max-vwap-pre 1.0 \
--max-vwap-post 1.0 \
--vwap-scaling-diff 0.0063 \
--vwap-time-step 0.0002 \
--weights-vwap true \
--weights-pre-vwap -0.000024 \
--weights-post-vwap -0.0000096 \
--initial-op-pre 0.001 \
--initial-op-post 0.0063 \
--max-op-pre 0.1042 \
--max-op-post 0.2083 \
--optimistic-scaling-diff 0.0042 \
--optimistic-time-step 0.0021 \
--weights-op true \
--weights-pre-op -0.0000144 \
--weights-post-op -0.00000576 \
--quote-offset 0.0 \
--cex-dex-min-profit-usd 1.0 \
--cex-dex-known-min-profit-usd -1.0