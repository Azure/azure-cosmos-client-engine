module github.com/Azure/azure-cosmos-client-engine/go/azcosmoscx/benchmarks

go 1.24.0

require (
	github.com/Azure/azure-cosmos-client-engine/go/azcosmoscx v0.0.0-00010101000000-000000000000
	github.com/Azure/azure-sdk-for-go/sdk/data/azcosmos v1.5.0-beta.0
)

replace github.com/Azure/azure-cosmos-client-engine/go/azcosmoscx => ../
