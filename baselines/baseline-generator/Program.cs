// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

// Default to the emulator, and it's well-known (non-secret) key
using Microsoft.Azure.Cosmos;

const string EmulatorKey = "C2y6yDjf5/R+ob0N8A7Cgv30VRDJIWEHLM+4QDU5DE2nQ9nDuVTqobD4b8mGGyPMbIZnqyMsEcaGQy67XIw/Jw==";
var endpoint = Environment.GetEnvironmentVariable("AZURE_COSMOS_ENDPOINT") ?? "https://localhost:8081";
string? key = Environment.GetEnvironmentVariable("AZURE_COSMOS_KEY");
string? baselineFile = null;
List<string> queryNames = new List<string>();
var validateCertificate = true;
var forceAad = false;
string? db = null;
var containersExist = false;

for (int i = 0; i < args.Length; i++)
{
    var arg = args[i].ToLowerInvariant();
    switch (arg)
    {
        case "--help":
        case "-h":
            Console.WriteLine("Usage: baseline-generator [options] [baseline-file]");
            Console.WriteLine("Options:");
            Console.WriteLine("  --endpoint <endpoint>       The endpoint of the Cosmos DB.");
            Console.WriteLine("  --key <key>                 The key for the Cosmos DB (if omitted, default AAD credential will be used).");
            Console.WriteLine("  --query <query-name>        Specific query to execute (can be specified multiple times).");
            Console.WriteLine("  --insecure                 Do not validate server certificate (for emulator).");
            Console.WriteLine("  --force-aad                Force AAD authentication (even for the emulator).");
            Console.WriteLine("  --db <database>           The database to use (if omitted, a new database will be created).");
            Console.WriteLine("  --containers-exist           Assume the containers already exist with appropriate test data and do not attempt to create them.");
            return;
        case "--endpoint":
        case "-e":
            if (i + 1 < args.Length)
            {
                endpoint = args[++i];
            }
            else
            {
                Console.WriteLine("Error: Missing value for --endpoint.");
                return;
            }
            break;
        case "--db":
            if (i + 1 < args.Length)
            {
                db = args[++i];
            }
            else
            {
                Console.WriteLine("Error: Missing value for --db.");
                return;
            }
            break;
        case "--key":
        case "-k":
            if (i + 1 < args.Length)
            {
                key = args[++i];
            }
            else
            {
                Console.WriteLine("Error: Missing value for --key.");
                return;
            }
            break;
        case "--query":
        case "-q":
            if (i + 1 < args.Length)
            {
                queryNames.Add(args[++i]);
            }
            else
            {
                Console.WriteLine("Error: Missing value for --query.");
                return;
            }
            break;
        case "--insecure":
            validateCertificate = false;
            break;
        case "--force-aad":
            forceAad = true;
            break;
        case "--containers-exist":
            containersExist = true;
            break;
        default:
            if (baselineFile == null)
            {
                baselineFile = args[i];
            }
            else
            {
                Console.WriteLine("Error: Unexpected argument: " + args[i]);
                return;
            }
            break;
    }
}

if (string.IsNullOrEmpty(baselineFile))
{
    Console.WriteLine("Error: Baseline file is required.");
    return;
}

if (endpoint == "https://localhost:8081" && string.IsNullOrEmpty(key))
{
    Console.WriteLine("Detected emulator endpoint, disabling certificate validation.");
    validateCertificate = false;
    if (!forceAad)
    {
        Console.WriteLine("Detected emulator endpoint, using well-known key.");
        key = EmulatorKey;
    }
}

var options = new CosmosClientOptions();
if (!validateCertificate)
{
    options.ServerCertificateCustomValidationCallback = (cert, chain, errors) =>
    {
        // Accept all certificates, when using the emulator
        return true;
    };
}

CosmosClient client;
if (string.IsNullOrEmpty(key) || forceAad)
{
    Console.WriteLine("Using AAD authentication.");
    var cred = new Azure.Identity.DefaultAzureCredential();
    client = new CosmosClient(endpoint, cred, options);
}
else
{
    Console.WriteLine("Using key authentication.");
    client = new CosmosClient(endpoint, key, options);
}

if (File.Exists(baselineFile))
{
    // Single baseline file
    Console.WriteLine($"Generating baseline: {baselineFile}");
    await BaselineGenerator.GenerateBaselineAsync(client, baselineFile, queryNames, db, containersExist);
}
else if (Directory.Exists(baselineFile))
{
    Console.WriteLine($"Generating all baselines in: {baselineFile}");
    var subdirFiles = Directory.EnumerateFiles(baselineFile, "*.json");
    foreach (var subdirFile in subdirFiles)
    {
        Console.WriteLine($"Generating baseline: {subdirFile}");
        await BaselineGenerator.GenerateBaselineAsync(client, subdirFile, queryNames, db, containersExist);
    }
}
