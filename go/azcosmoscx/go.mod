module github.com/Azure/azure-cosmos-client-engine/go/azcosmoscx

go 1.23.3

replace github.com/Azure/azure-sdk-for-go/sdk/data/azcosmos => ../../../azure-sdk-for-go/sdk/data/azcosmos

require (
	github.com/Azure/azure-sdk-for-go/sdk/data/azcosmos v1.2.1
	github.com/stretchr/testify v1.10.0
)

require (
	github.com/Azure/azure-sdk-for-go v68.0.0+incompatible // indirect
	github.com/Azure/azure-sdk-for-go/sdk/azcore v1.16.0 // indirect
	github.com/Azure/azure-sdk-for-go/sdk/internal v1.10.0 // indirect
	github.com/davecgh/go-spew v1.1.1 // indirect
	github.com/pmezard/go-difflib v1.0.0 // indirect
	golang.org/x/net v0.38.0 // indirect
	golang.org/x/text v0.23.0 // indirect
	gopkg.in/yaml.v3 v3.0.1 // indirect
)
