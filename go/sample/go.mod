module github.com/Azure/azure-cosmos-client-engine/go/sample

go 1.23.5

replace github.com/Azure/azure-cosmos-client-engine/go/azcosmoscx => ../azcosmoscx

require (
	github.com/Azure/azure-cosmos-client-engine/go/azcosmoscx v0.0.0
	github.com/Azure/azure-sdk-for-go/sdk/data/azcosmos v1.5.0-beta.1
)

require (
	github.com/Azure/azure-sdk-for-go/sdk/azcore v1.18.1 // indirect
	github.com/Azure/azure-sdk-for-go/sdk/internal v1.11.1 // indirect
	golang.org/x/net v0.42.0 // indirect
	golang.org/x/text v0.27.0 // indirect
)
