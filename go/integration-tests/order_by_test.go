// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

package integrationtests

import (
	"path"
	"testing"
)

const querySetRoot = path.Join("..", "..", "baselines", "queries")

func TestOrderBy(t *testing.T) {
	runIntegrationTest(t, "../../baselines/queries/order_by.json")
}
