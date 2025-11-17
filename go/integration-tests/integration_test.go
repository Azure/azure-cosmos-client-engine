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
	"math"
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
	Containers []azcosmos.ContainerProperties `json:"containers"`
	Data       []json.RawMessage              `json:"data"`
	Parameters map[string]interface{}         `json:"parameters"`
}

type QuerySet struct {
	Name     string      `json:"name"`
	TestData string      `json:"testData"`
	Queries  []QuerySpec `json:"queries"`
}

type QuerySpec struct {
	Name       string                 `json:"name"`
	Text       string                 `json:"query"`
	Container  string                 `json:"container"`
	Parameters map[string]interface{} `json:"parameters"`
	Validators map[string]string      `json:"validators"`
}

const ValidationIgnore = "ignore"
const ValidationEqual = "equal"
const ValidationOrderedDescending = "orderedDescending"
const ValidationOrderedAscending = "orderedAscending"
const AllowedFloatError = 1e-6

type QueryContext struct {
	Query      QuerySet
	TestData   TestData
	UniqueId   string
	Directory  string
	Containers map[string]*azcosmos.ContainerClient
}

type ValidationError struct {
	Item     int
	Property string
	Message  string
	Expected interface{}
	Actual   interface{}
}

var Validators = map[string]func(t *testing.T, propertyName string, expected, actual []interface{}) []ValidationError{
	ValidationIgnore: func(t *testing.T, propertyName string, expected, actual []interface{}) []ValidationError {
		return nil
	},
	ValidationEqual: func(t *testing.T, propertyName string, expected, actual []interface{}) []ValidationError {
		errors := make([]ValidationError, 0)
		for i, exp := range expected {
			if i >= len(actual) {
				return []ValidationError{{Item: i, Property: propertyName, Expected: exp, Actual: nil}}
			}
			act := actual[i]
			expectedPropertyValue := expected[i].(map[string]interface{})[propertyName]
			actualPropertyValue, ok := act.(map[string]interface{})[propertyName]
			if !ok {
				errors = append(errors, ValidationError{Item: i, Property: propertyName, Message: "missing expected property", Expected: expectedPropertyValue, Actual: nil})
				continue
			}

			validationError, err := validateJsonEquality(t, i, propertyName, expectedPropertyValue, actualPropertyValue)
			if err != nil {
				return []ValidationError{{Item: i, Property: propertyName, Message: fmt.Sprintf("error during validation: %v", err), Expected: expectedPropertyValue, Actual: actualPropertyValue}}
			}
			if validationError != nil {
				errors = append(errors, *validationError)
			}
		}
		return errors
	},
	ValidationOrderedDescending: func(t *testing.T, propertyName string, expected, actual []interface{}) []ValidationError {
		return validateOrdered(propertyName, actual, false)
	},
	ValidationOrderedAscending: func(t *testing.T, propertyName string, expected, actual []interface{}) []ValidationError {
		return validateOrdered(propertyName, actual, true)
	},
}

var DefaultValidators = map[string]string{
	"_etag":        ValidationIgnore,
	"_rid":         ValidationIgnore,
	"_self":        ValidationIgnore,
	"_ts":          ValidationIgnore,
	"_attachments": ValidationIgnore,
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

	return QueryContext{querySpec, testData, uniqueId, queryResultDir, nil}, nil
}

func (queryContext *QueryContext) RunWithTestResources(context context.Context, endpoint, key string, fn func(context context.Context, client *azcosmos.Client, database *azcosmos.DatabaseClient, queryContext *QueryContext)) error {
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

	// Create all containers
	queryContext.Containers = make(map[string]*azcosmos.ContainerClient)
	for _, containerProps := range queryContext.TestData.Containers {
		containerResponse, err := database.CreateContainer(context, containerProps, &azcosmos.CreateContainerOptions{
			ThroughputProperties: &throughputProperties,
		})
		if err != nil {
			return err
		}

		container, err := database.NewContainer(containerResponse.ContainerProperties.ID)
		if err != nil {
			return err
		}
		queryContext.Containers[containerProps.ID] = container

		// Insert test data into this container
		for _, item := range queryContext.TestData.Data {
			// Build partition key
			var deserializedItem map[string]interface{}
			err = json.Unmarshal(item, &deserializedItem)
			if err != nil {
				return err
			}

			partitionKey := azcosmos.NewPartitionKey()
			for _, path := range containerProps.PartitionKeyDefinition.Paths {
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
	}

	fn(context, client, database, queryContext)
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

	// Container IDs are already unique within the test data, no need to modify them
	return testData, nil
}

func loadExpectedResults(path string) ([]interface{}, error) {
	resultsFile, err := os.Open(path)
	if err != nil {
		return nil, err
	}

	defer resultsFile.Close()
	var results []interface{}
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
	azcosmoscx.EnableTracing()

	// Default to the emulator and it's well-known (not secret) key.
	endpoint := getenvOrDefault("AZURE_COSMOS_ENDPOINT", "https://localhost:8081")
	key := getenvOrDefault("AZURE_COSMOS_KEY", "C2y6yDjf5/R+ob0N8A7Cgv30VRDJIWEHLM+4QDU5DE2nQ9nDuVTqobD4b8mGGyPMbIZnqyMsEcaGQy67XIw/Jw==")

	// Find the integration test baseline file
	fullPath := path.Join("..", "..", "baselines", "queries", querySetPath)
	require.FileExists(t, fullPath)

	queryContext, err := LoadQueryContext(context.Background(), fullPath)
	if err != nil {
		t.Errorf("Failed to load query context: %v", err)
		return
	}

	err = queryContext.RunWithTestResources(context.Background(), endpoint, key, func(ctx context.Context, client *azcosmos.Client, database *azcosmos.DatabaseClient, queryContext *QueryContext) {
		for _, query := range queryContext.Query.Queries {
			t.Run(query.Name, func(t *testing.T) {
				// Find the container for this query
				container, ok := queryContext.Containers[query.Container]
				if !ok {
					t.Errorf("Query '%s' references container '%s', but that container was not found", query.Name, query.Container)
					return
				}

				// Load results for this test
				resultsFileName := fmt.Sprintf("%s.results.json", query.Name)
				resultsPath := path.Join(queryContext.Directory, resultsFileName)
				results, err := loadExpectedResults(resultsPath)
				require.NoError(t, err)

				err = runSingleQuery(t, &queryContext.TestData, results, query, container)
				require.NoError(t, err)
			})
		}
	})
	require.NoError(t, err)
}

func floatEqual(index int, expected, actual, allowedError float64) *ValidationError {
	delta := math.Abs(expected - actual)
	if delta > allowedError {
		return &ValidationError{
			Item:     index,
			Property: "<item>",
			Message:  fmt.Sprintf("float mismatch: expected %f, got %f (delta %f exceeds allowed error %f)", expected, actual, delta, AllowedFloatError),
			Expected: expected,
			Actual:   actual,
		}
	}
	return nil
}

func runSingleQuery(t *testing.T, testData *TestData, expectedResults []interface{}, query QuerySpec, container *azcosmos.ContainerClient) error {
	// Set up query parameters
	parameters := make([]azcosmos.QueryParameter, 0, len(query.Parameters)+len(testData.Parameters))
	for name, value := range query.Parameters {
		paramName := fmt.Sprintf("@%s", name)
		parameters = append(parameters, azcosmos.QueryParameter{Name: paramName, Value: value})
	}
	for name, value := range testData.Parameters {
		paramName := fmt.Sprintf("@testData_%s", name)
		parameters = append(parameters, azcosmos.QueryParameter{Name: paramName, Value: value})
	}

	queryEngine := azcosmoscx.NewQueryEngine()
	queryOptions := &azcosmos.QueryOptions{
		QueryEngine:     queryEngine,
		QueryParameters: parameters,
	}

	pager := container.NewQueryItemsPager(query.Text, azcosmos.NewPartitionKey(), queryOptions)

	actualItemCount := 0
	actualItems := make([]interface{}, 0, len(expectedResults))
	for pager.More() {
		page, err := pager.NextPage(context.TODO())
		if err != nil {
			return err
		}

		for idx, actualJson := range page.Items {
			actualItemCount++
			var actualItem interface{}
			err := json.Unmarshal(actualJson, &actualItem)
			if err != nil {
				return fmt.Errorf("failed to unmarshal item %d: %v", idx, err)
			}
			actualItems = append(actualItems, actualItem)
		}
	}
	assert.Equal(t, len(actualItems), actualItemCount, "Expected %d items, but got %d", len(actualItems), actualItemCount)

	if len(actualItems) != len(expectedResults) {
		return fmt.Errorf("expected %d results, but got %d", len(expectedResults), len(actualItems))
	}

	if len(actualItems) == 0 {
		// No results to validate
		return nil
	}

	// Check if the first expected item is a map, and if so, validate using property validators
	var errors []ValidationError
	if _, ok := expectedResults[0].(map[string]interface{}); ok {
		var err error
		errors, err = validateUsingValidators(t, actualItems, expectedResults, query.Validators)
		if err != nil {
			return err
		}
	} else {
		// Just do a direct comparison of each object. We already know the counts match
		for i := 0; i < len(expectedResults); i++ {
			validationError, err := validateJsonEquality(t, i, "<item>", expectedResults[i], actualItems[i])
			if err != nil {
				return err
			}
			if validationError != nil {
				errors = append(errors, *validationError)
			}
		}
	}

	if len(errors) > 0 {
		for _, err := range errors {
			t.Errorf("Item %d, property '%s' validation failed: %s\nExpected: %v\nActual: %v\nMessage: %s",
				err.Item, err.Property, err.Message, err.Expected, err.Actual, err.Message)
		}
	}
	return nil
}

func validateJsonEquality(t *testing.T, index int, property string, expected, actual interface{}) (*ValidationError, error) {
	// special handling for floats to allow for small differences
	switch expected.(type) {
	case float32:
		expectedFloat := expected.(float32)
		actualFloat := actual.(float32)
		return floatEqual(0, float64(expectedFloat), float64(actualFloat), AllowedFloatError), nil
	case float64:
		expectedFloat := expected.(float64)
		actualFloat := actual.(float64)
		return floatEqual(0, expectedFloat, actualFloat, AllowedFloatError), nil
	default:
		patch, err := jsondiff.Compare(expected, actual, jsondiff.Ignores("_etag", "_rid", "_self", "_ts", "_attachments"))
		if err != nil {
			return nil, fmt.Errorf("error comparing item %d: %v", index, err)
		}
		if len(patch) > 0 {
			return &ValidationError{
				Item:     index,
				Property: property,
				Message:  fmt.Sprintf("item mismatch: %s", patch),
				Expected: expected,
				Actual:   actual,
			}, nil
		}
		break
	}
	return nil, nil
}

func validateUsingValidators(t *testing.T, actualItems, expectedResults []interface{}, validators map[string]string) ([]ValidationError, error) {
	firstItem := actualItems[0].(map[string]interface{})
	properties := make([]string, 0, len(firstItem))
	for property := range firstItem {
		properties = append(properties, property)
	}
	errors := make([]ValidationError, 0)
	for _, property := range properties {
		validator, ok := validators[property]
		if !ok {
			validator, ok = DefaultValidators[property]
			if !ok {
				validator = ValidationEqual // Default to equal if no validator is specified
			}
		}
		validateFunc, ok := Validators[validator]
		if !ok {
			return nil, fmt.Errorf("unknown validator %s for property %s", validator, property)
		}
		localErrors := validateFunc(t, property, expectedResults, actualItems)
		errors = append(errors, localErrors...)
	}
	return errors, nil
}

// validateOrdered checks that the actual results are ordered by the specified property.
// ascending determines whether to check for ascending (true) or descending (false) order.
func validateOrdered(propertyName string, actual []interface{}, ascending bool) []ValidationError {
	errors := make([]ValidationError, 0)
	if len(actual) == 0 {
		return []ValidationError{{Item: 0, Property: propertyName, Message: "no actual results to validate against"}}
	}
	if len(actual) == 1 {
		return nil // A single item is always ordered
	}
	for i := 1; i < len(actual); i++ {
		currentValue, ok := actual[i-1].(map[string]interface{})[propertyName]
		if !ok {
			errors = append(errors, ValidationError{Item: i - 1, Property: propertyName, Message: "missing expected property", Expected: nil, Actual: nil})
			continue
		}
		nextValue, ok := actual[i].(map[string]interface{})[propertyName]
		if !ok {
			errors = append(errors, ValidationError{Item: i, Property: propertyName, Message: "missing expected property", Expected: nil, Actual: nil})
			continue
		}

		// Compare current and next values
		// TODO: Handle different types (e.g., strings, numbers)
		currentFloat := currentValue.(float64)
		nextFloat := nextValue.(float64)

		var orderValid bool
		if ascending {
			orderValid = currentFloat <= nextFloat
		} else {
			orderValid = currentFloat >= nextFloat
		}

		if !orderValid {
			orderDirection := "ascending"
			if !ascending {
				orderDirection = "descending"
			}
			errors = append(errors, ValidationError{
				Item:     i,
				Property: propertyName,
				Message:  fmt.Sprintf("expected %v to be %s relative to %v", nextFloat, orderDirection, currentFloat),
				Expected: currentValue,
				Actual:   nextValue,
			})
		}
	}
	return errors
}
