#!/bin/bash
npx ts-node \
contract-deployer.ts \
--cosmos-node="http://localhost:26657" \
--eth-node="http://localhost:8545" \
--eth-privkey="0x163F5F0F9A621D72FEDD85FFCA3D08D131AB4E812181E0D30FFD1C885D20AAC7" \
--contract=Gravity.json \
--test-mode=true \
--wnom-address="0x0F23c3f0C77582a5dB7fB3D61097B619982fb32f"