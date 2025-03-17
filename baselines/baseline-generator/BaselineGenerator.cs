using Microsoft.Azure.Cosmos;
using Newtonsoft.Json;
using Newtonsoft.Json.Linq;

public class BaselineDescription
{
    public required string Name { get; set; }
    public required string TestData { get; set; }
    public required string Result { get; set; }
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
        var description = JsonConvert.DeserializeObject<BaselineDescription>(file);
        if (description == null)
        {
            Console.WriteLine("Error: Unable to parse the baseline file.");
            return;
        }

        // Load test data
        var testDataPath = description.TestData;
        var fullTestDataPath = Path.Combine(containingDirectory, testDataPath);
        var testDataJson = await File.ReadAllTextAsync(fullTestDataPath);
        var testData = JsonConvert.DeserializeObject<TestData>(testDataJson);
        if (testData == null)
        {
            Console.WriteLine("Error: Unable to parse the test data file.");
            return;
        }

        // Connect to Cosmos DB
        var client = new CosmosClient(endpoint, key);
        var uniqueName = $"BaselineGenerator_{Guid.NewGuid():N}";

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
                try
                {
                    await container.CreateItemAsync(item);
                }
                catch (CosmosException ex)
                {
                    Console.WriteLine($"Error inserting item: {ex.StatusCode} - {ex.Message}");
                }
            }

            // Execute the query
            Console.WriteLine($"Executing query: {description.Query}");
            var results = container.GetItemQueryIterator<JObject>(description.Query);
            var resultList = new List<JObject>();
            while (results.HasMoreResults)
            {
                var response = await results.ReadNextAsync();
                resultList.AddRange(response);
            }

            // Save the results
            var resultFilePath = Path.Combine(containingDirectory, description.Result);
            Console.WriteLine($"Saving results to: {resultFilePath}");
            var resultJson = JsonConvert.SerializeObject(resultList, Formatting.Indented);
            await File.WriteAllTextAsync(resultFilePath, resultJson);
            Console.WriteLine($"Baseline generation completed successfully. Results saved to: {resultFilePath}");
        }
        finally
        {
            Console.WriteLine($"Deleting database: {uniqueName}");
            await database.DeleteAsync();
        }
    }
}