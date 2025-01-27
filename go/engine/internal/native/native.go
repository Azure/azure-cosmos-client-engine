// Package native provides low-level safe Go wrappers around the Cosmos Client Engine C API.
// This package isn't intended for direct usage, but rather as a building block for the higher-level query engine API.
package native

// TODO: We need to evaluate how to distribute the native library itself and how best to link it (static/shared).

// #cgo CFLAGS: -I${SRCDIR}/../../../../include
// #include <cosmoscx.h>
import "C"

func CosmosCX_Version() string {
	return C.GoString(C.cosmoscx_version())
}
