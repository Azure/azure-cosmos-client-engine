const cosmoscx = require(".");
const { CosmosClient } = require("@azure/cosmos")

let endpoint = "https://localhost:8081"
let key = "C2y6yDjf5/R+ob0N8A7Cgv30VRDJIWEHLM+4QDU5DE2nQ9nDuVTqobD4b8mGGyPMbIZnqyMsEcaGQy67XIw/Jw=="
let databaseName = "SampleDB"
let containerName = "SampleContainer"
let use_cosmoscx = false
let query = null

for (let i = 0; i < process.argv.length; i++) {
    const key = process.argv[i]
    switch (key) {
        case "--endpoint":
            endpoint = process.argv[i + 1]
            i++
            break
        case "--key":
            key = process.argv[i + 1]
            i++
            break
        case "--database":
            databaseName = process.argv[i + 1]
            i++
            break
        case "--container":
            containerName = process.argv[i + 1]
            i++
            break
        case "--use-cosmoscx":
            use_cosmoscx = true
            break
        default:
            query = key
            break
    }
}

console.log(`Using endpoint: ${endpoint}`)
console.log(`Using key: ${key}`)
console.log(`Using database: ${databaseName}`)
console.log(`Using container: ${containerName}`)
console.log(`Using query: ${query}`)
console.log(`Using cosmoscx: ${use_cosmoscx}`)