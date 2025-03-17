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
)

type TestData struct {
	ContainerProperties azcosmos.ContainerProperties `json:"containerProperties"`
	Data                []json.RawMessage            `json:"data"`
}

type QuerySpec struct {
	Name     string `json:"name"`
	TestData string `json:"testData"`
	Result   string `json:"result"`
	Query    string `json:"query"`
}

func getenvOrDefault(key, def string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return def
}

func runIntegrationTest(t *testing.T, baselinePath string) {
	endpoint := getenvOrDefault("AZURE_COSMOS_ENDPOINT", "https://localhost:8081")
	key := getenvOrDefault("AZURE_COSMOS_KEY", "C2y6yDjf5/R+ob0N8A7Cgv30VRDJIWEHLM+4QDU5DE2nQ9nDuVTqobD4b8mGGyPMbIZnqyMsEcaGQy67XIw/Jw==")
	if endpoint == "" || key == "" {
		t.Errorf("AZURE_COSMOS_ENDPOINT and AZURE_COSMOS_KEY environment variables must be set")
		return
	}

	// Find the integration test baseline file
	wd, err := os.Getwd()
	require.NoError(t, err)

	fullPath := path.Join(wd, baselinePath)
	require.FileExists(t, fullPath)

	baselineDir := path.Dir(fullPath)

	// Read the integration test baseline file
	baselineFile, err := os.Open(fullPath)
	require.NoError(t, err)
	defer baselineFile.Close()
	var querySpec QuerySpec
	err = json.NewDecoder(baselineFile).Decode(&querySpec)
	require.NoError(t, err)

	// Find the test data file
	testDataFile, err := os.Open(path.Join(baselineDir, querySpec.TestData))
	require.NoError(t, err)
	defer testDataFile.Close()
	var testData TestData
	err = json.NewDecoder(testDataFile).Decode(&testData)
	require.NoError(t, err)

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
	require.NoError(t, err)
	client, err := azcosmos.NewClientWithKey(endpoint, keyCredential, options)
	require.NoError(t, err)

	uniqueBytes := make([]byte, 4)
	_, err = rand.Read(uniqueBytes)
	require.NoError(t, err)
	encoded := base64.RawURLEncoding.EncodeToString(uniqueBytes)
	uniqueName := fmt.Sprintf("IntegrationTest_%s_%s", querySpec.Name, encoded)

	throughputProperties := azcosmos.NewManualThroughputProperties(40000)
	dbResponse, err := client.CreateDatabase(context.TODO(), azcosmos.DatabaseProperties{
		ID: uniqueName,
	}, &azcosmos.CreateDatabaseOptions{
		ThroughputProperties: &throughputProperties,
	})
	require.NoError(t, err)

	db, err := client.NewDatabase(dbResponse.DatabaseProperties.ID)
	require.NoError(t, err)
	defer func() {
		_, err := db.Delete(context.TODO(), nil)
		if err != nil {
			fmt.Printf("Failed to delete database %s: %v\n", dbResponse.DatabaseProperties.ID, err)
		}
	}()

	testData.ContainerProperties.ID = uniqueName
	containerResponse, err := db.CreateContainer(context.TODO(), testData.ContainerProperties, &azcosmos.CreateContainerOptions{
		ThroughputProperties: &throughputProperties,
	})
	require.NoError(t, err)

	container, err := db.NewContainer(containerResponse.ContainerProperties.ID)
	require.NoError(t, err)

	// Insert test data
	for _, item := range testData.Data {
		// Build partition key
		var deserializedItem map[string]interface{}
		err = json.Unmarshal(item, &deserializedItem)
		require.NoError(t, err)

		partitionKey := azcosmos.NewPartitionKey()
		for _, path := range testData.ContainerProperties.PartitionKeyDefinition.Paths {
			if path[0] != '/' {
				t.Errorf("Partition key path %s must start with '/'", path)
				return
			}
			property := path[1:]
			if strings.Contains(property, "/") {
				t.Errorf("Partition key path %s must not contain '/'", path)
				return
			}
			if value, ok := deserializedItem[property]; ok {
				switch v := value.(type) {
				case string:
					partitionKey = partitionKey.AppendString(v)
				default:
					t.Errorf("Unsupported partition key type %T", v)
					return
				}
			} else {
				t.Errorf("Partition key property %s not found in item", property)
				return
			}
		}

		jsonItem, err := item.MarshalJSON()
		require.NoError(t, err)

		_, err = container.CreateItem(context.TODO(), partitionKey, jsonItem, nil)
		require.NoError(t, err)
	}

	// Load expected results
	resultsFilePath := path.Join(baselineDir, querySpec.Result)
	expectedResultsFile, err := os.Open(resultsFilePath)
	require.NoError(t, err)
	defer expectedResultsFile.Close()
	var expectedResults []json.RawMessage
	err = json.NewDecoder(expectedResultsFile).Decode(&expectedResults)
	require.NoError(t, err)

	// Run the query
	queryEngine := azcosmoscx.NewQueryEngine()
	queryOptions := &azcosmos.QueryOptions{
		UnstablePreviewQueryEngine: queryEngine,
	}

	pager := container.NewQueryItemsPager(querySpec.Query, azcosmos.NewPartitionKey(), queryOptions)

	for pager.More() {
		page, err := pager.NextPage(context.TODO())
		require.NoError(t, err)

		for idx, item := range page.Items {
			var deserializedActualItem map[string]interface{}
			var deserializedExpectedItem map[string]interface{}
			err = json.Unmarshal(item, &deserializedActualItem)
			require.NoError(t, err)
			err = json.Unmarshal(expectedResults[idx], &deserializedExpectedItem)
			require.NoError(t, err)
			actualId := deserializedActualItem["id"]
			expectedId := deserializedExpectedItem["id"]

			// Comparing IDs is sufficient FOR NOW.
			// In the future, we may want to compare the entire item.
			// Comparing JSON objects can get tricky fast, so this is simpler.
			assert.Equal(t, expectedId, actualId, "Item %d does not match expected item. Actual ID: %v, Expected ID: %v", idx, actualId, expectedId)
		}
	}
}

func TestStreamingOrderBy1(t *testing.T) {
	runIntegrationTest(t, "../../baselines/queries/order_by.json")
}
