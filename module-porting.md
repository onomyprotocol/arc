# Overview

That instructions describe the steps to port the original arc module to a different chain to make it possible to use
multiple module in the same chain.

This doc is written for the `bnb` chain integration, if you use another, replace the `bnb` to `your-chain` in that doc.

# Steps

## Module

* Replace the `cosmos-gravity-bridge` to `arc` in all files.

* Replace the `github.com/onomyprotocol/arc/bnb/module/x/gravity` to `github.com/onomyprotocol/arc/bnb/module/x/arcbnb`
  in all files.

* Rename the package/folder `module/x/gravity` to `module/x/arcbnb`.

* Fix the import conflicts in the `module/app/app.go`, by replacing `gravity.` to `arcbnb.` and adding new
  import `"github.com/onomyprotocol/arc/module/x/arcbnb"` if not added automatically.

* Change the module name in the `module/x/arcbnb/types/keys.go`  `ModuleName = "gravity"` to `ModuleName = "arcbnb"`.

* Rename the folder `module/proto/gravity` to `module/proto/arcbnb`.

* Replace string `package gravity.v1;` to `package arcbnb.v1;` in folder `module/proto/arcbnb`.

* Replace string `gravity/` to `arcbnb/` in folder in folder `module/proto/arcbnb`.

* Open the `module` in the CLI and execute `make proto-gen`.

* Run the tests if the `module`, open the `moduel` in the CLI and execute `make test`.

* Update `proto-check-breaking` goal in the `Makefile`, updated the `#branch=main` to `#branch=bnb`

## Solidity

* Replace `/custom/gravity` to `/custom/arcbnb` in the `solidity` folder.

## Orchestrator

* Replace `gbt` to `arcbnbbt` in `orchestrator` folder (!!! Case sensitive !!!).

* Replace `GBT` to `ARCBNBBT` in the `.github/workflows` (!!! Case sensitive !!!).

* Rename `orchestrator/gbt` to `orchestrator/arcbnbbt`.

* Replace `proto/gravity` to `proto/arcbnb` in the `orchestrator` folder.

* Open `orchestrator/proto_build` in CLI and run `cargo run` to generate new rust proto client.

* Add `gravity_proto/src/prost/arcbnb.v1.rs` to git.

* Replace `prost/gravity.v1.rs` to `prost/arcbnb.v1.rs` in the `orchestrator/gravity_proto/lib.rs`

* Replace `gravity.v1` to `arcbnb.v1` in the all `orchestrator` files.

* Replace `subspace: "gravity"` to `subspace: "arcbnb"` in the all `orchestrator` files.

* Open `orchestrator` in the CLI and run `cargo fmt --all` to format the code.

* Open `orchestrator` in the CLI and run `cargo build --all && cargo test --all` to check the unit tests.

## Run tests

Open `tests` in the CLI and execute `USE_LOCAL_ARTIFACTS=1 bash all-up-test.sh HAPPY_PATH_V2` is the happy path passes
you can run all tests `USE_LOCAL_ARTIFACTS=1 bash run-all-tests.sh`.

## Update the workflows

* Update the `main` branch to `bnb` in all `.github` workflows.

* Create PR to the `bnb` branch. (The `breakage` test will fail first time, that is expected).