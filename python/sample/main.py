import sys
import azure.cosmos
import warnings
import urllib3

warnings.filterwarnings(
    "ignore", category=urllib3.exceptions.InsecureRequestWarning)

endpoint = "https://localhost:8081"
key = "C2y6yDjf5/R+ob0N8A7Cgv30VRDJIWEHLM+4QDU5DE2nQ9nDuVTqobD4b8mGGyPMbIZnqyMsEcaGQy67XIw/Jw=="
databaseName = "SampleDB"
containerName = "SampleContainer"
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
    else:
        query = sys.argv[i]
    i += 1

if query is None:
    print(
        "Usage: main.py [--endpoint ENDPOINT] [--key KEY] [--database DATABASE] [--container CONTAINER] QUERY")
    sys.exit(1)

# TODO: Integrate the native query engine!

client = azure.cosmos.CosmosClient(endpoint, key, connection_verify=False)
db = client.get_database_client(databaseName)
container = db.get_container_client(containerName)

items = container.query_items(query, enable_cross_partition_query=True)

pager = items.by_page(None)
pageNumber = 0
for page in pager:
    print(f"*** PAGE {pageNumber} ***")
    for item in page:
        print(item)
    pageNumber += 1
