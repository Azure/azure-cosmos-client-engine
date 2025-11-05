module github.com/Azure/azure-cosmos-client-engine/go/sample

go 1.23.5

replace (
	github.com/Azure/azure-cosmos-client-engine/go/azcosmoscx => ../azcosmoscx
)

require (
	github.com/Azure/azure-cosmos-client-engine/go/azcosmoscx v0.0.0
	github.com/Azure/azure-sdk-for-go/sdk/data/azcosmos v1.4.0
)

require (
	github.com/Azure/azure-sdk-for-go v68.0.0+incompatible // indirect
	github.com/Azure/azure-sdk-for-go/sdk/azcore v1.17.1 // indirect
	github.com/Azure/azure-sdk-for-go/sdk/internal v1.11.1 // indirect
	golang.org/x/net v0.38.0 // indirect
	golang.org/x/text v0.23.0 // indirect
)
