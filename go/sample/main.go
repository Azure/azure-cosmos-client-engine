// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

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
	pager := container.NewQueryItemsPager(query, azcosmos.NewPartitionKey(), &azcosmos.QueryOptions{
		UnstablePreviewQueryEngine: queryEngine,
	})

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
	key := "C2y6yDjf5/R+ob0N8A7Cgv30VRDJIWEHLM+4QDU5DE2nQ9nDuVTqobD4b8mGGyPMbIZnqyMsEcaGQy67XIw/Jw=="
	databaseName := "SampleDB"
	containerName := "SampleContainer"

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

	azcosmoscx.EnableTracing()

	client, err := azcosmos.NewClientWithKey(endpoint, cred, nil)
	if err != nil {
		panic(err)
	}

	container, err := client.NewContainer(databaseName, containerName)
	if err != nil {
		panic(err)
	}

	executeQuery(container, query, azcosmoscx.NewQueryEngine())

	// Run leak checker
	doLeakCheck()

	fmt.Println()
	fmt.Println()
	fmt.Println()
}
