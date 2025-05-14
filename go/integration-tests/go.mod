module github.com/Azure/azure-cosmos-client-engine/go/integration-tests

go 1.23.6

replace (
	github.com/Azure/azure-cosmos-client-engine/go/azcosmoscx => ../azcosmoscx
	github.com/Azure/azure-sdk-for-go/sdk/data/azcosmos => ../../../azure-sdk-for-go/sdk/data/azcosmos
)

require (
	github.com/Azure/azure-sdk-for-go/sdk/azcore v1.17.0
	github.com/stretchr/testify v1.10.0
)

require (
	github.com/Azure/azure-sdk-for-go v68.0.0+incompatible // indirect
	github.com/Azure/azure-sdk-for-go/sdk/internal v1.10.0 // indirect
	github.com/tidwall/gjson v1.18.0 // indirect
	github.com/tidwall/match v1.1.1 // indirect
	github.com/tidwall/pretty v1.2.1 // indirect
	github.com/tidwall/sjson v1.2.5 // indirect
	golang.org/x/net v0.37.0 // indirect
	golang.org/x/text v0.23.0 // indirect
)

require (
	github.com/Azure/azure-cosmos-client-engine/go/azcosmoscx v0.0.0-00010101000000-000000000000
	github.com/Azure/azure-sdk-for-go/sdk/data/azcosmos v1.3.0
	github.com/davecgh/go-spew v1.1.1 // indirect
	github.com/pmezard/go-difflib v1.0.0 // indirect
	github.com/rogpeppe/go-internal v1.14.1 // indirect
	github.com/wI2L/jsondiff v0.6.1
	gopkg.in/yaml.v3 v3.0.1 // indirect
)
