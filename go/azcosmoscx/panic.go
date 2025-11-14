//go:build panic_test

package azcosmoscx

// void cosmoscx_v0_panic();
import "C"

// Panic triggers a panic inside the Cosmos CX library for testing purposes.
func CosmosPanic() {
	C.cosmoscx_v0_panic()
}
