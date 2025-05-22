// Default to the emulator, and it's well-known (non-secret) key
var endpoint = "https://localhost:8081";
var key = "C2y6yDjf5/R+ob0N8A7Cgv30VRDJIWEHLM+4QDU5DE2nQ9nDuVTqobD4b8mGGyPMbIZnqyMsEcaGQy67XIw/Jw==";
string? baselineFile = null;

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
            Console.WriteLine("  --key <key>                 The key for the Cosmos DB.");
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

if (File.Exists(baselineFile))
{
    // Single baseline file
    Console.WriteLine($"Generating baseline: {baselineFile}");
    await BaselineGenerator.GenerateBaselineAsync(endpoint, key, baselineFile);
}
else if (Directory.Exists(baselineFile))
{
    Console.WriteLine($"Generating all baselines in: {baselineFile}");
    var subdirFiles = Directory.EnumerateFiles(baselineFile, "*.json");
    foreach (var subdirFile in subdirFiles)
    {
        Console.WriteLine($"Generating baseline: {subdirFile}");
        await BaselineGenerator.GenerateBaselineAsync(endpoint, key, subdirFile);
    }
}
