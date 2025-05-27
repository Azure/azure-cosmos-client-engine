// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

using Microsoft.Azure.Cosmos;
using Newtonsoft.Json;
using Newtonsoft.Json.Linq;

public class QuerySet
{
    public required string Name { get; set; }
    public required string TestData { get; set; }
    public required List<QuerySpec> Queries { get; set; }
}

public class QuerySpec
{
    public required string Name { get; set; }
    public required string Query { get; set; }
}

public class TestData
{
    public required ContainerProperties ContainerProperties { get; set; }
    public required JArray Data { get; set; }
}

public class BaselineGenerator
{
    public static async Task GenerateBaselineAsync(string endpoint, string key, string baselineFile)
    {
        // Load the file
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

        // Load test data
        var testDataPath = querySet.TestData;
        var fullTestDataPath = Path.Combine(containingDirectory, testDataPath);
        var testDataJson = await File.ReadAllTextAsync(fullTestDataPath);
        var testData = JsonConvert.DeserializeObject<TestData>(testDataJson);
        if (testData == null)
        {
            Console.WriteLine("Error: Unable to parse the test data file.");
            return;
        }

        // Connect to Cosmos DB
        var options = endpoint == "https://localhost:8081" ? new CosmosClientOptions
        {
            ServerCertificateCustomValidationCallback = (cert, chain, errors) =>
            {
                // Accept all certificates, when using the emulator
                return true;
            }
        } : new CosmosClientOptions;
        var client = new CosmosClient(endpoint, key, options);
        var uniqueName = $"baseline_{querySet.Name}_{Guid.NewGuid():N}";

        // Create a new database and container
        Console.WriteLine($"Creating database: {uniqueName}");
        var database = (await client.CreateDatabaseIfNotExistsAsync(uniqueName, throughput: 40_000)).Database;
        try
        {
            testData.ContainerProperties.Id = uniqueName;

            Console.WriteLine($"Creating container: {uniqueName}");
            var container = (await database.CreateContainerIfNotExistsAsync(testData.ContainerProperties, throughput: 40_000)).Container;

            // Insert test data
            Console.WriteLine($"Inserting test data into container: {uniqueName}");
            foreach (var item in testData.Data)
            {
                await container.CreateItemAsync(item);
            }

            foreach (var querySpec in querySet.Queries)
            {
                await GenerateQueryBaselineAsync(container, querySet, querySpec, containingDirectory);
            }
        }
        finally
        {
            Console.WriteLine($"Deleting database: {uniqueName}");
            await database.DeleteAsync();
        }
    }

    private static async Task GenerateQueryBaselineAsync(Container container, QuerySet querySet, QuerySpec querySpec, string containingDirectory)
    {
        var outputDir = Path.Combine(containingDirectory, querySet.Name);
        if (!Directory.Exists(outputDir))
        {
            Directory.CreateDirectory(outputDir);
        }
        var resultFilePath = Path.Combine(outputDir, $"{querySpec.Name}.results.json");

        // Execute the query
        Console.WriteLine($"Executing query: {querySpec.Query}");
        var results = container.GetItemQueryIterator<JToken>(querySpec.Query);
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
