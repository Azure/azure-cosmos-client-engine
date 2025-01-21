package engine

import "github.com/Azure/azure-cosmos-client-engine/go/engine/internal/native"

func Version() string {
	return native.CosmosCX_Version()
}
