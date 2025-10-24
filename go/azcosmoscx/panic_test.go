//go:build panic_test

package azcosmoscx_test

import (
	"testing"

	"github.com/Azure/azure-cosmos-client-engine/go/azcosmoscx"
)

// Panic triggers a panic inside the Cosmos CX library for testing purposes.
// To test this, you need to set the `panic_test` build tag when running tests, and obviously the tests will fail due to the panic.
// You can set the "COSMOSCX_GOTAGS" environment variable to "panic_test" to enable this tag when running `just test_go`.
func TestPanic(t *testing.T) {
	azcosmoscx.CosmosPanic()
}
