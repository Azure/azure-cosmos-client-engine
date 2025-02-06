package engine

// TODO: We need to evaluate how to distribute the native library itself and how best to link it (static/shared).

// #cgo CFLAGS: -I${SRCDIR}/../../include
// #include <cosmoscx.h>
import "C"

func Version() string {
	return C.GoString(C.cosmoscx_version())
}

// EnableTracing enables Cosmos Client Engine tracing.
// Once enabled, tracing cannot be disabled (for now). Tracing is controlled by setting the COSMOSCX_LOG environment variable, using the syntax of the `RUST_LOG` (https://docs.rs/env_logger/latest/env_logger/#enabling-logging) env var.
func EnableTracing() {
	C.cosmoscx_v0_tracing_enable()
}

type PartitionKeyRange struct {
	ID           string
	MinInclusive string
	MaxExclusive string
}
