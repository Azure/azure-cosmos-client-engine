module github.com/Azure/azure-cosmos-client-engine/go/sample

go 1.23.6

replace (
	github.com/Azure/azure-cosmos-client-engine/go/azcosmoscx => ../azcosmoscx
	github.com/Azure/azure-sdk-for-go/sdk/data/azcosmos => ../../../azure-sdk-for-go/sdk/data/azcosmos
)

require (
	github.com/Azure/azure-cosmos-client-engine/go/azcosmoscx v0.0.0
	github.com/Azure/azure-sdk-for-go/sdk/data/azcosmos v1.2.1
)

require (
	github.com/Azure/azure-sdk-for-go v68.0.0+incompatible // indirect
	github.com/Azure/azure-sdk-for-go/sdk/azcore v1.16.0 // indirect
	github.com/Azure/azure-sdk-for-go/sdk/internal v1.10.0 // indirect
	golang.org/x/net v0.38.0 // indirect
	golang.org/x/text v0.23.0 // indirect
)
