module github.com/onomyprotocol/arc/module/eth

go 1.16

require (
	github.com/cosmos/btcutil v1.0.4
	github.com/cosmos/cosmos-sdk v0.45.11
	github.com/cosmos/ibc-go/v2 v2.0.2
	github.com/ethereum/go-ethereum v1.10.10
	github.com/gogo/protobuf v1.3.3
	github.com/golang/protobuf v1.5.2
	github.com/gorilla/mux v1.8.0
	github.com/grpc-ecosystem/grpc-gateway v1.16.0
	github.com/pkg/errors v0.9.1
	github.com/rakyll/statik v0.1.7
	github.com/regen-network/cosmos-proto v0.3.1
	github.com/spf13/cast v1.5.0
	github.com/spf13/cobra v1.6.0
	github.com/spf13/viper v1.13.0
	github.com/stretchr/testify v1.8.0
	github.com/tendermint/tendermint v0.34.23
	github.com/tendermint/tm-db v0.6.6
	google.golang.org/genproto v0.0.0-20221014213838-99cd37c6964a
	google.golang.org/grpc v1.50.1
)

replace (
	// github.com/cosmos/cosmos-sdk => github.com/onomyprotocol/onomy-sdk v0.44.6-0.20220526083940-424017863a62
	github.com/gogo/grpc => google.golang.org/grpc v1.33.2
	github.com/gogo/protobuf => github.com/regen-network/protobuf v1.3.3-alpha.regen.1
	google.golang.org/grpc => google.golang.org/grpc v1.33.2
)
