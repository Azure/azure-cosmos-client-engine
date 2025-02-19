import sys
import azure.cosmos
import azure_cosmoscx
import warnings
import urllib3
import timeit

warnings.filterwarnings(
    "ignore", category=urllib3.exceptions.InsecureRequestWarning)

endpoint = "https://localhost:8081"
key = "C2y6yDjf5/R+ob0N8A7Cgv30VRDJIWEHLM+4QDU5DE2nQ9nDuVTqobD4b8mGGyPMbIZnqyMsEcaGQy67XIw/Jw=="
databaseName = "SampleDB"
containerName = "SampleContainer"
use_cosmoscx = False
query = None

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
        query = sys.argv[i]
    i += 1

if query is None:
    print(
        "Usage: main.py [--endpoint ENDPOINT] [--key KEY] [--database DATABASE] [--container CONTAINER] [--use-cosmoscx] QUERY")
    sys.exit(1)

query_engine = None
if use_cosmoscx:
    print("Using cosmoscx query engine")
    azure_cosmoscx.enable_tracing()
    query_engine = azure_cosmoscx.QueryEngine()
else:
    print("Using python query engine")

client = azure.cosmos.CosmosClient(
    endpoint, key, connection_verify=False, query_engine=query_engine)
db = client.get_database_client(databaseName)
container = db.get_container_client(containerName)


def run_query():
    items = container.query_items(query, enable_cross_partition_query=True)
    pager = items.by_page(None)
    for page in pager:
        for item in page:
            print(item)


# Run once, unmeasured, to warm up
run_query()

# count = 10
# time = timeit.timeit(stmt=run_query, number=count)
# print(f"Ran {count} times in {time * 1000}ms, {(time / count) * 1000}ms per run")

print()
print()
print()
