// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

package main

import (
	"context"
	"crypto/tls"
	"encoding/json"
	"fmt"
	"net/http"
	"os"
	"strings"

	"github.com/Azure/azure-cosmos-client-engine/go/azcosmoscx"
	"github.com/Azure/azure-sdk-for-go/sdk/azcore"
	"github.com/Azure/azure-sdk-for-go/sdk/azcore/log"
	"github.com/Azure/azure-sdk-for-go/sdk/azidentity"
	"github.com/Azure/azure-sdk-for-go/sdk/data/azcosmos"
	"github.com/Azure/azure-sdk-for-go/sdk/data/azcosmos/queryengine"
)

func getenvOrDefault(key, def string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return def
}

func loadQueryParameters(filePath string) ([]azcosmos.QueryParameter, error) {
	if filePath == "" {
		return nil, nil
	}

	data, err := os.ReadFile(filePath)
	if err != nil {
		return nil, fmt.Errorf("failed to read parameters file: %w", err)
	}

	var paramMap map[string]interface{}
	if err := json.Unmarshal(data, &paramMap); err != nil {
		return nil, fmt.Errorf("failed to parse parameters JSON: %w", err)
	}

	var parameters []azcosmos.QueryParameter
	for name, value := range paramMap {
		parameters = append(parameters, azcosmos.QueryParameter{
			Name:  name,
			Value: value,
		})
	}

	return parameters, nil
}

func executeQuery(container *azcosmos.ContainerClient, query string, queryEngine queryengine.QueryEngine, parameters []azcosmos.QueryParameter) {
	// Query for all items
	pager := container.NewQueryItemsPager(query, azcosmos.NewPartitionKey(), &azcosmos.QueryOptions{
		QueryEngine:     queryEngine,
		QueryParameters: parameters,
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
	key := ""
	databaseName := "SampleDB"
	containerName := "SampleContainer"
	parametersFile := ""

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
		case "--parameters-file":
			parametersFile = os.Args[i+1]
			i++
		default:
			query = arg
		}
	}

	if len(query) == 0 {
		fmt.Println("Usage: sample --endpoint ENDPOINT --key KEY --database DATABASE --container CONTAINER [--parameters-file PARAMETERS_FILE] QUERY")
		os.Exit(1)
	}

	azcosmoscx.EnableTracing()

	// Create a client with a custom transport that skips TLS verification
	// Since there's a self-signed certificate in the emulator, we need to skip verification
	transport := &http.Client{Transport: &http.Transport{
		TLSClientConfig: &tls.Config{InsecureSkipVerify: true},
	}}

	options := &azcosmos.ClientOptions{ClientOptions: azcore.ClientOptions{
		Transport: transport,
	}}

	log.SetListener(func(event log.Event, message string) {
		fmt.Printf("%s: %s\n", event, message)
	})

	var client *azcosmos.Client
	if len(key) == 0 {
		if endpoint == "https://localhost:8081" {
			cred, err := azcosmos.NewKeyCredential("C2y6yDjf5/R+ob0N8A7Cgv30VRDJIWEHLM+4QDU5DE2nQ9nDuVTqobD4b8mGGyPMbIZnqyMsEcaGQy67XIw/Jw==")
			if err != nil {
				panic(err)
			}
			client, err = azcosmos.NewClientWithKey(endpoint, cred, options)
			if err != nil {
				panic(err)
			}
		} else {
			cred, err := azidentity.NewAzureCLICredential(nil)
			if err != nil {
				panic(err)
			}
			client, err = azcosmos.NewClient(endpoint, cred, options)
			if err != nil {
				panic(err)
			}
		}
	} else {
		cred, err := azcosmos.NewKeyCredential(key)
		if err != nil {
			panic(err)
		}
		client, err = azcosmos.NewClientWithKey(endpoint, cred, options)
		if err != nil {
			panic(err)
		}
	}

	container, err := client.NewContainer(databaseName, containerName)
	if err != nil {
		panic(err)
	}

	if strings.HasPrefix(query, "@") {
		fileName := query[1:]
		data, err := os.ReadFile(fileName)
		if err != nil {
			panic(err)
		}
		query = string(data)
	}

	// Load query parameters if specified
	parameters, err := loadQueryParameters(parametersFile)
	if err != nil {
		panic(err)
	}

	executeQuery(container, query, azcosmoscx.NewQueryEngine(), parameters)

	// Run leak checker
	doLeakCheck()

	fmt.Println()
	fmt.Println()
	fmt.Println()
}
