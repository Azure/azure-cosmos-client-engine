# Copyright (c) Microsoft Corporation.
# Licensed under the MIT License.
import os

class TestConfig:
    local_host = 'https://localhost:8081/'
    # [SuppressMessage("Microsoft.Security", "CS002:SecretInNextLine", Justification="Cosmos DB Emulator Key")]
    masterKey = os.getenv('ACCOUNT_KEY',
                          'C2y6yDjf5/R+ob0N8A7Cgv30VRDJIWEHLM+4QDU5DE2nQ9nDuVTqobD4b8mGGyPMbIZnqyMsEcaGQy67XIw/Jw==')
    host = os.getenv('ACCOUNT_HOST', local_host)
    connection_str = os.getenv('ACCOUNT_CONNECTION_STR', 'AccountEndpoint={};AccountKey={};'.format(host, masterKey))
