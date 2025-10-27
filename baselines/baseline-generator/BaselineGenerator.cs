// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

using System.Text.Json;
using Microsoft.Azure.Cosmos;
using Newtonsoft.Json;
using Newtonsoft.Json.Linq;

public class QuerySet
{
    /// <summary>
    /// The name of the query set, used for generating the baseline file.
    /// </summary>
    public required string Name { get; set; }

    /// <summary>
    /// The path to the test data file, relative to the baseline file.
    /// </summary>
    public required string TestData { get; set; }

    /// <summary>
    /// The list of queries to be executed against the container.
    /// Each query should have a unique name across all files.
    /// </summary>
    public required List<QuerySpec> Queries { get; set; }
}

public class QuerySpec
{
    /// <summary>
    /// The name of the query, used for generating the baseline file.
    /// </summary>
    public required string Name { get; set; }

    /// <summary>
    /// The name of the container, found in the test data file, where the query will be executed.
    /// </summary>
    public required string Container { get; set; }

    /// <summary>
    /// The SQL query to be executed against the container.
    /// </summary>
    public required string Query { get; set; }

    /// <summary>
    /// Parameters that can be used in the query, prefixed with '@'.
    /// </summary>
    public Dictionary<string, JToken> Parameters { get; set; } = new Dictionary<string, JToken>();
}

public class TestData
{
    /// <summary>
    /// The data to be inserted into the container for testing.
    /// </summary>
    public required JArray Data { get; set; }

    /// <summary>
    /// Parameters that can be used in queries, prefixed with '@testData_'.
    /// </summary>
    public Dictionary<string, JToken> Parameters { get; set; } = new Dictionary<string, JToken>();

    /// <summary>
    /// Containers that will be created for the test data.
    /// Each container should have a unique name across all files.
    /// Each container will be created with the properties listed here and the same set of data, specified in <see cref="Data" /> will be inserted into each container.
    /// </summary>
    public required List<ContainerProperties> Containers { get; set; }
}

public class BaselineGenerator
{
    const int ThroughputForTwoPartitions = 12_000;
    public static async Task GenerateBaselineAsync(CosmosClient client, string baselineFile, IEnumerable<string>? queryNames = null, string? databaseName = null, bool containersExist = false)
    {
        var containingDirectory = Path.GetDirectoryName(baselineFile);
        if (containingDirectory == null)
        {
            Console.WriteLine("Error: Unable to determine the containing directory.");
            return;
        }

        var file = await File.ReadAllTextAsync(baselineFile);
        var querySet = JsonConvert.DeserializeObject<QuerySet>(file);
        if (querySet == null || querySet.Queries == null)
        {
            Console.WriteLine($"Error: Unable to parse the baseline file: {baselineFile}");
            return;
        }

        var testDataPath = querySet.TestData;
        var fullTestDataPath = Path.Combine(containingDirectory, testDataPath);
        var testDataJson = await File.ReadAllTextAsync(fullTestDataPath);
        var testData = JsonConvert.DeserializeObject<TestData>(testDataJson);
        if (testData == null)
        {
            Console.WriteLine("Error: Unable to parse the test data file.");
            return;
        }

        bool weCreatedDatabase = false;
        if (string.IsNullOrEmpty(databaseName))
        {
            databaseName = $"baseline_{querySet.Name}_{Guid.NewGuid():N}";
            weCreatedDatabase = true;
        }

        Console.WriteLine("Running query set: " + querySet.Name);
        Console.WriteLine($"- Using database: {databaseName}");
        var database = (await client.CreateDatabaseIfNotExistsAsync(databaseName, throughput: ThroughputForTwoPartitions)).Database;
        try
        {
            Dictionary<string, Container> containers;
            if (containersExist)
            {
                Console.WriteLine($"- Assuming containers already exist, skipping creation.");
                containers = new Dictionary<string, Container>();
                foreach (var containerProperties in testData.Containers)
                {
                    var container = database.GetContainer(containerProperties.Id);
                    containers.Add(containerProperties.Id, container);
                }
            }
            else
            {
                Console.WriteLine($"- Creating containers and inserting test data.");
                containers = await CreateTestContainersAsync(database, testData);
            }

            foreach (var querySpec in querySet.Queries)
            {
                // Skip queries not in the list if query names are specified
                if (queryNames != null && queryNames.Any() && !queryNames.Contains(querySpec.Name))
                {
                    continue;
                }
                Console.WriteLine("- Running query: " + querySpec.Name);
                if (!containers.TryGetValue(querySpec.Container, out var container))
                {
                    Console.WriteLine($"Error: Container {querySpec.Container} not found in test data.");
                    continue;
                }
                await GenerateQueryBaselineAsync(container, testData, querySet, querySpec, containingDirectory);
            }
        }
        finally
        {
            if (weCreatedDatabase)
            {
                Console.WriteLine($"Deleting database: {databaseName}");
                await database.DeleteAsync();
            }
        }
    }

    private static async Task<Dictionary<string, Container>> CreateTestContainersAsync(Database database, TestData testData)
    {
        var containers = new Dictionary<string, Container>();
        foreach (var containerProperties in testData.Containers)
        {
            Console.WriteLine($"-- Creating container: {containerProperties.Id}");
            var container = (await database.CreateContainerIfNotExistsAsync(containerProperties, throughput: ThroughputForTwoPartitions)).Container;

            // Insert test data
            Console.WriteLine($"-- Inserting test data into container: {containerProperties.Id}");
            foreach (var item in testData.Data)
            {
                await container.CreateItemAsync(item);
            }
            containers.Add(containerProperties.Id, container);

            // Validate the feed ranges
            var feedRanges = await container.GetFeedRangesAsync();
            Console.WriteLine($"-- Container {containerProperties.Id} has {feedRanges.Count} feed ranges.");
            if (feedRanges.Count < 2)
            {
                Console.Error.WriteLine($"Warning: Container {containerProperties.Id} has less than 2 feed ranges, it may not be properly partitioned.");
            }
        }
        return containers;
    }

    private static async Task GenerateQueryBaselineAsync(Container container, TestData testData, QuerySet querySet, QuerySpec querySpec, string containingDirectory)
    {
        var outputDir = Path.Combine(containingDirectory, querySet.Name);
        if (!Directory.Exists(outputDir))
        {
            Directory.CreateDirectory(outputDir);
        }
        var resultFilePath = Path.Combine(outputDir, $"{querySpec.Name}.results.json");

        // Build the query
        var query = new QueryDefinition(querySpec.Query);
        foreach (var parameter in querySpec.Parameters)
        {
            query.WithParameter($"@{parameter.Key}", parameter.Value);
        }
        // Add parameters from the test data, using the 'testData_' prefix.
        foreach (var parameter in testData.Parameters)
        {
            query.WithParameter($"@testData_{parameter.Key}", parameter.Value);
        }

        Console.WriteLine($"-- Executing query: {querySpec.Query}");
        var results = container.GetItemQueryIterator<JToken>(query);
        var resultList = new List<JToken>();
        while (results.HasMoreResults)
        {
            var response = await results.ReadNextAsync();

            foreach (var token in response)
            {
                if (token is JObject obj)
                {
                    // Remove system-generated properties
                    obj.Remove("_rid");
                    obj.Remove("_self");
                    obj.Remove("_etag");
                    obj.Remove("_ts");
                }

                resultList.Add(token);
            }
        }

        // Save the results
        Console.WriteLine($"Saving results to: {resultFilePath}");
        var resultJson = JsonConvert.SerializeObject(resultList, Formatting.Indented);
        await File.WriteAllTextAsync(resultFilePath, resultJson);
        Console.WriteLine($"Baseline generation completed successfully. Results saved to: {resultFilePath}");
    }
}
