package main

import (
	"os"

	"github.com/cosmos/cosmos-sdk/server"
	"github.com/onomyprotocol/arc/module/bnb/cmd/gravity/cmd"
	_ "github.com/onomyprotocol/arc/module/bnb/config"
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
