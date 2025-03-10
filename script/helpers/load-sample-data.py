import azure.cosmos
import sys
import warnings
import urllib3
import json

warnings.filterwarnings(
    "ignore", category=urllib3.exceptions.InsecureRequestWarning)

endpoint = "https://localhost:8081"
key = "C2y6yDjf5/R+ob0N8A7Cgv30VRDJIWEHLM+4QDU5DE2nQ9nDuVTqobD4b8mGGyPMbIZnqyMsEcaGQy67XIw/Jw=="
databaseName = "TestDB"
containerName = "TestContainer"
use_cosmoscx = False
sample_path = None

# Parse arguments
i = 1
while i < len(sys.argv):
    if sys.argv[i] == "--endpoint":
        endpoint = sys.argv[i+1]
        i += 1
    elif sys.argv[i] == "--key":
        key = sys.argv[i+1]
        i += 1
    elif sys.argv[i] == "--database":
        databaseName = sys.argv[i+1]
        i += 1
    elif sys.argv[i] == "--container":
        containerName = sys.argv[i+1]
        i += 1
    elif sys.argv[i] == "--use-cosmoscx":
        use_cosmoscx = True
    else:
        sample_path = sys.argv[i]
    i += 1

if sample_path is None:
    print(
        "Usage: load-sample-data.py [--endpoint ENDPOINT] [--key KEY] [--database DATABASE] [--container CONTAINER] [--use-cosmoscx] SAMPLE_PATH")
    sys.exit(1)

print(
    f"Loading sample data from {sample_path} into {containerName} in {databaseName} on {endpoint}")

with open(sample_path, "r") as f:
    sample_data = json.load(f)

if sample_data is None or sample_data['data'] is None:
    print(f"Failed to load sample data from {sample_path}")
    sys.exit(1)

client = azure.cosmos.CosmosClient(
    endpoint, key, connection_verify=False)

# Warn the user that we're creating a DB with a min of 4000 RUs, which could be costly unless you're using the emulator
print(f"Creating database {databaseName} with 4000-40000 RUs (autoscaled), in order to force multiple physical partitions. This may incur significant costs unless you're using the emulator.")
if endpoint == "https://localhost:8081":
    print("NOTE: You appear to be using the emulator, which will not incur any costs.")
confirm = input(
    f"Are you sure you want to create this database? (y/n): ")
if confirm.lower() != "y":
    print("Aborting.")

throughput = azure.cosmos.ThroughputProperties(auto_scale_max_throughput=40000)
db = client.create_database_if_not_exists(
    databaseName, offer_throughput=throughput)
container = db.create_container_if_not_exists(
    containerName, partition_key=azure.cosmos.PartitionKey(["/categoryId"]))

data = sample_data['data']
print(f"Inserting {len(data)} items into container...")
for item in data:
    container.upsert_item(item)
print(f"Inserted {len(data)} items into container {containerName} in database {databaseName} on {endpoint}")
