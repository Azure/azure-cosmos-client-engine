package main

import (
	"context"
	"fmt"
	"os"

	"github.com/Azure/azure-cosmos-client-engine/go/azcosmoscx"
	"github.com/Azure/azure-sdk-for-go/sdk/data/azcosmos"
)

func getenvOrDefault(key, def string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return def
}

func main() {
	endpoint := "https://localhost:8081"
	key := "C2y6yDjf5/R+ob0N8A7Cgv30VRDJIWEHLM+4QDU5DE2nQ9nDuVTqobD4b8mGGyPMbIZnqyMsEcaGQy67XIw/Jw=="
	databaseName := "SampleDB"
	containerName := "SampleContainer"

	var query string

	for i := 1; i < len(os.Args); i++ {
		key := os.Args[i]
		switch key {
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
		default:
			query = key
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

	azcosmoscx.EnableTracing()

	client, err := azcosmos.NewClientWithKey(endpoint, cred, &azcosmos.ClientOptions{
		QueryEngine: azcosmoscx.NewQueryEngine(),
	})
	if err != nil {
		panic(err)
	}

	container, err := client.NewContainer(databaseName, containerName)
	if err != nil {
		panic(err)
	}

	// Query for all items
	pager := container.NewQueryItemsPager(query, azcosmos.NewPartitionKey(), nil)

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
