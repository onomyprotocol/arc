package main

// NOTE: this is supposed to be an exact copy of module/cmd/gravity, except that app/consumer
// and cmd/consumer/cmd is used where applicable, and some minor name changes are needed.

import (
	"os"

	"github.com/cosmos/cosmos-sdk/server"
	"github.com/onomyprotocol/arc/module/cmd/consumer/cmd"
	_ "github.com/onomyprotocol/arc/module/config"
)

func main() {
	rootCmd, _ := cmd.NewRootCmd()
	if err := cmd.Execute(rootCmd); err != nil {
		switch e := err.(type) {
		case server.ErrorCode:
			os.Exit(e.Code)
		default:
			os.Exit(1)
		}
	}
}
