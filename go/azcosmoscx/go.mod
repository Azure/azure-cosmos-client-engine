module github.com/Azure/azure-cosmos-client-engine/go/azcosmoscx

go 1.23.3

replace github.com/Azure/azure-sdk-for-go/sdk/data/azcosmos => ../../../azure-sdk-for-go/sdk/data/azcosmos

require (
	github.com/Azure/azure-sdk-for-go/sdk/data/azcosmos v1.3.1-beta.1
	github.com/stretchr/testify v1.10.0
)

require (
	github.com/davecgh/go-spew v1.1.1 // indirect
	github.com/kr/pretty v0.3.1 // indirect
	github.com/pmezard/go-difflib v1.0.0 // indirect
	github.com/rogpeppe/go-internal v1.12.0 // indirect
	gopkg.in/check.v1 v1.0.0-20201130134442-10cb98267c6c // indirect
	gopkg.in/yaml.v3 v3.0.1 // indirect
)
