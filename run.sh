#!/bin/sh


RUST_LOG="brontes=warn,brontes_inspect::mev_inspectors::cex_dex::markout=info"

cargo run --release --features arbitrum -- \
run \
--with-metrics \
--behind-tip 50 \
# VWAP window (tight, post-first)
--initial-pre 0.0005 \
--initial-post 0.004 \
--max-vwap-pre 1.0 \
--max-vwap-post 1.0 \
--vwap-scaling-diff 0.02 \
--vwap-time-step 0.0002 \
--weights-vwap true \
--weights-pre-vwap -0.00007 \
--weights-post-vwap -0.000017 \
# Optimistic window (tight, post-first)
--initial-op-pre 0.0005 \
--initial-op-post 0.004 \
--max-op-pre 0.08 \
--max-op-post 0.16 \
--optimistic-scaling-diff 0.02 \
--optimistic-time-step 0.0015 \
--weights-op true \
--weights-pre-op -0.00007 \
--weights-post-op -0.000017 \
# Misc
--quote-offset 0.0 \
--cex-dex-min-profit-usd 0.5 \
--cex-dex-known-min-profit-usd -1.0