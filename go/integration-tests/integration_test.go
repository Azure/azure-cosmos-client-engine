// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

package integrationtests

import (
	"context"
	"crypto/rand"
	"crypto/tls"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"net/http"
	"os"
	"path"
	"strings"
	"testing"

	"github.com/Azure/azure-cosmos-client-engine/go/azcosmoscx"
	"github.com/Azure/azure-sdk-for-go/sdk/azcore"
	"github.com/Azure/azure-sdk-for-go/sdk/data/azcosmos"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
	"github.com/wI2L/jsondiff"
)

type TestData struct {
	ContainerProperties azcosmos.ContainerProperties `json:"containerProperties"`
	Data                []json.RawMessage            `json:"data"`
}

type QuerySet struct {
	Name     string      `json:"name"`
	TestData string      `json:"testData"`
	Queries  []QuerySpec `json:"queries"`
}

type QuerySpec struct {
	Name string `json:"name"`
	Text string `json:"query"`
}

type QueryContext struct {
	Query     QuerySet
	TestData  TestData
	UniqueId  string
	Directory string
}

func LoadQueryContext(context context.Context, queryPath string) (queryContext QueryContext, err error) {
	queryDir := path.Dir(queryPath)

	// Read the integration test baseline file
	queryFile, err := os.Open(queryPath)
	if err != nil {
		return QueryContext{}, err
	}

	defer queryFile.Close()
	var querySpec QuerySet
	err = json.NewDecoder(queryFile).Decode(&querySpec)
	if err != nil {
		return QueryContext{}, err
	}

	uniqueBytes := make([]byte, 4)
	_, err = rand.Read(uniqueBytes)
	if err != nil {
		return QueryContext{}, err
	}
	encoded := base64.RawURLEncoding.EncodeToString(uniqueBytes)
	uniqueId := fmt.Sprintf("it_%s_%s", querySpec.Name, encoded)

	testData, err := loadTestData(resolvePath(queryDir, querySpec.TestData), uniqueId)
	if err != nil {
		return QueryContext{}, err
	}

	queryResultDir := path.Join(queryDir, querySpec.Name)

	return QueryContext{querySpec, testData, uniqueId, queryResultDir}, nil
}

func (queryContext *QueryContext) RunWithTestResources(context context.Context, endpoint, key string, fn func(context context.Context, client *azcosmos.Client, database *azcosmos.DatabaseClient, container *azcosmos.ContainerClient)) error {
	client, err := createClient(endpoint, key)
	if err != nil {
		return err
	}

	throughputProperties := azcosmos.NewManualThroughputProperties(40000)
	dbResponse, err := client.CreateDatabase(context, azcosmos.DatabaseProperties{
		ID: queryContext.UniqueId,
	}, &azcosmos.CreateDatabaseOptions{
		ThroughputProperties: &throughputProperties,
	})
	if err != nil {
		return err
	}

	database, err := client.NewDatabase(dbResponse.DatabaseProperties.ID)
	if err != nil {
		return err
	}
	defer database.Delete(context, nil)

	containerResponse, err := database.CreateContainer(context, queryContext.TestData.ContainerProperties, &azcosmos.CreateContainerOptions{
		ThroughputProperties: &throughputProperties,
	})
	if err != nil {
		return err
	}

	container, err := database.NewContainer(containerResponse.ContainerProperties.ID)
	if err != nil {
		return err
	}
	// Deleting the database will delete the container

	// Insert test data
	for _, item := range queryContext.TestData.Data {
		// Build partition key
		var deserializedItem map[string]interface{}
		err = json.Unmarshal(item, &deserializedItem)
		if err != nil {
			return err
		}

		partitionKey := azcosmos.NewPartitionKey()
		for _, path := range queryContext.TestData.ContainerProperties.PartitionKeyDefinition.Paths {
			if path[0] != '/' {
				return fmt.Errorf("Partition key path %s must start with '/'", path)
			}
			property := path[1:]
			if strings.Contains(property, "/") {
				return fmt.Errorf("Partition key path %s must not contain '/'", path)
			}
			if value, ok := deserializedItem[property]; ok {
				switch v := value.(type) {
				case string:
					partitionKey = partitionKey.AppendString(v)
				default:
					return fmt.Errorf("Unsupported partition key type %T", v)
				}
			} else {
				return fmt.Errorf("Partition key property %s not found in item", property)
			}
		}

		jsonItem, err := item.MarshalJSON()
		if err != nil {
			return err
		}

		_, err = container.CreateItem(context, partitionKey, jsonItem, nil)
		if err != nil {
			return err
		}
	}

	fn(context, client, database, container)
	return nil
}

func resolvePath(baseDir, relativePath string) string {
	// Resolve the path relative to the base directory
	if path.IsAbs(relativePath) {
		return relativePath
	}
	return path.Join(baseDir, relativePath)
}

func loadTestData(path, uniqueId string) (TestData, error) {
	testDataFile, err := os.Open(path)
	if err != nil {
		return TestData{}, err
	}

	defer testDataFile.Close()
	var testData TestData
	err = json.NewDecoder(testDataFile).Decode(&testData)
	if err != nil {
		return TestData{}, err
	}

	// Fill in the container ID
	testData.ContainerProperties.ID = uniqueId
	return testData, nil
}

func loadResults(path string) ([]json.RawMessage, error) {
	resultsFile, err := os.Open(path)
	if err != nil {
		return nil, err
	}

	defer resultsFile.Close()
	var results []json.RawMessage
	err = json.NewDecoder(resultsFile).Decode(&results)
	if err != nil {
		return nil, err
	}

	return results, nil
}

func createClient(endpoint, key string) (*azcosmos.Client, error) {
	// Create a client with a custom transport that skips TLS verification
	// Since there's a self-signed certificate in the emulator, we need to skip verification
	transport := &http.Client{Transport: &http.Transport{
		TLSClientConfig: &tls.Config{InsecureSkipVerify: true},
	}}

	options := &azcosmos.ClientOptions{ClientOptions: azcore.ClientOptions{
		Transport: transport,
	}}

	// Open a cosmos client
	keyCredential, err := azcosmos.NewKeyCredential(key)
	if err != nil {
		return nil, err
	}
	return azcosmos.NewClientWithKey(endpoint, keyCredential, options)
}

func getenvOrDefault(key, def string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return def
}

func runIntegrationTest(t *testing.T, querySetPath string) {
	// Default to the emulator and it's well-known (not secret) key.
	endpoint := getenvOrDefault("AZURE_COSMOS_ENDPOINT", "https://localhost:8081")
	key := getenvOrDefault("AZURE_COSMOS_KEY", "C2y6yDjf5/R+ob0N8A7Cgv30VRDJIWEHLM+4QDU5DE2nQ9nDuVTqobD4b8mGGyPMbIZnqyMsEcaGQy67XIw/Jw==")

	// Find the integration test baseline file
	wd, err := os.Getwd()
	require.NoError(t, err)

	fullPath := path.Join(wd, querySetPath)
	require.FileExists(t, fullPath)

	queryContext, err := LoadQueryContext(context.Background(), fullPath)
	if err != nil {
		t.Errorf("Failed to load query context: %v", err)
		return
	}

	err = queryContext.RunWithTestResources(context.Background(), endpoint, key, func(ctx context.Context, client *azcosmos.Client, database *azcosmos.DatabaseClient, container *azcosmos.ContainerClient) {
		for _, query := range queryContext.Query.Queries {
			t.Run(query.Name, func(t *testing.T) {
				// Load results for this test
				resultsFileName := fmt.Sprintf("%s.results.json", query.Name)
				resultsPath := path.Join(queryContext.Directory, resultsFileName)
				results, err := loadResults(resultsPath)
				require.NoError(t, err)

				err = runSingleQuery(t, results, query, container)
				require.NoError(t, err)
			})
		}
	})
	require.NoError(t, err)
}

func runSingleQuery(t *testing.T, results []json.RawMessage, query QuerySpec, container *azcosmos.ContainerClient) error {
	// Run the query
	queryEngine := azcosmoscx.NewQueryEngine()
	queryOptions := &azcosmos.QueryOptions{
		QueryEngine: queryEngine,
	}

	pager := container.NewQueryItemsPager(query.Text, azcosmos.NewPartitionKey(), queryOptions)

	actualItemCount := 0
	for pager.More() {
		page, err := pager.NextPage(context.TODO())
		if err != nil {
			return err
		}

		for idx, actualJson := range page.Items {
			actualItemCount++
			// Find the expected item
			if idx >= len(results) {
				return fmt.Errorf("expected %d items, but got %d", len(results), actualItemCount)
			}
			expectedJson := results[idx]

			// Compare with jsondiff, but make sure we ignore the system-generated properties.
			diff, err := jsondiff.CompareJSON(expectedJson, actualJson, jsondiff.Ignores(
				"/_rid",
				"/_self",
				"/_etag",
				"/_attachments",
				"/_ts",
			))
			if err != nil {
				return err
			}
			assert.Empty(t, diff, "Item %d does not match expected result. Diff: %s", idx, diff)
		}
	}

	assert.Equal(t, len(results), actualItemCount, "Expected %d items, but got %d", len(results), actualItemCount)
	return nil
}
