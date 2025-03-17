package main

import (
	"context"
	"fmt"
	"os"

	"github.com/Azure/azure-cosmos-client-engine/go/azcosmoscx"
	"github.com/Azure/azure-sdk-for-go/sdk/data/azcosmos"
	"github.com/Azure/azure-sdk-for-go/sdk/data/azcosmos/unstable/queryengine"
)

func getenvOrDefault(key, def string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return def
}

func executeQuery(container *azcosmos.ContainerClient, query string, queryEngine queryengine.QueryEngine) {
	// Query for all items
	options := &azcosmos.QueryOptions{
		UnstablePreviewQueryEngine: queryEngine,
	}
	pager := container.NewQueryItemsPager(query, azcosmos.NewPartitionKey(), options)

	for pager.More() {
		page, err := pager.NextPage(context.TODO())
		if err != nil {
			panic(err)
		}

		for _, item := range page.Items {
			fmt.Println(string(item))
		}
	}
}

func main() {
	endpoint := "https://localhost:8081"

	// Emulator key; not a secret!
	key := "C2y6yDjf5/R+ob0N8A7Cgv30VRDJIWEHLM+4QDU5DE2nQ9nDuVTqobD4b8mGGyPMbIZnqyMsEcaGQy67XIw/Jw=="

	databaseName := "SampleDB"
	containerName := "SampleContainer"
	useCosmosCX := false

	var query string

	for i := 1; i < len(os.Args); i++ {
		arg := os.Args[i]
		switch arg {
		case "--endpoint":
			endpoint = os.Args[i+1]
			i++
		case "--key":
			key = os.Args[i+1]
			i++
		case "--database":
			databaseName = os.Args[i+1]
			i++
		case "--container":
			containerName = os.Args[i+1]
			i++
		case "--use-cosmoscx":
			useCosmosCX = true
		default:
			query = arg
		}
	}

	if len(query) == 0 {
		fmt.Println("Usage: sample --endpoint ENDPOINT --key KEY --database DATABASE --container CONTAINER QUERY")
		os.Exit(1)
	}

	cred, err := azcosmos.NewKeyCredential(key)
	if err != nil {
		panic(err)
	}

	var queryEngine queryengine.QueryEngine
	if useCosmosCX {
		azcosmoscx.EnableTracing()
		queryEngine = azcosmoscx.NewQueryEngine()
	}

	client, err := azcosmos.NewClientWithKey(endpoint, cred, &azcosmos.ClientOptions{})
	if err != nil {
		panic(err)
	}

	container, err := client.NewContainer(databaseName, containerName)
	if err != nil {
		panic(err)
	}

	executeQuery(container, query, queryEngine)

	// Run leak checker
	doLeakCheck()
}
