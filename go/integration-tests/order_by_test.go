package integrationtests

import (
	"testing"
)

func TestOrderBy(t *testing.T) {
	runIntegrationTest(t, "../../baselines/queries/order_by.json")
}
