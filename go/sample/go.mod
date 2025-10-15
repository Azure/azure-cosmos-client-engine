module github.com/Azure/azure-cosmos-client-engine/go/sample

go 1.23.5

// replace (
// 	github.com/Azure/azure-cosmos-client-engine/go/azcosmoscx => ../azcosmoscx
// 	github.com/Azure/azure-sdk-for-go/sdk/data/azcosmos => github.com/Azure/azure-sdk-for-go/sdk/data/azcosmos v1.4.1-0.20251015181701-ad122df3a8ea
// )

require (
	github.com/Azure/azure-cosmos-client-engine/go/azcosmoscx v0.0.6
	github.com/Azure/azure-sdk-for-go/sdk/azcore v1.19.1
	github.com/Azure/azure-sdk-for-go/sdk/azidentity v1.13.0
	github.com/Azure/azure-sdk-for-go/sdk/data/azcosmos v1.5.0-beta.0
)

require (
	github.com/Azure/azure-sdk-for-go v68.0.0+incompatible // indirect
	github.com/Azure/azure-sdk-for-go/sdk/internal v1.11.2 // indirect
	github.com/AzureAD/microsoft-authentication-library-for-go v1.5.0 // indirect
	github.com/golang-jwt/jwt/v5 v5.3.0 // indirect
	github.com/google/uuid v1.6.0 // indirect
	github.com/kylelemons/godebug v1.1.0 // indirect
	github.com/pkg/browser v0.0.0-20240102092130-5ac0b6a4141c // indirect
	golang.org/x/crypto v0.41.0 // indirect
	golang.org/x/net v0.43.0 // indirect
	golang.org/x/sys v0.35.0 // indirect
	golang.org/x/text v0.28.0 // indirect
)
