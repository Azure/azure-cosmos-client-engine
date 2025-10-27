targetScope = 'resourceGroup'

// The name of the Cosmos DB account
param name string

// The location ID where the Cosmos DB account will be created. For example 'westus2'. Run 'az account list-locations' to get the list of locations.
param location string

// The location name where the Cosmos DB account will be created. For example 'West US 2'. Run 'az account list-locations' to get the list of locations.
param locationName string

resource testAccount 'Microsoft.DocumentDb/databaseAccounts@2025-05-01-preview' = {
  kind: 'GlobalDocumentDB'
  name: name
  location: location
  properties: {
    databaseAccountOfferType: 'Standard'
    locations: [
      {
        failoverPriority: 0
        locationName: locationName
        isZoneRedundant: true
      }
    ]
    backupPolicy: {
      type: 'Periodic'
      periodicModeProperties: {
        backupIntervalInMinutes: 240
        backupRetentionIntervalInHours: 8
        backupStorageRedundancy: 'Geo'
      }
    }
    isVirtualNetworkFilterEnabled: false
    virtualNetworkRules: []
    ipRules: [
      {
        ipAddressOrRange: '4.210.172.107'
      }
      {
        ipAddressOrRange: '13.88.56.148'
      }
      {
        ipAddressOrRange: '13.91.105.215'
      }
      {
        ipAddressOrRange: '40.91.218.243'
      }
      {
        ipAddressOrRange: '52.148.138.235'
      }
    ]
    minimalTlsVersion: 'Tls12'
    enableMultipleWriteLocations: true
    capabilities: []
    capacityMode: 'Provisioned'
    enableFreeTier: false
    disableLocalAuth: false
  }
}
