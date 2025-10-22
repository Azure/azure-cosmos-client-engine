// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

package integrationtests

import (
	"testing"
)

func TestOrderBy(t *testing.T) {
	runIntegrationTest(t, "order_by.json")
}

func TestAggregates(t *testing.T) {
	runIntegrationTest(t, "aggregates.json")
}
